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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Base {
    pub reference: Vec<u8>,
    pub commit: String,
}

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
    #[serde(default)]
    pub comparison: String,
    #[serde(default)]
    pub save_base: Option<Base>,
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
            if x == b'A' || y == b'A' {
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
    files.sort_by(|a, b| a.path.cmp(&b.path));
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
        comparison: "Working".into(),
        save_base: None,
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

fn revision(root: &Path, reference: &[u8], cancelled: &AtomicBool) -> Result<String, String> {
    let mut argument = reference.to_vec();
    argument.extend_from_slice(b"^{commit}");
    let bytes = crate::git::run_git_command(
        command(root)
            .args(["rev-parse", "--verify", "--end-of-options"])
            .arg(OsStr::from_bytes(&argument)),
        cancelled,
    )
    .map_err(|error| {
        format!(
            "Local comparison revision unavailable; press b to select a full local ref: {error}"
        )
    })?;
    let commit = String::from_utf8(bytes).map_err(|_| "Invalid Git commit identity")?;
    let commit = commit.trim();
    if !matches!(commit.len(), 40 | 64) || !commit.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Invalid Git commit identity".into());
    }
    Ok(commit.into())
}

fn committed_files(
    root: &Path,
    ancestor: &str,
    head: &str,
    cancelled: &AtomicBool,
) -> Result<Vec<ChangedFile>, String> {
    let bytes = crate::git::run_git_command(
        command(root).args([
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--name-status",
            "-z",
            "--find-renames",
            ancestor,
            head,
            "--",
        ]),
        cancelled,
    )?;
    let mut records = bytes.split(|b| *b == 0).filter(|r| !r.is_empty());
    let mut changed = Vec::new();
    while let Some(state) = records.next() {
        let first = records
            .next()
            .ok_or("Incomplete committed file record")?
            .to_vec();
        let rename = matches!(state.first(), Some(b'R' | b'C'));
        let (path, previous) = if rename {
            (
                records
                    .next()
                    .ok_or("Incomplete committed rename record")?
                    .to_vec(),
                Some(first),
            )
        } else {
            (first, None)
        };
        changed.push(ChangedFile {
            path,
            previous,
            state: format!(
                "committed{}",
                match state.first() {
                    Some(b'R') => " rename",
                    Some(b'D') => " deletion",
                    Some(b'A') => " added",
                    _ => "",
                }
            ),
            staged: false,
            unstaged: false,
            new: false,
            conflict: false,
        });
    }
    Ok(changed)
}

fn whole_patch(root: &Path, ancestor: &str, file: &ChangedFile, cancelled: &AtomicBool) -> String {
    let mut cmd = command(root);
    cmd.args([
        "diff",
        "--no-ext-diff",
        "--no-textconv",
        "--no-color",
        "--find-renames",
        "--submodule=short",
        "--ignore-submodules=none",
        ancestor,
        "--",
    ])
    .arg(OsStr::from_bytes(&file.path));
    if let Some(previous) = &file.previous {
        cmd.arg(OsStr::from_bytes(previous));
    }
    match crate::git::run_git_command(&mut cmd, cancelled) {
        Ok(bytes) if bytes.len() <= CONTENT_LIMIT => {
            if bytes.is_empty() {
                "No net tracked change against common ancestor (current edits may undo committed work).".into()
            } else if bytes
                .windows(b"Subproject commit".len())
                .any(|w| w == b"Subproject commit")
            {
                format!(
                    "Submodule: nested files are outside this comparison; inspect inside the submodule.\n{}",
                    content_printable(&bytes)
                )
            } else {
                content_printable(&bytes)
            }
        }
        Ok(_) => "Oversized patch: exceeds 64 KiB; inspect with your own tools".into(),
        Err(error) => format!("Patch unavailable/oversized: {}", printable(&error)),
    }
}

