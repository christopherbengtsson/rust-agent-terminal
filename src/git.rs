use std::{
    ffi::OsString,
    io::{ErrorKind, Read},
    os::unix::{io::AsRawFd, process::CommandExt},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc::{self, Receiver, SyncSender},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use crate::events::Event;

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Worktree {
    #[serde(with = "unix_path")]
    pub path: PathBuf,
    pub head: Option<String>,
    pub branch: Option<String>,
    pub bare: bool,
    pub locked: Option<String>,
    pub prunable: Option<String>,
}

impl Worktree {
    pub fn label(&self) -> &str {
        if self.bare {
            "(bare)"
        } else {
            self.branch.as_deref().unwrap_or("(detached HEAD)")
        }
    }
}

mod unix_path {
    use super::*;
    use serde::{Deserialize, Serialize};
    pub fn serialize<S: serde::Serializer>(path: &Path, serializer: S) -> Result<S::Ok, S::Error> {
        use std::os::unix::ffi::OsStrExt;
        path.as_os_str().as_bytes().serialize(serializer)
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<PathBuf, D::Error> {
        let bytes = Vec::<u8>::deserialize(deserializer)?;
        path_from_bytes(&bytes).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Discovery {
    pub request: u64,
    pub repository: Vec<u8>,
    pub result: Result<Vec<Worktree>, String>,
}

const DEADLINE: Duration = Duration::from_secs(2);
const OUTPUT_LIMIT: usize = 256 * 1024;

// Concrete execution-owned Git subprocess boundary. Process groups include Git's
// helpers; neither a pipe-filling child nor an inherited helper pipe can stall it.
fn run_git(
    repository: &Path,
    arguments: &[&str],
    cancelled: &AtomicBool,
) -> Result<Vec<u8>, String> {
    run_git_command(
        Command::new("git")
            .arg("-C")
            .arg(repository)
            .args(arguments),
        cancelled,
    )
}

pub(crate) fn run_git_command(
    command: &mut Command,
    cancelled: &AtomicBool,
) -> Result<Vec<u8>, String> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // SAFETY: setsid is async-signal-safe; this child owns a fresh process group.
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("Cannot run Git: {e}"))?;
    let group = child.id() as i32;
    let result = (|| {
        let mut stdout = child.stdout.take().ok_or("Git stdout unavailable")?;
        let mut stderr = child.stderr.take().ok_or("Git stderr unavailable")?;
        for fd in [stdout.as_raw_fd(), stderr.as_raw_fd()] {
            // SAFETY: valid owned pipe descriptors; preserve existing flags.
            unsafe {
                let flags = libc::fcntl(fd, libc::F_GETFL);
                if flags < 0 || libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) < 0 {
                    return Err(format!(
                        "Cannot bound Git output: {}",
                        std::io::Error::last_os_error()
                    ));
                }
            }
        }
        let deadline = Instant::now() + DEADLINE;
        let mut out = Vec::new();
        let mut err = Vec::new();
        let mut done = [false; 2];
        loop {
            if cancelled.load(Ordering::Acquire) {
                return Err("Git job cancelled".into());
            }
            if Instant::now() >= deadline {
                return Err("Git job timed out after 2 seconds; retry refresh/adoption".into());
            }
            // Bound each pipe's contribution per turn, even when a child floods it.
            for (index, pipe, bytes) in [
                (0, &mut stdout as &mut dyn Read, &mut out),
                (1, &mut stderr as &mut dyn Read, &mut err),
            ] {
                let mut block = [0; 8192];
                match pipe.read(&mut block) {
                    Ok(0) => done[index] = true,
                    Ok(n) => {
                        bytes.extend_from_slice(&block[..n]);
                        if bytes.len() > OUTPUT_LIMIT {
                            return Err("Git output exceeds 256 KiB limit".into());
                        }
                    }
                    Err(e)
                        if e.kind() == ErrorKind::WouldBlock
                            || e.kind() == ErrorKind::Interrupted => {}
                    Err(e) => return Err(format!("Cannot read Git output: {e}")),
                }
            }
            if let Some(status) = child
                .try_wait()
                .map_err(|e| format!("Cannot poll Git: {e}"))?
                && done.iter().all(|done| *done)
            {
                return if status.success() {
                    Ok(out)
                } else {
                    Err(format!(
                        "Git {status}: {}",
                        String::from_utf8_lossy(&err).trim()
                    ))
                };
            }
            thread::sleep(Duration::from_millis(5));
        }
    })();
    // Reap on every path, and remove helpers that outlive the direct process.
    // SAFETY: group belongs to the child created above, never a restored PID.
    unsafe {
        libc::kill(-group, libc::SIGKILL);
    }
    let _ = child.wait();
    result
}

pub struct GitWorker {
    requests: SyncSender<(u64, PathBuf)>,
    #[cfg(test)]
    repository: PathBuf,
    worker: thread::JoinHandle<()>,
    cancelled: Arc<AtomicBool>,
    pub events: Receiver<Event>,
}
impl Drop for GitWorker {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
    }
}
impl GitWorker {
    pub fn start(_repository: PathBuf) -> std::io::Result<Self> {
        let (requests, receiver) = mpsc::sync_channel::<(u64, PathBuf)>(1);
        let (sender, events) = mpsc::sync_channel(1);
        let cancelled = Arc::new(AtomicBool::new(false));
        let cancellation = cancelled.clone();
        let worker = thread::Builder::new()
            .name("git-discovery".into())
            .spawn(move || {
                loop {
                    if cancellation.load(Ordering::Acquire) {
                        break;
                    }
                    let (request, repository) =
                        match receiver.recv_timeout(Duration::from_millis(10)) {
                            Ok(request) => request,
                            Err(mpsc::RecvTimeoutError::Timeout) => continue,
                            Err(_) => break,
                        };
                    let result = discover_cancelled(&repository, &cancellation);
                    let mut event = Event::DiscoveryCompleted {
                        request,
                        repository: repository.clone(),
                        result,
                    };
                    loop {
                        if cancellation.load(Ordering::Acquire) {
                            return;
                        }
                        match sender.try_send(event) {
                            Ok(()) => break,
                            Err(mpsc::TrySendError::Disconnected(_)) => return,
                            Err(mpsc::TrySendError::Full(value)) => event = value,
                        }
                        thread::sleep(Duration::from_millis(5));
                    }
                }
            })?;
        Ok(Self {
            requests,
            #[cfg(test)]
            repository: _repository,
            worker,
            cancelled,
            events,
        })
    }
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }
    pub fn is_finished(&self) -> bool {
        self.worker.is_finished()
    }
    #[cfg(test)]
    pub fn refresh(&self, request: u64) -> Result<(), String> {
        self.refresh_for(request, self.repository.clone())
    }
    pub fn refresh_for(&self, request: u64, repository: PathBuf) -> Result<(), String> {
        if self.cancelled.load(Ordering::Acquire) {
            return Err("Previous discovery cancellation is finishing; retry".into());
        }
        self.requests
            .try_send((request, repository))
            .map_err(|error| format!("Cannot refresh: {error}"))
    }
}

