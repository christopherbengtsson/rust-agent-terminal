use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::Command,
    sync::mpsc::{self, Receiver, SyncSender},
    thread,
};

use crate::events::Event;

#[derive(Debug, Default)]
pub struct Worktree {
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

pub struct GitWorker {
    requests: SyncSender<()>,
    pub events: Receiver<Event>,
}

impl GitWorker {
    pub fn start(repository: PathBuf) -> std::io::Result<Self> {
        let (requests, receiver) = mpsc::sync_channel(1);
        let (sender, events) = mpsc::sync_channel(1);
        thread::Builder::new()
            .name("git-discovery".into())
            .spawn(move || {
                while receiver.recv().is_ok() {
                    let result = discover(&repository);
                    if sender.send(Event::WorktreesLoaded(result)).is_err() {
                        break;
                    }
                }
            })?;
        Ok(Self { requests, events })
    }

    pub fn refresh(&self) -> Result<(), String> {
        self.requests
            .try_send(())
            .map_err(|error| format!("Cannot refresh: {error}"))
    }
}

fn discover(repository: &Path) -> Result<Vec<Worktree>, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repository)
        .args(["worktree", "list", "--porcelain", "-z"])
        .output()
        .map_err(|error| format!("Cannot run Git (is it installed?): {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "Git {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    parse_worktrees(&output.stdout)
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
        worker.refresh().unwrap();
        let Event::WorktreesLoaded(result) =
            worker.events.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(result.unwrap_err().contains("not a git repository"));
    }

    #[cfg(unix)]
    #[test]
    fn parser_preserves_non_utf8_paths() {
        use std::os::unix::ffi::OsStrExt;
        let worktrees =
            parse_worktrees(b"worktree /tmp/task-\xff\0HEAD abc\0detached\0\0").unwrap();
        assert_eq!(worktrees[0].path.as_os_str().as_bytes(), b"/tmp/task-\xff");
    }
}