fn inspect_whole(
    root: &Path,
    workspace: &str,
    request: &str,
    selected: Option<&[u8]>,
    retained: Option<&Base>,
    selected_ref: Option<&[u8]>,
    cancelled: &AtomicBool,
) -> Result<Snapshot, String> {
    let reference = selected_ref.or(retained.map(|b| b.reference.as_slice()))
        .ok_or("Whole workspace needs a selected local base. Press b and enter refs/heads/<branch> or refs/remotes/<remote>/<branch>. Inspection never fetches.")?;
    if reference.len() > 32768
        || reference.contains(&0)
        || !(reference.starts_with(b"refs/heads/") || reference.starts_with(b"refs/remotes/"))
    {
        return Err("Select an unambiguous full local ref: refs/heads/<branch> or refs/remotes/<remote>/<branch>; archive/review refs are not comparison bases".into());
    }
    let checkout = crate::git::resolve_checkout(root, cancelled)?;
    let head = revision(&checkout.root, b"HEAD", cancelled)?;
    let base_commit = revision(&checkout.root, reference, cancelled)?;
    let ancestors = crate::git::run_git_command(command(&checkout.root).args(["merge-base", "--all", &head, &base_commit]), cancelled)
        .map_err(|error| format!("No usable common ancestor with selected local base; histories may be unrelated. Press b to select a related base. {error}"))?;
    let ancestors = String::from_utf8(ancestors).map_err(|_| "Invalid common ancestor")?;
    let ancestors = ancestors.split_whitespace().collect::<Vec<_>>();
    if ancestors.len() != 1 {
        return Err("Ambiguous common ancestors; select another local base with b".into());
    }
    let ancestor = ancestors[0];
    let branch = crate::git::run_git_command(
        command(&checkout.root).args(["symbolic-ref", "--quiet", "HEAD"]),
        cancelled,
    )
    .map(|b| printable(String::from_utf8_lossy(&b).trim()))
    .unwrap_or_else(|_| "detached HEAD".into());
    let before = status(&checkout.root, cancelled)?;
    let (working, mut omitted) = files(&before)?;
    let mut combined = committed_files(&checkout.root, ancestor, &head, cancelled)?;
    for mut file in working {
        if let Some(old) = combined.iter_mut().find(|old| old.path == file.path) {
            file.state = format!("{} + {}", old.state, file.state);
            if file.previous.is_none() {
                file.previous.clone_from(&old.previous);
            }
            *old = file;
        } else {
            combined.push(file);
        }
    }
    combined.sort_by(|a, b| a.path.cmp(&b.path));
    omitted += combined.len().saturating_sub(FILE_LIMIT);
    combined.truncate(FILE_LIMIT);
    let chosen = selected
        .and_then(|path| combined.iter().find(|f| f.path == path))
        .or_else(|| combined.first());
    let base = Base {
        reference: reference.to_vec(),
        commit: base_commit.clone(),
    };
    let comparison = format!(
        "Whole workspace\nBranch {branch}\nHEAD {head}\nLocal base {} {base_commit}\nCommon ancestor {ancestor}{}",
        printable(&String::from_utf8_lossy(reference)),
        retained
            .filter(|old| old.reference == reference && old.commit != base_commit)
            .map(|old| format!("\nBase moved since selection {}", old.commit))
            .unwrap_or_default()
    );
    let mut content = format!("{comparison}\n\n");
    match chosen {
        None => content.push_str("No workspace changes since common ancestor."),
        Some(file) if file.new => content.push_str(&format!(
            "New file (not in index)\n{}",
            new_content(&checkout.root, &file.path).unwrap_or_else(|e| e)
        )),
        Some(file) if file.conflict => content.push_str(&format!(
            "Conflict: unmerged index; comparison is unresolved. Current file:\n{}",
            new_content(&checkout.root, &file.path).unwrap_or_else(|e| e)
        )),
        Some(file) => {
            content
                .push_str("Common ancestor to current working files (commits and current edits)\n");
            content.push_str(&whole_patch(&checkout.root, ancestor, file, cancelled));
            if file.staged {
                content.push_str("\nStaged · HEAD to index\n");
                content.push_str(&patch(&checkout.root, file, true, cancelled));
            }
            if file.unstaged {
                content.push_str("\nUnstaged · index to working files\n");
                content.push_str(&patch(&checkout.root, file, false, cancelled));
            }
        }
    }
    let changed_during_capture = status(&checkout.root, cancelled)? != before
        || revision(&checkout.root, b"HEAD", cancelled)? != head
        || revision(&checkout.root, reference, cancelled)? != base_commit
        || crate::git::run_git_command(
            command(&checkout.root).args(["symbolic-ref", "--quiet", "HEAD"]),
            cancelled,
        )
        .map(|b| printable(String::from_utf8_lossy(&b).trim()))
        .unwrap_or_else(|_| "detached HEAD".into())
            != branch;
    let selected = chosen.map(|f| f.path.clone());
    Ok(Snapshot {
        workspace: workspace.into(),
        request: request.into(),
        captured_millis: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
        checkout,
        files: combined,
        omitted,
        selected,
        content,
        changed_during_capture,
        comparison,
        save_base: selected_ref.map(|_| base),
    })
}

