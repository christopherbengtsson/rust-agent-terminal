//! Dashboard-local editor artifacts and explicit live-source targets.
use serde::Deserialize;
use std::{
    ffi::OsStr,
    fs, io,
    io::IsTerminal,
    os::unix::{
        ffi::OsStrExt,
        fs::{DirBuilderExt, PermissionsExt},
        process::CommandExt,
    },
    path::PathBuf,
    process::{Command, ExitStatus},
    sync::{atomic::AtomicBool, mpsc},
};

#[derive(Clone, Debug)]
pub enum Target {
    Comparison(Box<crate::working_diff::Snapshot>),
    Source { path: PathBuf, line: usize },
}
#[derive(Debug)]
pub struct Prepared {
    pub path: PathBuf,
    pub line: usize,
    artifact: Option<PathBuf>,
    pub inspection: bool,
}
impl Drop for Prepared {
    fn drop(&mut self) {
        // Cleanup belongs to a worker too: large artifact deletion never stalls navigation.
        if let Some(dir) = self.artifact.take() {
            let _ = std::thread::spawn(move || {
                let _ = fs::remove_dir_all(dir);
            });
        }
    }
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub executable: String,
    #[serde(default)]
    pub arguments: Vec<String>,
    #[serde(default)]
    pub position: Position,
}
#[derive(Clone, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Position {
    #[default]
    None,
    Vim,
    Plus,
}
impl Config {
    pub fn from_environment() -> Result<Self, String> {
        let config = match std::env::var("RAT_EDITOR_JSON") {
            Ok(json) => serde_json::from_str::<Self>(&json)
                .map_err(|e| format!("Invalid RAT_EDITOR_JSON: {e}"))?,
            Err(std::env::VarError::NotPresent) => Self {
                executable: "vim".into(),
                arguments: vec![],
                position: Position::Vim,
            },
            Err(e) => return Err(format!("Invalid RAT_EDITOR_JSON: {e}")),
        };
        if config.executable.is_empty()
            || config.executable.contains('\0')
            || config.arguments.iter().any(|a| a.contains('\0'))
        {
            return Err(
                "Editor executable/arguments must be nonempty executable and NUL-free argv".into(),
            );
        }
        Ok(config)
    }
    pub fn run(&self, prepared: &Prepared) -> io::Result<ExitStatus> {
        let mut command = Command::new(&self.executable);
        command.args(&self.arguments);
        match self.position {
            Position::Vim => {
                if prepared.inspection {
                    command.args(["-R", "-n", "-i", "NONE"]);
                }
                command.arg(format!("+{}", prepared.line));
                command.arg("--");
            }
            Position::Plus => {
                command.arg(format!("+{}", prepared.line));
            }
            Position::None => {}
        }
        command.arg(&prepared.path).process_group(0);
        let mut child = command.spawn()?;
        if io::stdin().is_terminal() {
            if let Err(error) = foreground(child.id() as i32) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
            // A child reading before tcsetpgrp can receive SIGTTIN. Resume this owned group.
            unsafe {
                libc::kill(-(child.id() as i32), libc::SIGCONT);
            }
        }
        child.wait()
    }
}

/// Change foreground ownership while blocking SIGTTOU in this caller only.
/// The editor has an independent process group, so native Ctrl-C targets it alone.
pub fn foreground(group: i32) -> io::Result<()> {
    unsafe {
        let mut signals: libc::sigset_t = std::mem::zeroed();
        let mut previous: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut signals);
        libc::sigaddset(&mut signals, libc::SIGTTOU);
        let error = libc::pthread_sigmask(libc::SIG_BLOCK, &signals, &mut previous);
        if error != 0 {
            return Err(io::Error::from_raw_os_error(error));
        }
        let result = libc::tcsetpgrp(libc::STDIN_FILENO, group);
        let outcome = if result == -1 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        };
        libc::pthread_sigmask(libc::SIG_SETMASK, &previous, std::ptr::null_mut());
        outcome
    }
}

pub fn launch(
    config: Config,
    prepared: Prepared,
) -> io::Result<mpsc::Receiver<io::Result<ExitStatus>>> {
    let (sender, result) = mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name("external-editor".into())
        .spawn(move || {
            let outcome = config.run(&prepared);
            let _ = sender.send(outcome);
            drop(prepared);
        })?;
    Ok(result)
}

