//! Read-only, execution-owned snapshots. Paths remain Unix bytes through Git argv.
use serde::{Deserialize, Serialize};
use std::{
    ffi::OsStr,
    fs::OpenOptions,
    io::Read,
    os::unix::{ffi::OsStrExt, fs::OpenOptionsExt},
    path::Path,
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

const FILE_LIMIT: usize = 1024;
const CONTENT_LIMIT: usize = 64 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChangedFile {
    pub path: Vec<u8>,
    pub previous: Option<Vec<u8>>,
    pub state: String,
    pub staged: bool,
    pub unstaged: bool,
    pub new: bool,
    pub conflict: bool,
}
impl ChangedFile {
    pub fn label(&self) -> String {
        let path = printable(&String::from_utf8_lossy(&self.path));
        match &self.previous {
            Some(previous) => format!(
                "{} -> {path}",
                printable(&String::from_utf8_lossy(previous))
            ),
            None => path,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub workspace: String,
    pub request: String,
    pub captured_millis: u64,
    pub checkout: crate::git::CheckoutIdentity,
    pub files: Vec<ChangedFile>,
    pub omitted: usize,
    pub selected: Option<Vec<u8>>,
    pub content: String,
    pub changed_during_capture: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Response {
    pub workspace: String,
    pub request: String,
    pub result: Result<Snapshot, String>,
}

/// Escape terminal controls, bidi overrides and separators; never interpret ANSI.
pub fn printable(text: &str) -> String {
    text.chars().flat_map(|c| {
        if c.is_control() || matches!(c, '\u{2028}'..='\u{202e}' | '\u{2066}'..='\u{2069}' | '\u{200b}'..='\u{200f}' | '\u{061c}') {
            c.escape_default().collect::<Vec<_>>()
        } else { vec![c] }
    }).collect()
}
fn content_printable(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .split('\n')
        .map(printable)
        .collect::<Vec<_>>()
        .join("\n")
}
fn command(root: &Path) -> Command {
    let mut command = Command::new("git");
    command
        .arg("--no-pager")
        .arg("--no-optional-locks")
        .arg("--literal-pathspecs")
        .args(["-c", "core.fsmonitor=false", "-c", "diff.external=", "-C"])
        .arg(root)
        .env("GIT_OPTIONAL_LOCKS", "0");
    command
}
fn status(root: &Path, cancelled: &AtomicBool) -> Result<Vec<u8>, String> {
    crate::git::run_git_command(
        command(root).args([
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
            "--ignore-submodules=none",
        ]),
        cancelled,
    )
}
fn files(bytes: &[u8]) -> Result<(Vec<ChangedFile>, usize), String> {
    let mut records = bytes
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty());
    let mut files = Vec::new();
    let mut omitted = 0;
    while let Some(record) = records.next() {
        if record.len() < 4 || record[2] != b' ' {
            return Err("Malformed Git status record; refresh".into());
        }
        let x = record[0];
        let y = record[1];
        let previous = if matches!(x, b'R' | b'C') || matches!(y, b'R' | b'C') {
            Some(
                records
                    .next()
                    .ok_or("Incomplete Git rename record")?
                    .to_vec(),
            )
        } else {
            None
        };
        let new = x == b'?' && y == b'?';
        let conflict = x == b'U' || y == b'U' || matches!((x, y), (b'A', b'A') | (b'D', b'D'));
        let staged = !new && x != b' ';
        let unstaged = !new && y != b' ';
        let state = if new {
            "new file".into()
        } else if conflict {
            "conflict (unmerged index)".into()
        } else {
            let mut states = Vec::new();
            if staged {
                states.push("staged");
            }
            if unstaged {
                states.push("unstaged");
            }
            if x == b'R' || y == b'R' {
                states.push("rename");
            }
            if x == b'D' || y == b'D' {
                states.push("deletion");
            }
            if x == b'A' {
                states.push("added");
            }
            if x == b'T' || y == b'T' {
                states.push("type change");
            }
            states.join(" + ")
        };
        if files.len() < FILE_LIMIT {
            files.push(ChangedFile {
                path: record[3..].to_vec(),
                previous,
                state,
                staged,
                unstaged,
                new,
                conflict,
            });
        } else {
            omitted += 1;
        }
    }
    Ok((files, omitted))
}
fn new_content(root: &Path, path: &[u8]) -> Result<String, String> {
    let path = root.join(OsStr::from_bytes(path));
    // NOFOLLOW refuses symlinks; NONBLOCK prevents FIFOs/devices from waiting at open.
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|error| {
            format!("New/current file unavailable (missing, symlink or unsupported): {error}")
        })?;
    let metadata = file.metadata().map_err(|error| error.to_string())?;
    if !metadata.is_file() {
        return Err("Unsupported non-regular file; content not read".into());
    }
    if metadata.len() > CONTENT_LIMIT as u64 {
        return Err("Oversized file: content exceeds 64 KiB; inspect with your own tools".into());
    }
    let mut bytes = Vec::new();
    file.take(CONTENT_LIMIT as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() > CONTENT_LIMIT {
        return Err("Oversized file: changed beyond 64 KiB while reading; refresh".into());
    }
    if bytes.contains(&0) {
        return Ok("Binary file: text content unavailable".into());
    }
    Ok(content_printable(&bytes))
}
fn patch(root: &Path, file: &ChangedFile, staged: bool, cancelled: &AtomicBool) -> String {
    let mut cmd = command(root);
    cmd.args([
        "diff",
        "--no-ext-diff",
        "--no-textconv",
        "--no-color",
        "--find-renames",
        "--submodule=short",
        "--ignore-submodules=none",
    ]);
    if staged {
        cmd.arg("--cached");
    }
    cmd.arg("--").arg(OsStr::from_bytes(&file.path));
    if let Some(previous) = &file.previous {
        cmd.arg(OsStr::from_bytes(previous));
    }
    match crate::git::run_git_command(&mut cmd, cancelled) {
        Ok(bytes) if bytes.len() <= CONTENT_LIMIT => {
            let text = content_printable(&bytes);
            if text.is_empty() {
                "No text patch available (submodule, missing file or concurrent change); refresh. Submodule nested files require inspection inside that repository.".into()
            } else if text.contains("Subproject commit") {
                format!(
                    "Submodule: nested files are outside this comparison; inspect inside the submodule.\n{text}"
                )
            } else {
                text
            }
        }
        Ok(_) => "Oversized patch: exceeds 64 KiB; inspect with your own tools".into(),
        Err(error) => format!("Patch unavailable/oversized: {}", printable(&error)),
    }
}
fn inspect(
    root: &Path,
    workspace: &str,
    request: &str,
    selected: Option<&[u8]>,
    cancelled: &AtomicBool,
) -> Result<Snapshot, String> {
    let checkout = crate::git::resolve_checkout(root, cancelled)?;
    let before = status(&checkout.root, cancelled)?;
    let (files, omitted) = files(&before)?;
    let selected = selected
        .and_then(|path| files.iter().find(|file| file.path == path))
        .or_else(|| files.first());
    let content = match selected {
        None => "No working changes.".into(),
        Some(file) if file.new => format!(
            "New file (not in index)\n{}",
            new_content(&checkout.root, &file.path).unwrap_or_else(|error| error)
        ),
        Some(file) if file.conflict => format!(
            "Conflict: unmerged index; resolve explicitly with your Git tools. Current file below may contain conflict markers.\n{}",
            new_content(&checkout.root, &file.path).unwrap_or_else(|error| error)
        ),
        Some(file) => {
            let mut text = String::new();
            if file.staged {
                text.push_str("Staged · HEAD to index\n");
                text.push_str(&patch(&checkout.root, file, true, cancelled));
            }
            if file.unstaged {
                text.push_str("\nUnstaged · index to working files\n");
                text.push_str(&patch(&checkout.root, file, false, cancelled));
            }
            text
        }
    };
    let changed_during_capture = status(&checkout.root, cancelled)? != before;
    let selected = selected.map(|file| file.path.clone());
    Ok(Snapshot {
        workspace: workspace.into(),
        request: request.into(),
        checkout,
        files,
        omitted,
        selected,
        content,
        changed_during_capture,
        captured_millis: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
    })
}

pub struct Job {
    worker: thread::JoinHandle<()>,
    cancelled: Arc<AtomicBool>,
    pub result: Receiver<Response>,
}
impl Job {
    pub fn start(
        checkout: crate::git::CheckoutIdentity,
        workspace: String,
        request: String,
        selected: Option<Vec<u8>>,
    ) -> std::io::Result<Self> {
        let cancelled = Arc::new(AtomicBool::new(false));
        let cancellation = cancelled.clone();
        let (sender, result) = mpsc::sync_channel(1);
        let worker = thread::Builder::new()
            .name("working-diff".into())
            .spawn(move || {
                let result = inspect(
                    &checkout.root,
                    &workspace,
                    &request,
                    selected.as_deref(),
                    &cancellation,
                )
                .and_then(|snapshot| {
                    if snapshot.checkout != checkout {
                        Err("Checkout identity changed; re-adopt before inspection".into())
                    } else {
                        Ok(snapshot)
                    }
                });
                let _ = sender.try_send(Response {
                    workspace,
                    request,
                    result,
                });
            })?;
        Ok(Self {
            worker,
            cancelled,
            result,
        })
    }
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }
    pub fn is_finished(&self) -> bool {
        self.worker.is_finished()
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancel();
    }
}

pub struct View {
    pub workspace: String,
    pub name: String,
    pub request: String,
    pub snapshot: Option<Snapshot>,
    pub loading: bool,
    pub error: Option<String>,
    pub scroll: u16,
    pub horizontal: u16,
    pub selected: Option<Vec<u8>>,
}
impl View {
    pub fn new(workspace: String, name: String) -> Self {
        Self {
            workspace,
            name,
            request: String::new(),
            snapshot: None,
            loading: false,
            error: None,
            scroll: 0,
            horizontal: 0,
            selected: None,
        }
    }
    pub fn refresh(&mut self) -> crate::events::Request {
        self.request = uuid::Uuid::new_v4().to_string();
        self.loading = true;
        self.error = None;
        crate::events::Request::InspectWorking {
            request: self.request.clone(),
            workspace: self.workspace.clone(),
            selected: self.selected.clone(),
        }
    }
    pub fn observe(&mut self, response: Response) {
        if !self.loading || response.workspace != self.workspace || response.request != self.request
        {
            return;
        }
        self.loading = false;
        match response.result {
            Err(error) => self.error = Some(printable(&error)),
            Ok(snapshot) => {
                if self.selected == snapshot.selected {
                    if let Some(old) = &self.snapshot {
                        // Preserve a visible context line where it survives refreshed edits.
                        if let Some(anchor) = old.content.lines().nth(self.scroll as usize)
                            && !anchor.is_empty()
                            && let Some(line) =
                                snapshot.content.lines().position(|line| line == anchor)
                        {
                            self.scroll = line.min(u16::MAX as usize) as u16;
                        }
                    }
                } else {
                    self.scroll = 0;
                    self.horizontal = 0;
                }
                self.scroll = self.scroll.min(
                    snapshot
                        .content
                        .lines()
                        .count()
                        .saturating_sub(1)
                        .min(u16::MAX as usize) as u16,
                );
                self.selected = snapshot.selected.clone();
                self.snapshot = Some(snapshot);
            }
        }
    }
    pub fn navigate(&mut self, next: bool) -> Option<crate::events::Request> {
        if self.loading {
            return None;
        }
        let files = &self.snapshot.as_ref()?.files;
        let index = files
            .iter()
            .position(|file| Some(&file.path) == self.selected.as_ref())
            .unwrap_or(0);
        let next = if next {
            (index + 1).min(files.len().saturating_sub(1))
        } else {
            index.saturating_sub(1)
        };
        let path = &files.get(next)?.path;
        if Some(path) == self.selected.as_ref() {
            return None;
        }
        self.selected = Some(path.clone());
        self.scroll = 0;
        self.horizontal = 0;
        Some(self.refresh())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, process::Command};
    fn git(root: &std::path::Path, args: &[&str]) {
        let output = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    #[test]
    fn inspection_disables_repository_diff_textconv_and_fsmonitor_programs_and_labels_submodules() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        git(root, &["init", "-q"]);
        git(root, &["config", "user.name", "Test"]);
        git(root, &["config", "user.email", "test@example.com"]);
        fs::write(root.join("tracked"), "original\n").unwrap();
        fs::write(root.join(".gitattributes"), "tracked diff=untrusted\n").unwrap();
        git(root, &["add", "."]);
        git(root, &["commit", "-qm", "initial"]);
        let program = root.join("program");
        fs::write(
            &program,
            "#!/bin/sh\n/bin/mkdir \"${0}.called\"\n/bin/cat \"$1\"\n",
        )
        .unwrap();
        fs::set_permissions(&program, fs::Permissions::from_mode(0o700)).unwrap();
        for setting in [
            "diff.external",
            "diff.untrusted.command",
            "diff.untrusted.textconv",
            "core.fsmonitor",
        ] {
            git(root, &["config", setting, program.to_str().unwrap()]);
        }
        fs::write(root.join("tracked"), "changed\n").unwrap();
        let snapshot = inspect(root, "w", "r", Some(b"tracked"), &AtomicBool::new(false)).unwrap();
        assert!(snapshot.content.contains("+changed"));
        assert!(
            !root.join("program.called").exists(),
            "inspection ran configured repository code"
        );
        git(root, &["config", "--unset", "core.fsmonitor"]);
        let nested = tempfile::tempdir().unwrap();
        git(nested.path(), &["init", "-q"]);
        git(nested.path(), &["config", "user.name", "Test"]);
        git(nested.path(), &["config", "user.email", "test@example.com"]);
        fs::write(nested.path().join("nested"), "nested base\n").unwrap();
        git(nested.path(), &["add", "."]);
        git(nested.path(), &["commit", "-qm", "nested initial"]);
        git(
            root,
            &[
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "add",
                "-q",
                nested.path().to_str().unwrap(),
                "module",
            ],
        );
        git(root, &["add", "."]);
        git(root, &["commit", "-qm", "submodule"]);
        fs::write(root.join("module/nested"), "nested dirty\n").unwrap();
        let snapshot = inspect(root, "w", "r", Some(b"module"), &AtomicBool::new(false)).unwrap();
        assert!(
            snapshot.content.contains("Submodule"),
            "{}",
            snapshot.content
        );
        assert!(snapshot.content.contains("nested files"));
    }
    #[test]
    fn dashboard_working_files_refresh_safely_and_preserve_context_at_both_sizes() {
        use crate::{
            app::App,
            events::{Effect, Observation, Request},
            ui::Ui,
        };
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        use ratatui::{Terminal, backend::TestBackend};
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        git(root, &["init", "-q"]);
        fs::write(root.join("alpha"), "first content\n").unwrap();
        fs::write(root.join("beta"), "second content\n").unwrap();
        let mut app = App::new(root.into());
        app.workspace_selected = Some("w".into());
        app.session
            .registry
            .workspaces
            .push(crate::registry::Workspace {
                id: "w".into(),
                task: "t".into(),
                legacy_tasks: vec![],
                name: "coding".into(),
                project: None,
                path: None,
                checkout: None,
                sessions: vec![],
            });
        let key = |code| KeyEvent::new(code, KeyModifiers::NONE);
        let complete = |app: &mut App, effect| {
            let Some(Effect::Session(Request::InspectWorking {
                request,
                workspace,
                selected,
            })) = effect
            else {
                panic!("comparison request missing")
            };
            let snapshot = inspect(
                root,
                &workspace,
                &request,
                selected.as_deref(),
                &AtomicBool::new(false),
            )
            .unwrap();
            app.observe_session(Observation {
                working_diff: Some(Response {
                    workspace,
                    request,
                    result: Ok(snapshot),
                }),
                ..app.session.clone()
            });
        };
        let effect = app.input(key(KeyCode::Char('f')));
        complete(&mut app, effect);
        let effect = app.input(key(KeyCode::Char('j')));
        complete(&mut app, effect);
        assert_eq!(
            app.diff.as_ref().unwrap().selected.as_deref(),
            Some(b"beta".as_slice())
        );
        app.input(key(KeyCode::Down));
        fs::write(root.join("beta"), "second content\nagent changed files\n").unwrap();
        let effect = app.input(key(KeyCode::Char('r')));
        let request = app.diff.as_ref().unwrap().request.clone();
        app.observe_session(Observation {
            working_diff: Some(Response {
                workspace: "another-workspace".into(),
                request: request.clone(),
                result: Err("wrong workspace".into()),
            }),
            ..app.session.clone()
        });
        assert!(app.diff.as_ref().unwrap().loading);
        complete(&mut app, effect);
        assert_eq!(app.diff.as_ref().unwrap().scroll, 1);
        app.input(key(KeyCode::Home));
        for (cols, rows) in [(160, 40), (60, 18)] {
            let mut terminal = Terminal::new(TestBackend::new(cols, rows)).unwrap();
            terminal
                .draw(|frame| Ui::default().draw(frame, &app))
                .unwrap();
            let text = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>();
            assert!(text.contains("Working"), "{text}");
            assert!(text.contains("beta"), "{text}");
            assert!(text.contains("agent changed files"), "{text}");
            assert!(text.contains("Snapshot"), "{text}");
            assert!(text.contains("r refresh"), "{text}");
        }
        assert!(matches!(
            app.input(key(KeyCode::Char('s'))),
            Some(Effect::Session(Request::Stop))
        ));
        app.input(key(KeyCode::Esc));
        let effect = app.input(key(KeyCode::Char('f')));
        complete(&mut app, effect);
        assert_eq!(
            app.diff.as_ref().unwrap().selected.as_deref(),
            Some(b"alpha".as_slice())
        );
    }
    #[test]
    fn working_inspection_labels_renames_deletions_binary_conflicts_and_oversized_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        git(root, &["init", "-q"]);
        git(root, &["config", "user.name", "Test"]);
        git(root, &["config", "user.email", "test@example.com"]);
        fs::write(root.join("old"), "rename me\n").unwrap();
        fs::write(root.join("delete"), "delete me\n").unwrap();
        fs::write(root.join("binary"), [0, 1, 2]).unwrap();
        git(root, &["add", "."]);
        git(root, &["commit", "-qm", "initial"]);
        git(root, &["mv", "old", "renamed"]);
        fs::remove_file(root.join("delete")).unwrap();
        fs::write(root.join("binary"), [0, 3, 4]).unwrap();
        fs::write(root.join("large"), vec![b'x'; 70000]).unwrap();
        let snapshot = inspect(root, "w", "r", Some(b"renamed"), &AtomicBool::new(false)).unwrap();
        let renamed = snapshot
            .files
            .iter()
            .find(|file| file.path == b"renamed")
            .unwrap();
        assert_eq!(renamed.previous.as_deref(), Some(b"old".as_slice()));
        assert!(renamed.state.contains("rename"));
        assert!(snapshot.content.contains("rename from old"));
        assert!(
            snapshot
                .files
                .iter()
                .any(|file| file.path == b"delete" && file.state.contains("deletion"))
        );
        let binary = inspect(root, "w", "r", Some(b"binary"), &AtomicBool::new(false)).unwrap();
        assert!(binary.content.contains("Binary files"));
        let large = inspect(root, "w", "r", Some(b"large"), &AtomicBool::new(false)).unwrap();
        assert!(large.content.contains("Oversized"));
        // A real unmerged index, independent of porcelain parser implementation.
        git(root, &["reset", "--hard", "HEAD"]);
        git(root, &["checkout", "-qb", "other"]);
        fs::write(root.join("old"), "other\n").unwrap();
        git(root, &["commit", "-qam", "other"]);
        git(root, &["checkout", "-q", "-"]);
        fs::write(root.join("old"), "ours\n").unwrap();
        git(root, &["commit", "-qam", "ours"]);
        assert!(
            !Command::new("git")
                .arg("-C")
                .arg(root)
                .args(["merge", "other"])
                .output()
                .unwrap()
                .status
                .success()
        );
        let conflict = inspect(root, "w", "r", Some(b"old"), &AtomicBool::new(false)).unwrap();
        assert!(
            conflict
                .files
                .iter()
                .any(|file| file.path == b"old" && file.conflict)
        );
        assert!(conflict.content.contains("<<<<<<< HEAD"));
    }
    #[test]
    fn working_inspection_preserves_index_and_includes_overlapping_and_new_changes() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        git(root, &["init", "-q"]);
        git(root, &["config", "user.name", "Test"]);
        git(root, &["config", "user.email", "test@example.com"]);
        fs::write(root.join("tracked"), "original\n").unwrap();
        git(root, &["add", "."]);
        git(root, &["commit", "-qm", "initial"]);
        fs::write(root.join("tracked"), "staged\n").unwrap();
        git(root, &["add", "tracked"]);
        fs::write(root.join("tracked"), "unstaged\n").unwrap();
        fs::write(root.join("new\nfile"), "new text\n\x1b[31m unsafe\n").unwrap();
        let index = fs::read(root.join(".git/index")).unwrap();
        let snapshot = inspect(
            root,
            "workspace",
            "request",
            Some(b"tracked"),
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(snapshot.files.len(), 2);
        assert!(
            snapshot
                .files
                .iter()
                .any(|file| file.path == b"new\nfile" && file.state == "new file")
        );
        assert!(snapshot.content.contains("Staged"));
        assert!(snapshot.content.contains("+staged"));
        assert!(snapshot.content.contains("Unstaged"));
        assert!(snapshot.content.contains("+unstaged"));
        assert_eq!(index, fs::read(root.join(".git/index")).unwrap());
        let new = inspect(
            root,
            "workspace",
            "next",
            Some(b"new\nfile"),
            &AtomicBool::new(false),
        )
        .unwrap();
        assert!(new.content.contains("new text"));
        assert!(!new.content.contains('\x1b'));
    }
}