// Restricted execution mode for a dashboard refused control of a live owner.
// It has no state directory, session interface, or mutation commands.
pub fn discovery_helper(repository: &Path) -> std::io::Result<()> {
    Err(Command::new("git")
        .arg("-C")
        .arg(repository)
        .args(["worktree", "list", "--porcelain", "-z"])
        .exec())
}
pub fn read_only_discovery(repository: &Path) -> Result<Vec<Worktree>, String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let output = run_git_command(
        Command::new(executable)
            .arg("--discover-worktrees")
            .arg(repository),
        &AtomicBool::new(false),
    )?;
    parse_worktrees(&output)
}

fn discover_cancelled(repository: &Path, cancelled: &AtomicBool) -> Result<Vec<Worktree>, String> {
    parse_worktrees(&run_git(
        repository,
        &["worktree", "list", "--porcelain", "-z"],
        cancelled,
    )?)
}
#[cfg(test)]
fn discover(repository: &Path) -> Result<Vec<Worktree>, String> {
    discover_cancelled(repository, &AtomicBool::new(false))
}

// Concrete shared-Git mutation admission, not session ownership. Call only in
// execution workers after canonical common-dir discovery and release as soon as
// Git mutation/rechecks finish. Creation/archive consume this in later tickets.
#[allow(dead_code)]
pub struct RepositoryMutation {
    common_dir: PathBuf,
}
static MUTATING_REPOSITORIES: std::sync::Mutex<Option<std::collections::HashSet<PathBuf>>> =
    std::sync::Mutex::new(None);