pub struct Job {
    worker: thread::JoinHandle<()>,
    cancelled: Arc<AtomicBool>,
    pub result: Receiver<Response>,
}
impl Job {
    pub fn start_comparison(
        checkout: crate::git::CheckoutIdentity,
        workspace: String,
        request: String,
        selected: Option<Vec<u8>>,
        whole: bool,
        retained: Option<Base>,
        selected_ref: Option<Vec<u8>>,
    ) -> std::io::Result<Self> {
        let cancelled = Arc::new(AtomicBool::new(false));
        let cancellation = cancelled.clone();
        let (sender, result) = mpsc::sync_channel(1);
        let worker = thread::Builder::new()
            .name("working-diff".into())
            .spawn(move || {
                let result = if whole {
                    inspect_whole(
                        &checkout.root,
                        &workspace,
                        &request,
                        selected.as_deref(),
                        retained.as_ref(),
                        selected_ref.as_deref(),
                        &cancellation,
                    )
                } else {
                    inspect(
                        &checkout.root,
                        &workspace,
                        &request,
                        selected.as_deref(),
                        &cancellation,
                    )
                }
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
    pub whole: bool,
    pub base_entry: Option<String>,
    pub base_selection: Option<Vec<u8>>,
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
            whole: false,
            base_entry: None,
            base_selection: None,
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
            whole: self.whole,
            base: self.base_selection.clone(),
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
                self.base_selection = None;
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
    fn whole_workspace_includes_commits_and_overlapping_current_edits_without_index_changes() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        git(root, &["init", "-q"]);
        git(root, &["config", "user.name", "Test"]);
        git(root, &["config", "user.email", "test@example.com"]);
        fs::write(root.join("tracked"), "original\n").unwrap();
        git(root, &["add", "."]);
        git(root, &["commit", "-qm", "base"]);
        git(root, &["branch", "base"]);
        fs::write(root.join("tracked"), "committed\n").unwrap();
        git(root, &["commit", "-qam", "work"]);
        fs::write(root.join("tracked"), "staged\n").unwrap();
        git(root, &["add", "."]);
        fs::write(root.join("tracked"), "current\n").unwrap();
        fs::write(root.join("new"), "new text\n").unwrap();
        let index = fs::read(root.join(".git/index")).unwrap();
        let snapshot = inspect_whole(
            root,
            "w",
            "r",
            Some(b"tracked"),
            None,
            Some(b"refs/heads/base"),
            &AtomicBool::new(false),
        )
        .unwrap();
        assert!(snapshot.content.contains("-original"));
        assert!(snapshot.content.contains("+current"));
        assert!(snapshot.files.iter().any(|f| f.path == b"new" && f.new));
        assert!(snapshot.comparison.contains("Whole workspace"));
        assert!(snapshot.comparison.contains("HEAD"));
        assert_eq!(index, fs::read(root.join(".git/index")).unwrap());
    }
    #[test]
    fn whole_refresh_labels_moved_bases_branches_and_ref_failures_without_guessing() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        git(root, &["init", "-q", "-b", "main"]);
        git(root, &["config", "user.name", "Test"]);
        git(root, &["config", "user.email", "test@example.com"]);
        fs::write(root.join("file"), "base\n").unwrap();
        git(root, &["add", "."]);
        git(root, &["commit", "-qm", "base"]);
        git(root, &["branch", "base"]);
        fs::write(root.join("file"), "committed\n").unwrap();
        git(root, &["commit", "-qam", "work"]);
        let first = inspect_whole(
            root,
            "w",
            "first",
            Some(b"file"),
            None,
            Some(b"refs/heads/base"),
            &AtomicBool::new(false),
        )
        .unwrap();
        let retained = first.save_base.unwrap();
        assert!(first.content.contains("-base"));
        git(root, &["branch", "-f", "base", "HEAD"]);
        let moved = inspect_whole(
            root,
            "w",
            "moved",
            None,
            Some(&retained),
            None,
            &AtomicBool::new(false),
        )
        .unwrap();
        assert!(
            moved
                .comparison
                .contains(&format!("Base moved since selection {}", retained.commit))
        );
        assert!(moved.files.is_empty());
        git(root, &["checkout", "-qb", "switched"]);
        let switched = inspect_whole(
            root,
            "w",
            "switch",
            None,
            Some(&retained),
            None,
            &AtomicBool::new(false),
        )
        .unwrap();
        assert!(switched.comparison.contains("Branch refs/heads/switched"));
        git(root, &["branch", "-D", "base"]);
        let missing = inspect_whole(
            root,
            "w",
            "missing",
            None,
            Some(&retained),
            None,
            &AtomicBool::new(false),
        )
        .unwrap_err();
        assert!(missing.contains("revision unavailable"));
        for reference in [
            None,
            Some(b"main".as_slice()),
            Some(b"refs/rat/archive/recovery".as_slice()),
        ] {
            assert!(
                inspect_whole(
                    root,
                    "w",
                    "invalid",
                    None,
                    None,
                    reference,
                    &AtomicBool::new(false)
                )
                .is_err()
            );
        }
        git(root, &["checkout", "--orphan", "unrelated"]);
        git(root, &["commit", "-qm", "unrelated"]);
        assert!(
            inspect_whole(
                root,
                "w",
                "unrelated",
                None,
                None,
                Some(b"refs/heads/main"),
                &AtomicBool::new(false)
            )
            .unwrap_err()
            .contains("unrelated")
        );
    }

    #[test]
    fn whole_rejects_criss_cross_multiple_common_ancestors() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        git(root, &["init", "-q", "-b", "main"]);
        git(root, &["config", "user.name", "Test"]);
        git(root, &["config", "user.email", "test@example.com"]);
        git(root, &["commit", "--allow-empty", "-qm", "initial"]);
        let output = |args: &[&str]| {
            let result = Command::new("git")
                .arg("-C")
                .arg(root)
                .args(args)
                .output()
                .unwrap();
            assert!(result.status.success());
            String::from_utf8(result.stdout).unwrap().trim().to_owned()
        };
        let original = output(&["rev-parse", "HEAD"]);
        let tree = output(&["rev-parse", "HEAD^{tree}"]);
        let left = output(&["commit-tree", &tree, "-p", &original, "-m", "left"]);
        let right = output(&["commit-tree", &tree, "-p", &original, "-m", "right"]);
        let merge_left = output(&[
            "commit-tree",
            &tree,
            "-p",
            &left,
            "-p",
            &right,
            "-m",
            "merge left",
        ]);
        let merge_right = output(&[
            "commit-tree",
            &tree,
            "-p",
            &right,
            "-p",
            &left,
            "-m",
            "merge right",
        ]);
        git(root, &["update-ref", "refs/heads/main", &merge_left]);
        git(root, &["update-ref", "refs/heads/base", &merge_right]);
        let error = inspect_whole(
            root,
            "w",
            "ambiguous",
            None,
            None,
            Some(b"refs/heads/base"),
            &AtomicBool::new(false),
        )
        .unwrap_err();
        assert!(error.contains("Ambiguous common ancestors"));
    }