pub fn start(target: Target) -> io::Result<mpsc::Receiver<Result<Prepared, String>>> {
    let (sender, receiver) = mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name("editor-preparation".into())
        .spawn(move || {
            let _ = sender.send(prepare(target));
        })?;
    Ok(receiver)
}
fn prepare(target: Target) -> Result<Prepared, String> {
    match target {
        Target::Comparison(snapshot) => {
            let content = snapshot.editor_content.as_ref().ok_or(
                "Owner did not return a complete comparison artifact; upgrade the supervisor",
            )?;
            let dir = std::env::temp_dir().join(format!("rat-inspection-{}", uuid::Uuid::new_v4()));
            fs::DirBuilder::new()
                .mode(0o700)
                .create(&dir)
                .map_err(|e| e.to_string())?;
            let mut prepared = Prepared {
                path: dir.join("comparison.diff"),
                line: 1,
                artifact: Some(dir),
                inspection: true,
            };
            fs::write(&prepared.path, content).map_err(|e| e.to_string())?;
            fs::set_permissions(&prepared.path, fs::Permissions::from_mode(0o400))
                .map_err(|e| e.to_string())?;
            prepared.line = snapshot
                .editor_line
                .filter(|line| *line > 0 && *line <= content.lines().count())
                .ok_or(
                    "Owner did not return a valid comparison position; upgrade the supervisor",
                )?;
            Ok(prepared)
        }
        Target::Source { path, line } => Ok(Prepared {
            path,
            line,
            artifact: None,
            inspection: false,
        }),
    }
}

#[derive(Clone, Debug, serde::Serialize, Deserialize)]
pub struct SourceResponse {
    pub request: String,
    pub workspace: String,
    pub result: Result<(Vec<u8>, usize), String>,
}

pub struct SourceJob {
    worker: std::thread::JoinHandle<()>,
    cancelled: std::sync::Arc<AtomicBool>,
    pub result: mpsc::Receiver<SourceResponse>,
}
impl SourceJob {
    pub fn start(
        checkout: crate::git::CheckoutIdentity,
        request: String,
        workspace: String,
        path: Option<Vec<u8>>,
        line: usize,
    ) -> io::Result<Self> {
        let cancelled = std::sync::Arc::new(AtomicBool::new(false));
        let cancellation = cancelled.clone();
        let (sender, result) = mpsc::sync_channel(1);
        let worker = std::thread::Builder::new()
            .name("editor-source-location".into())
            .spawn(move || {
                let result = resolve_source(checkout, path, &cancellation)
                    .map(|path| (crate::session::path_bytes(&path), line.max(1)));
                let _ = sender.send(SourceResponse {
                    request,
                    workspace,
                    result,
                });
            })?;
        Ok(Self {
            worker,
            cancelled,
            result,
        })
    }
    pub fn is_finished(&self) -> bool {
        self.worker.is_finished()
    }
    pub fn cancel(&self) {
        self.cancelled
            .store(true, std::sync::atomic::Ordering::Release);
    }
}
impl Drop for SourceJob {
    fn drop(&mut self) {
        self.cancel();
    }
}

/// Execution-side checkout validation. Dashboard receives only the explicit local edit location.
fn resolve_source(
    checkout: crate::git::CheckoutIdentity,
    path: Option<Vec<u8>>,
    cancelled: &AtomicBool,
) -> Result<PathBuf, String> {
    let current = crate::git::resolve_checkout(&checkout.root, cancelled)?;
    if current != checkout {
        return Err("Checkout identity changed; refresh/adopt before source editing".into());
    }
    match path {
        Some(path) => {
            let relative = std::path::Path::new(OsStr::from_bytes(&path));
            if !relative
                .components()
                .all(|c| matches!(c, std::path::Component::Normal(_)))
            {
                return Err("Unsupported source path; no editable file opened".into());
            }
            let target = checkout.root.join(relative).canonicalize().map_err(|e| {
                format!("Source missing/unavailable; inspect its comparison instead: {e}")
            })?;
            if !target.starts_with(&checkout.root) || !target.is_file() {
                return Err("Source is not an existing regular file within this checkout; inspect comparison instead".into());
            }
            Ok(target)
        }
        None => Ok(checkout.root),
    }
}