#[allow(dead_code)]
impl RepositoryMutation {
    pub fn acquire(common_dir: &Path, cancelled: &AtomicBool) -> Result<Self, String> {
        let common_dir = common_dir
            .canonicalize()
            .map_err(|e| format!("Repository identity unavailable: {e}"))?;
        let deadline = Instant::now() + DEADLINE;
        loop {
            if cancelled.load(Ordering::Acquire) {
                return Err("Repository mutation cancelled".into());
            }
            {
                let mut active = MUTATING_REPOSITORIES
                    .lock()
                    .map_err(|_| "Repository mutation ownership unavailable")?;
                if active.get_or_insert_default().insert(common_dir.clone()) {
                    return Ok(Self { common_dir });
                }
            }
            if Instant::now() >= deadline {
                return Err("Repository busy; retry the mutation".into());
            }
            thread::sleep(Duration::from_millis(5));
        }
    }
}
impl Drop for RepositoryMutation {
    fn drop(&mut self) {
        if let Ok(mut active) = MUTATING_REPOSITORIES.lock()
            && let Some(active) = active.as_mut()
        {
            active.remove(&self.common_dir);
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CheckoutIdentity {
    #[serde(with = "unix_path")]
    pub root: PathBuf,
    #[serde(with = "unix_path")]
    pub git_dir: PathBuf,
    #[serde(with = "unix_path")]
    pub common_dir: PathBuf,
}

pub(crate) fn resolve_checkout(
    path: &Path,
    cancelled: &AtomicBool,
) -> Result<CheckoutIdentity, String> {
    let canonical = path.canonicalize().map_err(|e| {
        format!("Checkout unavailable: {e}; refresh and choose an existing checkout")
    })?;
    let resolve = |args: &[&str]| -> Result<PathBuf, String> {
        let mut bytes = run_git(&canonical, args, cancelled)?;
        // Git adds exactly one terminator; do not trim path whitespace/newlines.
        if bytes.last() == Some(&b'\n') {
            bytes.pop();
        }
        path_from_bytes(&bytes)?
            .canonicalize()
            .map_err(|e| format!("Checkout unavailable: {e}"))
    };
    let root = resolve(&["rev-parse", "--path-format=absolute", "--show-toplevel"])?;
    let git_dir = resolve(&["rev-parse", "--absolute-git-dir"])?;
    let common_dir = resolve(&["rev-parse", "--path-format=absolute", "--git-common-dir"])?;
    Ok(CheckoutIdentity {
        root,
        git_dir,
        common_dir,
    })
}

pub struct AdoptionJob {
    worker: thread::JoinHandle<()>,
    cancelled: Arc<AtomicBool>,
    pub result: Receiver<Result<CheckoutIdentity, String>>,
}
impl AdoptionJob {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }
    pub fn is_finished(&self) -> bool {
        self.worker.is_finished()
    }
    pub fn start(path: PathBuf) -> std::io::Result<Self> {
        let cancelled = Arc::new(AtomicBool::new(false));
        let cancellation = cancelled.clone();
        let (sender, result) = mpsc::sync_channel(1);
        let worker = thread::Builder::new()
            .name("git-adoption".into())
            .spawn(move || {
                let _ = sender.try_send(resolve_checkout(&path, &cancellation));
            })?;
        Ok(Self {
            worker,
            cancelled,
            result,
        })
    }
}
impl Drop for AdoptionJob {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
    }
}

fn parse_worktrees(output: &[u8]) -> Result<Vec<Worktree>, String> {
    let mut worktrees = Vec::new();
    let mut current: Option<Worktree> = None;
    for field in output.split(|&byte| byte == 0) {
        if field.is_empty() {
            if let Some(worktree) = current.take() {
                worktrees.push(worktree);
            }
        } else if let Some(path) = field.strip_prefix(b"worktree ") {
            if current.is_some() {
                return Err("Malformed Git worktree record".into());
            }
            current = Some(Worktree {
                path: path_from_bytes(path)?,
                ..Worktree::default()
            });
        } else if let Some(worktree) = current.as_mut() {
            if let Some(head) = field.strip_prefix(b"HEAD ") {
                worktree.head = Some(String::from_utf8_lossy(head).into_owned());
            } else if let Some(branch) = field.strip_prefix(b"branch ") {
                let branch = branch.strip_prefix(b"refs/heads/").unwrap_or(branch);
                worktree.branch = Some(String::from_utf8_lossy(branch).into_owned());
            } else if field == b"bare" {
                worktree.bare = true;
            } else if field == b"locked" || field.starts_with(b"locked ") {
                worktree.locked = Some(
                    String::from_utf8_lossy(field.strip_prefix(b"locked ").unwrap_or(b""))
                        .into_owned(),
                );
            } else if field == b"prunable" || field.starts_with(b"prunable ") {
                worktree.prunable = Some(
                    String::from_utf8_lossy(field.strip_prefix(b"prunable ").unwrap_or(b""))
                        .into_owned(),
                );
            }
        } else {
            return Err("Malformed Git worktree output".into());
        }
    }
    if let Some(worktree) = current {
        worktrees.push(worktree);
    }
    Ok(worktrees)
}

fn path_from_bytes(bytes: &[u8]) -> Result<PathBuf, String> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        Ok(PathBuf::from(OsString::from_vec(bytes.to_vec())))
    }
    #[cfg(not(unix))]
    {
        std::str::from_utf8(bytes)
            .map(|path| PathBuf::from(OsString::from(path)))
            .map_err(|error| format!("Invalid worktree path: {error}"))
    }
}