    #[test]
    fn dashboard_switches_explicitly_selects_base_and_reopens_working_at_both_sizes() {
        use crate::{
            app::App,
            events::{Effect, Observation, Request},
            ui::Ui,
        };
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        use ratatui::{Terminal, backend::TestBackend};
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        git(root, &["init", "-q", "-b", "main"]);
        git(root, &["config", "user.name", "Test"]);
        git(root, &["config", "user.email", "test@example.com"]);
        fs::write(root.join("file"), "base\n").unwrap();
        git(root, &["add", "."]);
        git(root, &["commit", "-qm", "base"]);
        git(root, &["branch", "base"]);
        fs::write(root.join("file"), "committed\n").unwrap();
        git(root, &["commit", "-qam", "work"]);
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
                whole,
                base,
            })) = effect
            else {
                panic!("missing inspection")
            };
            let result = if whole {
                inspect_whole(
                    root,
                    &workspace,
                    &request,
                    selected.as_deref(),
                    None,
                    base.as_deref(),
                    &AtomicBool::new(false),
                )
            } else {
                inspect(
                    root,
                    &workspace,
                    &request,
                    selected.as_deref(),
                    &AtomicBool::new(false),
                )
            };
            app.observe_session(Observation {
                working_diff: Some(Response {
                    workspace,
                    request,
                    result,
                }),
                ..app.session.clone()
            });
        };
        let effect = app.input(key(KeyCode::Char('f')));
        complete(&mut app, effect);
        assert!(!app.diff.as_ref().unwrap().whole);
        let effect = app.input(key(KeyCode::Char('w')));
        complete(&mut app, effect);
        assert!(
            app.diff
                .as_ref()
                .unwrap()
                .error
                .as_ref()
                .unwrap()
                .contains("selected local base")
        );
        app.input(key(KeyCode::Char('b')));
        app.paste("refs/heads/base".into());
        let effect = app.input(key(KeyCode::Enter));
        complete(&mut app, effect);
        assert!(app.diff.as_ref().unwrap().whole);
        assert!(
            app.diff
                .as_ref()
                .unwrap()
                .snapshot
                .as_ref()
                .unwrap()
                .content
                .contains("+committed")
        );
        for (cols, rows) in [(160, 40), (60, 18)] {
            app.diff.as_mut().unwrap().scroll = 0;
            let mut terminal = Terminal::new(TestBackend::new(cols, rows)).unwrap();
            terminal
                .draw(|frame| Ui::default().draw(frame, &app))
                .unwrap();
            let text = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|c| c.symbol())
                .collect::<String>();
            assert!(text.contains("Whole workspace"), "{text}");
            assert!(text.contains("Branch refs/heads/main"), "{text}");
            assert!(
                text.contains("b base") || text.contains("b local base"),
                "{text}"
            );
            assert!(text.contains("Snapshot"), "{text}");
            for _ in 0..12 {
                app.input(key(KeyCode::Down));
            }
            terminal
                .draw(|frame| Ui::default().draw(frame, &app))
                .unwrap();
            let text = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|c| c.symbol())
                .collect::<String>();
            assert!(text.contains("+committed"), "{text}");
        }
        app.input(key(KeyCode::Esc));
        let effect = app.input(key(KeyCode::Char('f')));
        complete(&mut app, effect);
        let view = app.diff.as_ref().unwrap();
        assert!(!view.whole);
        assert!(view.snapshot.as_ref().unwrap().files.is_empty());
    }

    #[test]
    fn whole_preserves_committed_renames_deletions_binary_and_unix_paths() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        git(root, &["init", "-q"]);
        git(root, &["config", "user.name", "Test"]);
        git(root, &["config", "user.email", "test@example.com"]);
        fs::write(root.join("before"), "rename content\n").unwrap();
        fs::write(root.join("deleted"), "deleted content\n").unwrap();
        fs::write(root.join("binary"), [0, 1, 2]).unwrap();
        git(root, &["add", "."]);
        git(root, &["commit", "-qm", "base"]);
        git(root, &["branch", "base"]);
        git(root, &["mv", "before", "after"]);
        git(root, &["rm", "deleted"]);
        fs::write(root.join("binary"), [0, 3, 4]).unwrap();
        git(root, &["commit", "-qam", "workspace changes"]);
        // APFS rejects invalid UTF-8 filenames (EILSEQ); other Unix filesystems
        // additionally exercise non-UTF-8 bytes through the same public snapshot.
        let raw: &[u8] = if cfg!(target_os = "macos") {
            b"new\nfile"
        } else {
            b"new\xff\nfile"
        };
        fs::write(root.join(OsStr::from_bytes(raw)), "raw path content\n").unwrap();
        let index = fs::read(root.join(".git/index")).unwrap();
        for (path, expected) in [
            (b"after".as_slice(), "rename from before"),
            (b"deleted".as_slice(), "-deleted content"),
            (b"binary".as_slice(), "Binary files"),
            (raw, "raw path content"),
        ] {
            let whole = inspect_whole(
                root,
                "w",
                "committed-edge",
                Some(path),
                None,
                Some(b"refs/heads/base"),
                &AtomicBool::new(false),
            )
            .unwrap();
            assert_eq!(whole.selected.as_deref(), Some(path));
            assert!(whole.content.contains(expected), "{}", whole.content);
            assert!(whole.files.iter().any(|f| f.path == raw && f.new));
        }
        assert_eq!(index, fs::read(root.join(".git/index")).unwrap());
    }

    #[test]
    fn oversized_file_lists_remain_bounded_with_an_explicit_omission_count() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        git(root, &["init", "-q"]);
        for index in 0..1030 {
            fs::write(root.join(format!("file-{index:04}")), "text\n").unwrap();
        }
        let snapshot = inspect(root, "w", "r", None, &AtomicBool::new(false)).unwrap();
        assert_eq!(snapshot.files.len(), 1024);
        assert_eq!(snapshot.omitted, 6);
        assert_eq!(snapshot.selected.as_deref(), Some(b"file-0000".as_slice()));
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
        git(root, &["branch", "base"]);
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
        let whole = inspect_whole(
            root,
            "w",
            "whole",
            Some(b"tracked"),
            None,
            Some(b"refs/heads/base"),
            &AtomicBool::new(false),
        )
        .unwrap();
        assert!(whole.content.contains("+changed"));
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
                ..
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
                workspace: "w".into(),
                request: "old-request".into(),
                result: Err("stale request".into()),
            }),
            ..app.session.clone()
        });
        assert!(app.diff.as_ref().unwrap().loading);

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
        git(root, &["branch", "base"]);
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
        for (path, expected) in [
            (b"renamed".as_slice(), "rename from old"),
            (b"delete".as_slice(), "-delete me"),
            (b"binary".as_slice(), "Binary files"),
        ] {
            let whole = inspect_whole(
                root,
                "w",
                "edge",
                Some(path),
                None,
                Some(b"refs/heads/base"),
                &AtomicBool::new(false),
            )
            .unwrap();
            assert!(whole.content.contains(expected), "{}", whole.content);
        }
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
        let whole = inspect_whole(
            root,
            "w",
            "conflict",
            Some(b"old"),
            None,
            Some(b"refs/heads/base"),
            &AtomicBool::new(false),
        )
        .unwrap();
        assert!(whole.content.contains("comparison is unresolved"));
        assert!(whole.content.contains("<<<<<<< HEAD"));
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
        std::os::unix::fs::symlink(root.join("tracked"), root.join("link")).unwrap();
        let link = inspect(
            root,
            "workspace",
            "link-request",
            Some(b"link"),
            &AtomicBool::new(false),
        )
        .unwrap();
        assert!(link.content.contains("unsupported"));
        fs::remove_file(root.join("link")).unwrap();
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