/// Position an actual source buffer at the new-side hunk nearest the visible diff line.
pub fn source_line(content: &str, visible: usize) -> usize {
    let mut line = 1;
    let mut hunk = false;
    for (index, text) in content.lines().enumerate().take(visible + 1) {
        if text.starts_with("@@ ") {
            if let Some(new) = text.split_whitespace().find_map(|s| s.strip_prefix('+')) {
                line = new
                    .split(',')
                    .next()
                    .and_then(|n| n.parse().ok())
                    .unwrap_or(1);
                hunk = true;
            }
        } else if hunk && index != visible && (text.starts_with('+') || text.starts_with(' ')) {
            line += 1;
        }
    }
    line.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, process::Command, sync::atomic::AtomicBool};

    #[test]
    fn dashboard_editor_refresh_is_attributed_and_keeps_navigation_context() {
        use crate::{
            app::App,
            events::{Effect, Observation, Request},
            working_diff::{Response, View},
        };
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let mut app = App::new("/retained/project".into());
        app.workspace_selected = Some("workspace".into());
        app.history_scroll = 17;
        app.history_raw = true;
        app.selected = Some(3);
        let mut view = View::new("workspace".into(), "name".into());
        view.scroll = 9;
        view.horizontal = 8;
        app.diff = Some(view);
        let key = KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE);
        for version in [0, 1] {
            app.session.editor_protocol = version;
            assert!(app.input(key).is_none());
            assert!(
                app.editor_message
                    .as_deref()
                    .unwrap()
                    .contains("upgraded supervisor")
            );
        }
        app.session.editor_protocol = 2;
        let Some(Effect::Session(Request::InspectWorking {
            editor: true,
            request,
            workspace,
            ..
        })) = app.input(key)
        else {
            panic!("fresh owner-generated full comparison required")
        };
        assert_eq!(workspace, "workspace");
        app.observe_session(Observation {
            working_diff: Some(Response {
                workspace: "other".into(),
                request: request.clone(),
                result: Err("late".into()),
            }),
            ..Observation::default()
        });
        assert!(app.editor_ready.is_none());
        assert!(app.diff.as_ref().unwrap().loading);
        app.observe_session(Observation {
            working_diff: Some(Response {
                workspace,
                request,
                result: Err("capture failed".into()),
            }),
            ..Observation::default()
        });
        assert!(app.editor_ready.is_none());
        assert_eq!(
            app.diff.as_ref().unwrap().error.as_deref(),
            Some("capture failed")
        );
        assert_eq!(app.repository, PathBuf::from("/retained/project"));
        assert_eq!(app.workspace_selected.as_deref(), Some("workspace"));
        assert_eq!(app.selected, Some(3));
        assert_eq!(app.history_scroll, 17);
        assert!(app.history_raw);
        assert_eq!(app.diff.as_ref().unwrap().scroll, 9);
        assert_eq!(app.diff.as_ref().unwrap().horizontal, 8);
    }

    #[test]
    fn scrolled_new_file_opens_live_source_at_visible_line() {
        check_scrolled_new_file(false);
    }

    fn check_scrolled_new_file(native: bool) {
        use crate::{
            app::App,
            events::{Effect, Request},
            working_diff::View,
        };
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        use std::os::unix::ffi::OsStringExt;
        let dir = tempfile::tempdir().unwrap();
        assert!(
            Command::new("git")
                .arg("init")
                .arg("-q")
                .arg(dir.path())
                .status()
                .unwrap()
                .success()
        );
        fs::write(dir.path().join("new"), "one\ntwo\nthree\nfour\nfive\n").unwrap();
        let snapshot = crate::working_diff::inspect(
            dir.path(),
            "workspace",
            "capture",
            Some(b"new"),
            &AtomicBool::new(false),
        )
        .unwrap();
        let mut app = App::new(dir.path().into());
        app.session.editor_protocol = 2;
        let mut view = View::new("workspace".into(), "name".into());
        view.snapshot = Some(snapshot);
        view.scroll = 4;
        app.diff = Some(view);
        let Some(Effect::Session(Request::EditorSource { path, line, .. })) =
            app.input(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE))
        else {
            panic!("source editor request required")
        };
        assert_eq!(path, Some(b"new".to_vec()));
        assert_eq!(
            line, 4,
            "fourth source line is visible after the new-file header"
        );
        if native {
            let checkout =
                crate::git::resolve_checkout(dir.path(), &AtomicBool::new(false)).unwrap();
            let job = SourceJob::start(checkout, "source".into(), "workspace".into(), path, line)
                .unwrap();
            let (path, line) = job.result.recv().unwrap().result.unwrap();
            let prepared = start(Target::Source {
                path: PathBuf::from(std::ffi::OsString::from_vec(path)),
                line,
            })
            .unwrap()
            .recv()
            .unwrap()
            .unwrap();
            assert_vim_position(&prepared);
        }
    }

    #[test]
    fn renamed_and_abbreviated_comparison_opens_selected_raw_path_section() {
        check_renamed_comparison(false);
    }

    fn check_renamed_comparison(native: bool) {
        use crate::working_diff::{ComparisonOptions, Job};
        use std::os::unix::ffi::OsStrExt;
        let dir = tempfile::tempdir().unwrap();
        let git = |args: &[&str]| {
            assert!(
                Command::new("git")
                    .arg("-C")
                    .arg(dir.path())
                    .args(args)
                    .status()
                    .unwrap()
                    .success()
            )
        };
        git(&["init", "-q", "-b", "main"]);
        git(&["config", "user.email", "test@example.invalid"]);
        git(&["config", "user.name", "Test"]);
        fs::write(dir.path().join("old"), "retained rename content\n").unwrap();
        git(&["add", "."]);
        git(&["commit", "-qm", "base"]);
        for destination in [
            "renamed".into(),
            format!("{}/{}/new", "z".repeat(150), "q".repeat(150)),
        ] {
            if let Some(parent) = std::path::Path::new(&destination).parent() {
                fs::create_dir_all(dir.path().join(parent)).unwrap();
            }
            git(&["mv", "old", &destination]);
            fs::write(dir.path().join("a-first"), "other file\n").unwrap();
            for whole in [false, true] {
                let checkout =
                    crate::git::resolve_checkout(dir.path(), &AtomicBool::new(false)).unwrap();
                let job = Job::start_comparison(
                    checkout,
                    "workspace".into(),
                    "capture".into(),
                    Some(destination.as_bytes().to_vec()),
                    ComparisonOptions {
                        whole,
                        editor: true,
                        retained: None,
                        selected_ref: whole.then(|| b"refs/heads/main".to_vec()),
                    },
                )
                .unwrap();
                let snapshot = job.result.recv().unwrap().result.unwrap();
                assert_eq!(snapshot.selected.as_deref(), Some(destination.as_bytes()));
                assert!(
                    snapshot
                        .files
                        .iter()
                        .any(|f| f.path == destination.as_bytes()
                            && f.previous.as_deref() == Some(b"old"))
                );
                let prepared = start(Target::Comparison(Box::new(snapshot)))
                    .unwrap()
                    .recv()
                    .unwrap()
                    .unwrap();
                let artifact = fs::read_to_string(&prepared.path).unwrap();
                let heading = artifact.lines().nth(prepared.line - 1).unwrap();
                assert!(
                    heading.starts_with("=== old -> "),
                    "selected rename must open its section, got {heading:?}"
                );
                assert_ne!(prepared.path.as_os_str().as_bytes(), destination.as_bytes());
                if native {
                    assert_vim_position(&prepared);
                }
            }
            git(&["reset", "--hard", "-q", "HEAD"]);
        }
    }

    #[test]
    #[ignore = "actual installed macOS Vim positioning; run serially with editor qualification"]
    fn installed_vim_opens_captured_source_and_comparison_positions() {
        assert_eq!(std::env::consts::OS, "macos");
        check_scrolled_new_file(true);
        check_renamed_comparison(true);
    }

    fn assert_vim_position(prepared: &Prepared) {
        let dir = tempfile::tempdir().unwrap();
        let receipt = dir.path().join("position");
        let config = Config {
            executable: "/usr/bin/vim".into(),
            arguments: vec![
                "-es".into(),
                "-u".into(),
                "NONE".into(),
                "--cmd".into(),
                format!(
                    "autocmd VimEnter * call writefile([string(line('.'))], '{}') | qa!",
                    receipt.display()
                ),
            ],
            position: Position::Vim,
        };
        assert!(config.run(prepared).unwrap().success());
        assert_eq!(
            fs::read_to_string(receipt)
                .unwrap()
                .trim()
                .parse::<usize>()
                .unwrap(),
            prepared.line
        );
        eprintln!(
            "Installed Vim opened {} at line {}",
            prepared.path.display(),
            prepared.line
        );
    }

    #[test]
    fn live_source_position_uses_new_side_hunks() {
        let diff = "Staged\n@@ -3,2 +8,3 @@\n unchanged\n-removed\n+added\n+next";
        assert_eq!(source_line(diff, 2), 8);
        assert_eq!(source_line(diff, 4), 9);
        assert_eq!(source_line(diff, 5), 10);
    }

    #[test]
    fn inspection_is_stable_outside_checkout_and_live_source_never_invents_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for args in [
            &["init", "-q"][..],
            &["config", "user.email", "test@example.com"],
            &["config", "user.name", "Test"],
        ] {
            assert!(
                Command::new("git")
                    .arg("-C")
                    .arg(root)
                    .args(args)
                    .status()
                    .unwrap()
                    .success()
            );
        }
        fs::write(root.join("file"), "base\n").unwrap();
        assert!(
            Command::new("git")
                .arg("-C")
                .arg(root)
                .args(["add", "."])
                .status()
                .unwrap()
                .success()
        );
        assert!(
            Command::new("git")
                .arg("-C")
                .arg(root)
                .args(["commit", "-qm", "base"])
                .status()
                .unwrap()
                .success()
        );
        fs::write(root.join("file"), "first\n").unwrap();
        let mut snapshot = crate::working_diff::inspect(
            root,
            "workspace",
            "first-request",
            Some(b"file"),
            &AtomicBool::new(false),
        )
        .unwrap();
        snapshot.editor_content = Some(
            crate::working_diff::editor_comparison(&mut snapshot, &AtomicBool::new(false)).unwrap(),
        );
        let prepared = prepare(Target::Comparison(Box::new(snapshot.clone()))).unwrap();
        let original = fs::read_to_string(&prepared.path).unwrap();
        assert!(original.contains("first-request"));
        assert!(original.contains("+first"));
        assert!(!prepared.path.starts_with(root));
        fs::write(root.join("file"), "second\n").unwrap();
        assert_eq!(original, fs::read_to_string(&prepared.path).unwrap());
        let mut fresh = crate::working_diff::inspect(
            root,
            "workspace",
            "fresh-request",
            Some(b"file"),
            &AtomicBool::new(false),
        )
        .unwrap();
        fresh.editor_content = Some(
            crate::working_diff::editor_comparison(&mut fresh, &AtomicBool::new(false)).unwrap(),
        );
        let reopened = prepare(Target::Comparison(Box::new(fresh))).unwrap();
        let refreshed = fs::read_to_string(&reopened.path).unwrap();
        assert!(refreshed.contains("fresh-request") && refreshed.contains("+second"));
        assert!(!refreshed.contains("+first"));
        assert_ne!(prepared.path, reopened.path);

        let source = || {
            resolve_source(
                snapshot.checkout.clone(),
                Some(b"file".to_vec()),
                &AtomicBool::new(false),
            )
        };
        assert_eq!(source().unwrap(), root.join("file").canonicalize().unwrap());
        fs::remove_file(root.join("file")).unwrap();
        assert!(source().unwrap_err().contains("missing"));
        assert!(!root.join("file").exists());
        assert!(
            fs::read_to_string(&prepared.path)
                .unwrap()
                .contains("+first")
        );
        use std::os::unix::ffi::OsStringExt;
        let raw = std::ffi::OsString::from_vec(b"f\n$(touch injected);'quote'".to_vec());
        fs::write(root.join(&raw), "raw source").unwrap();
        let resolved = resolve_source(
            snapshot.checkout.clone(),
            Some(b"f\n$(touch injected);'quote'".to_vec()),
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(resolved, root.join(raw).canonicalize().unwrap());
        let destination = root.join("literal-argv-destination");
        fs::write(&destination, "old").unwrap();
        let prepared_source = prepare(Target::Source {
            path: destination.clone(),
            line: 1,
        })
        .unwrap();
        let config = Config {
            executable: "/bin/cp".into(),
            arguments: vec![resolved.to_str().unwrap().into()],
            position: Position::None,
        };
        assert!(config.run(&prepared_source).unwrap().success());
        assert_eq!(fs::read_to_string(destination).unwrap(), "raw source");
        assert!(!root.join("injected").exists());
        let external = tempfile::NamedTempFile::new().unwrap();
        std::os::unix::fs::symlink(external.path(), root.join("link")).unwrap();
        assert!(
            resolve_source(
                snapshot.checkout.clone(),
                Some(b"link".to_vec()),
                &AtomicBool::new(false)
            )
            .is_err()
        );
        assert!(
            resolve_source(
                snapshot.checkout,
                Some(b"../missing".to_vec()),
                &AtomicBool::new(false)
            )
            .is_err()
        );
    }
}