#[cfg(test)]
mod tests {
    use std::{ffi::OsStr, time::Duration};

    use super::*;

    fn git(repository: &Path, arguments: &[&OsStr]) {
        let output = Command::new("git")
            .arg("-C")
            .arg(repository)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .args(arguments)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn discovers_real_linked_detached_locked_and_prunable_worktrees() {
        let temporary = tempfile::tempdir().unwrap();
        let main = temporary.path().join("main");
        std::fs::create_dir(&main).unwrap();
        git(&main, &["init".as_ref(), "-b".as_ref(), "main".as_ref()]);
        git(
            &main,
            &[
                "-c".as_ref(),
                "user.name=Fixture".as_ref(),
                "-c".as_ref(),
                "user.email=fixture@example.invalid".as_ref(),
                "-c".as_ref(),
                "commit.gpgsign=false".as_ref(),
                "commit".as_ref(),
                "--allow-empty".as_ref(),
                "-m".as_ref(),
                "initial".as_ref(),
            ],
        );
        let linked = temporary.path().join("topic with spaces\nand newline");
        git(
            &main,
            &[
                "worktree".as_ref(),
                "add".as_ref(),
                "-b".as_ref(),
                "topic".as_ref(),
                linked.as_os_str(),
            ],
        );
        git(
            &main,
            &[
                "worktree".as_ref(),
                "lock".as_ref(),
                "--reason".as_ref(),
                "keep this checkout".as_ref(),
                linked.as_os_str(),
            ],
        );
        let detached = temporary.path().join("detached");
        git(
            &main,
            &[
                "worktree".as_ref(),
                "add".as_ref(),
                "--detach".as_ref(),
                detached.as_os_str(),
            ],
        );

        // Query from a linked worktree, not just the main repository.
        let worktrees = discover(&linked).unwrap();
        assert_eq!(worktrees.len(), 3);
        let topic = worktrees
            .iter()
            .find(|w| w.branch.as_deref() == Some("topic"))
            .unwrap();
        assert_eq!(topic.path, linked.canonicalize().unwrap());
        assert_eq!(topic.locked.as_deref(), Some("keep this checkout"));
        let detached_record = worktrees
            .iter()
            .find(|w| w.path == detached.canonicalize().unwrap())
            .unwrap();
        assert_eq!(detached_record.label(), "(detached HEAD)");
        assert!(detached_record.head.is_some());

        std::fs::remove_dir_all(&detached).unwrap();
        assert!(
            discover(&main)
                .unwrap()
                .iter()
                .any(|w| w.prunable.is_some())
        );
    }

    #[test]
    fn bare_and_unborn_repositories_are_discoverable() {
        let temporary = tempfile::tempdir().unwrap();
        git(temporary.path(), &["init".as_ref(), "--bare".as_ref()]);
        let worktrees = discover(temporary.path()).unwrap();
        assert_eq!(worktrees.len(), 1);
        assert!(worktrees[0].bare);
        assert_eq!(worktrees[0].label(), "(bare)");

        let unborn = tempfile::tempdir().unwrap();
        git(
            unborn.path(),
            &["init".as_ref(), "-b".as_ref(), "main".as_ref()],
        );
        assert_eq!(
            discover(unborn.path()).unwrap()[0].branch.as_deref(),
            Some("main")
        );
    }

    #[test]
    fn worker_reports_non_repository_failure_as_an_event() {
        let temporary = tempfile::tempdir().unwrap();
        let worker = GitWorker::start(temporary.path().to_owned()).unwrap();
        worker.refresh(1).unwrap();
        let Event::DiscoveryCompleted { result, .. } =
            worker.events.recv_timeout(Duration::from_secs(5)).unwrap()
        else {
            panic!("unexpected event")
        };
        assert!(result.unwrap_err().contains("not a git repository"));
    }

    #[test]
    fn adoption_resolves_aliases_and_non_utf8_checkouts_without_mutating_git() {
        use std::os::unix::ffi::OsStringExt;
        let temp = tempfile::tempdir().unwrap();
        let main = temp.path().join("main");
        std::fs::create_dir(&main).unwrap();
        git(&main, &["init".as_ref(), "-b".as_ref(), "main".as_ref()]);
        git(
            &main,
            &[
                "-c".as_ref(),
                "user.name=Fixture".as_ref(),
                "-c".as_ref(),
                "user.email=fixture@example.invalid".as_ref(),
                "-c".as_ref(),
                "commit.gpgsign=false".as_ref(),
                "commit".as_ref(),
                "--allow-empty".as_ref(),
                "-m".as_ref(),
                "initial".as_ref(),
            ],
        );
        let invalid = temp
            .path()
            .join(OsString::from_vec(b"checkout-\xff\n".to_vec()));
        let linked = match std::fs::create_dir(&invalid) {
            Ok(()) => {
                std::fs::remove_dir(&invalid).unwrap();
                invalid.clone()
            }
            Err(error) if error.raw_os_error() == Some(libc::EILSEQ) => {
                eprintln!(
                    "Native filesystem rejects non-UTF-8 names ({error}); exercising byte-preserving wire/parser and explicit unavailable-path rejection instead"
                );
                temp.path().join("checkout with newline\n")
            }
            Err(error) => panic!("Cannot create fixture checkout: {error}"),
        };
        git(
            &main,
            &[
                "worktree".as_ref(),
                "add".as_ref(),
                "--detach".as_ref(),
                linked.as_os_str(),
            ],
        );
        let alias = temp.path().join("alias");
        std::os::unix::fs::symlink(&linked, &alias).unwrap();
        let resolve = |path: PathBuf| {
            AdoptionJob::start(path)
                .unwrap()
                .result
                .recv_timeout(Duration::from_secs(3))
                .unwrap()
        };
        if linked != invalid {
            assert!(resolve(invalid).is_err());
        }
        let identity = resolve(linked.clone()).unwrap();
        let aliased = resolve(alias).unwrap();
        assert_eq!(identity, aliased);
        assert_eq!(identity.root, linked.canonicalize().unwrap());
        assert_eq!(
            identity.common_dir,
            main.join(".git").canonicalize().unwrap()
        );
        assert_ne!(identity.git_dir, identity.common_dir);
        let json = serde_json::to_vec(&identity).unwrap();
        assert_eq!(
            serde_json::from_slice::<CheckoutIdentity>(&json).unwrap(),
            identity
        );
        let worktrees = discover(&main).unwrap();
        assert_eq!(worktrees.len(), 2);
        assert!(
            worktrees
                .iter()
                .any(|tree| tree.path == identity.root && tree.branch.is_none())
        );
        assert!(resolve(temp.path().join("missing")).is_err());
        let bare = temp.path().join("bare");
        std::fs::create_dir(&bare).unwrap();
        git(&bare, &["init".as_ref(), "--bare".as_ref()]);
        assert!(resolve(bare).is_err());
    }

    #[test]
    fn git_deadline_cancellation_and_output_limit_bound_children_and_helpers() {
        let cancelled = AtomicBool::new(false);
        let start = Instant::now();
        let result = run_git_command(
            Command::new("/bin/sh").args(["-c", "sleep 10 & wait"]),
            &cancelled,
        );
        assert!(result.unwrap_err().contains("timed out"));
        assert!(start.elapsed() < Duration::from_secs(3));
        let result = run_git_command(Command::new("/bin/sh").args(["-c", "yes x"]), &cancelled);
        assert!(result.unwrap_err().contains("256 KiB"));
        let cancelled = Arc::new(AtomicBool::new(false));
        let signal = cancelled.clone();
        let worker = thread::spawn(move || {
            run_git_command(Command::new("/bin/sh").args(["-c", "sleep 10"]), &signal)
        });
        cancelled.store(true, Ordering::Release);
        assert!(worker.join().unwrap().unwrap_err().contains("cancelled"));
    }

    #[test]
    fn repository_mutation_lease_serializes_aliases_but_not_other_repositories() {
        let temp = tempfile::tempdir().unwrap();
        let common = temp.path().join("common");
        let other = temp.path().join("other");
        std::fs::create_dir(&common).unwrap();
        std::fs::create_dir(&other).unwrap();
        std::os::unix::fs::symlink(&common, temp.path().join("alias")).unwrap();
        let cancel = AtomicBool::new(false);
        let first = RepositoryMutation::acquire(&common, &cancel).unwrap();
        let _unrelated = RepositoryMutation::acquire(&other, &cancel).unwrap();
        let (send, received) = mpsc::channel();
        let alias = temp.path().join("alias");
        let worker = thread::spawn(move || {
            let _second = RepositoryMutation::acquire(&alias, &AtomicBool::new(false)).unwrap();
            send.send(()).unwrap();
        });
        assert!(received.recv_timeout(Duration::from_millis(100)).is_err());
        drop(first);
        received.recv_timeout(Duration::from_secs(1)).unwrap();
        worker.join().unwrap();
        let cancelled = AtomicBool::new(true);
        assert!(RepositoryMutation::acquire(&common, &cancelled).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn parser_preserves_non_utf8_paths() {
        use std::os::unix::ffi::OsStrExt;
        let worktrees =
            parse_worktrees(b"worktree /tmp/task-\xff\0HEAD abc\0detached\0\0").unwrap();
        assert_eq!(worktrees[0].path.as_os_str().as_bytes(), b"/tmp/task-\xff");
        let json = serde_json::to_vec(&worktrees[0]).unwrap();
        let restored: Worktree = serde_json::from_slice(&json).unwrap();
        assert_eq!(restored.path.as_os_str().as_bytes(), b"/tmp/task-\xff");
    }
}
