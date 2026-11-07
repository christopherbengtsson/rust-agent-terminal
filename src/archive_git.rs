//! Bounded Git checkpoint and original-index recovery execution.
use crate::{
    archive::{Phase, Record},
    git::RepositoryMutation,
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::AtomicBool,
    time::Duration,
};

fn git(
    r: &Record,
    args: &[&str],
    index: Option<&Path>,
    cancel: &AtomicBool,
) -> Result<Vec<u8>, String> {
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(&r.checkout.root)
        .args(["-c", "core.fsync=committed", "-c", "core.fsyncMethod=fsync"])
        .args(args);
    if let Some(index) = index {
        cmd.env("GIT_INDEX_FILE", index);
    }
    crate::git::run_git_command_deadline(&mut cmd, cancel, Duration::from_secs(30))
}
fn text(
    r: &Record,
    args: &[&str],
    index: Option<&Path>,
    cancel: &AtomicBool,
) -> Result<String, String> {
    String::from_utf8(git(r, args, index, cancel)?)
        .map(|s| s.trim().into())
        .map_err(|_| "Git object identity was not UTF-8".into())
}
fn same(r: &Record, cancel: &AtomicBool) -> Result<(), String> {
    if crate::git::resolve_checkout(&r.checkout.root, cancel)? != r.checkout {
        return Err("Checkout identity changed; archive refused".into());
    }
    Ok(())
}
fn location(r: &Record) -> PathBuf {
    r.checkout.git_dir.join("rat-archive").join(&r.request)
}
fn index_path(r: &Record) -> PathBuf {
    r.checkout.git_dir.join("index")
}
fn index_hash(r: &Record, cancel: &AtomicBool) -> Result<String, String> {
    match fs::symlink_metadata(index_path(r)) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok("absent-index".into());
        }
        Err(error) => return Err(error.to_string()),
        Ok(_) => {}
    }
    let output = crate::git::run_git_command_deadline(
        Command::new("git")
            .arg("-C")
            .arg(&r.checkout.root)
            .arg("hash-object")
            .arg(index_path(r)),
        cancel,
        Duration::from_secs(2),
    )?;
    String::from_utf8(output)
        .map(|s| s.trim().into())
        .map_err(|_| "Invalid index identity".into())
}
// A missing index is Git's empty index, including valid empty-tree HEADs.
// Materialize that meaning only in our alternate index; never create the real
// index until checkpoint protection and final snapshot verification succeed.
fn copy_real_index(r: &Record, destination: &Path, cancel: &AtomicBool) -> Result<(), String> {
    match fs::copy(index_path(r), destination) {
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            git(r, &["read-tree", "--empty"], Some(destination), cancel).map(|_| ())
        }
        Err(error) => Err(error.to_string()),
    }
}
struct Lock {
    path: PathBuf,
    held: bool,
}
impl Drop for Lock {
    fn drop(&mut self) {
        if self.held {
            let _ = fs::remove_file(&self.path);
        }
    }
}
fn lock_index(r: &Record) -> Result<Lock, String> {
    let p = r.checkout.git_dir.join("index.lock");
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&p)
        .map_err(|e| format!("Index is busy; archive refused: {e}"))?;
    Ok(Lock {
        path: p,
        held: true,
    })
}
fn pin(r: &Record, reference: &str, oid: &str, cancel: &AtomicBool) -> Result<(), String> {
    if let Ok(existing) = text(r, &["rev-parse", "--verify", reference], None, cancel) {
        if existing == oid {
            return Ok(());
        }
        return Err("Recovery reference already identifies other work".into());
    }
    git(r, &["update-ref", reference, oid, ""], None, cancel)?;
    if text(r, &["rev-parse", "--verify", reference], None, cancel)? != oid {
        return Err("Recovery reference verification failed".into());
    }
    Ok(())
}
fn inventory(r: &Record, index: &Path, cancel: &AtomicBool) -> Result<String, String> {
    git(r, &["add", "-A", "--", "."], Some(index), cancel)?;
    text(r, &["write-tree"], Some(index), cancel)
}
fn blockers(r: &Record, cancel: &AtomicBool) -> Result<(), String> {
    for name in [
        "MERGE_HEAD",
        "CHERRY_PICK_HEAD",
        "REVERT_HEAD",
        "rebase-merge",
        "rebase-apply",
        "BISECT_LOG",
    ] {
        if r.checkout.git_dir.join(name).exists() {
            return Err(format!(
                "{name} is present; finish or abort that Git operation before archive"
            ));
        }
    }
    if !git(r, &["ls-files", "--unmerged", "-z"], None, cancel)?.is_empty() {
        return Err("Resolve index conflicts before archive".into());
    }
    let visible = git(
        r,
        &[
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--cached",
            "--name-only",
            "--ita-visible-in-index",
            "-z",
        ],
        None,
        cancel,
    )?;
    let invisible = git(
        r,
        &[
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--cached",
            "--name-only",
            "--ita-invisible-in-index",
            "-z",
        ],
        None,
        cancel,
    )?;
    if visible != invisible {
        return Err("Intent-to-add index entries are unsupported; stage or unstage those paths before archive".into());
    }
    let files = git(r, &["ls-files", "--stage", "-z"], None, cancel)?;
    if files.split(|b| *b == 0).any(|e| e.starts_with(b"160000 ")) {
        return Err("Submodule/gitlink state requires a separate save; archive refused".into());
    }
    if !git(r, &["rev-parse", "--shared-index-path"], None, cancel)?.is_empty() {
        return Err("Split index is unsupported; disable it before archive".into());
    }
    let flags = git(r, &["ls-files", "-v", "-z"], None, cancel)?;
    if flags
        .split(|b| *b == 0)
        .any(|e| !e.is_empty() && (e[0] == b'S' || e[0].is_ascii_lowercase()))
    {
        return Err(
            "Sparse/skip-worktree/assume-unchanged index state is unsupported; archive refused"
                .into(),
        );
    }
    // A nested repository is not a recursive backup. Refuse before staging it.
    let others = git(
        r,
        &["ls-files", "--others", "--exclude-standard", "-z"],
        None,
        cancel,
    )?;
    for name in others.split(|b| *b == 0).filter(|n| !n.is_empty()) {
        use std::os::unix::ffi::OsStrExt;
        let mut path = r.checkout.root.join(std::ffi::OsStr::from_bytes(name));
        while path != r.checkout.root {
            if path.join(".git").exists() {
                return Err("Nested repository requires its own save; archive refused".into());
            }
            if !path.pop() {
                break;
            }
        }
    }
    Ok(())
}
pub(super) fn prepare(mut r: Record, cancel: &AtomicBool) -> Result<Record, String> {
    same(&r, cancel)?;
    let _lease = RepositoryMutation::acquire(&r.checkout.common_dir, cancel)?;
    let _lock = lock_index(&r)?;
    blockers(&r, cancel)?;
    r.before = Some(text(
        &r,
        &["rev-parse", "--verify", "HEAD^{commit}"],
        None,
        cancel,
    )?);
    r.branch = Some(
        text(&r, &["symbolic-ref", "--quiet", "HEAD"], None, cancel)
            .map_err(|_| "Archive requires a workspace branch; detached HEAD retained unchanged")?,
    );
    let dir = location(&r);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let original = dir.join("original-index");
    let working = dir.join("working-index");
    copy_real_index(&r, &original, cancel)?;
    fs::File::open(&original)
        .and_then(|f| f.sync_all())
        .map_err(|e| e.to_string())?;
    r.index_hash = Some(index_hash(&r, cancel)?);
    r.index_tree = Some(text(&r, &["write-tree"], Some(&original), cancel)?);
    fs::copy(&original, &working).map_err(|e| e.to_string())?;
    r.tree = Some(inventory(&r, &working, cancel)?);
    // add may introduce gitlinks from nested directories that ls-files did not descend.
    if git(&r, &["ls-files", "--stage", "-z"], Some(&working), cancel)?
        .split(|b| *b == 0)
        .any(|e| e.starts_with(b"160000 "))
    {
        return Err("Nested repository would save only a gitlink; archive refused".into());
    }
    r.excluded = git(
        &r,
        &[
            "ls-files",
            "--others",
            "--ignored",
            "--exclude-standard",
            "-z",
        ],
        None,
        cancel,
    )?
    .split(|b| *b == 0)
    .filter(|n| !n.is_empty())
    .map(Vec::from)
    .collect();
    fs::File::open(&working)
        .and_then(|f| f.sync_all())
        .map_err(|e| e.to_string())?;
    fs::File::open(&dir)
        .and_then(|f| f.sync_all())
        .map_err(|e| e.to_string())?;
    pin(
        &r,
        &format!("{}-index", r.reference),
        r.index_tree.as_deref().ok_or("Missing staged tree")?,
        cancel,
    )?;
    pin(
        &r,
        &format!("{}-prepared", r.reference),
        r.tree.as_deref().ok_or("Missing prepared tree")?,
        cancel,
    )?;
    fs::File::open(dir.parent().ok_or("Archive metadata parent unavailable")?)
        .and_then(|f| f.sync_all())
        .map_err(|e| e.to_string())?;
    fs::File::open(&r.checkout.git_dir)
        .and_then(|f| f.sync_all())
        .map_err(|e| e.to_string())?;
    r.phase = Phase::Prepared;
    r.message = "Snapshot prepared; waiting for durable checkpoint authorization".into();
    Ok(r)
}
pub(super) fn save(mut r: Record, cancel: &AtomicBool) -> Result<Record, String> {
    same(&r, cancel)?;
    let _lease = RepositoryMutation::acquire(&r.checkout.common_dir, cancel)?;
    let mut lock = lock_index(&r)?;
    blockers(&r, cancel)?;
    let before = r.before.as_deref().ok_or("Missing prepared HEAD")?;
    let expected = r.tree.as_deref().ok_or("Missing prepared tree")?;
    let working = location(&r).join("working-index");
    let verify = location(&r).join("verify-index");
    let committing = location(&r).join("commit-index");
    let message = format!(
        "Archive checkpoint\n\nWorkspace: {}\nArchive-Request: {}",
        r.workspace, r.request
    );
    let head = text(&r, &["rev-parse", "HEAD"], None, cancel)?;
    let already_saved = head != before
        && text(&r, &["show", "-s", "--format=%P", &head], None, cancel)? == before
        && text(&r, &["show", "-s", "--format=%T", &head], None, cancel)? == expected
        && text(&r, &["show", "-s", "--format=%B", &head], None, cancel)? == message;
    if head != before && !already_saved {
        return Err("HEAD changed since archive preparation; no further mutation performed".into());
    }
    if text(&r, &["symbolic-ref", "--quiet", "HEAD"], None, cancel)?.as_str()
        != r.branch.as_deref().ok_or("Missing prepared branch")?
    {
        return Err("Workspace branch changed; archive refused".into());
    }
    let current_index = location(&r).join("current-index");
    copy_real_index(&r, &current_index, cancel)?;
    let admitted_index_hash = index_hash(&r, cancel)?;
    let original_index =
        admitted_index_hash == *r.index_hash.as_ref().ok_or("Missing prepared index")?;
    let pinned_head = text(&r, &["rev-parse", "--verify", &r.reference], None, cancel)
        .ok()
        .is_some_and(|oid| oid == head);
    if !original_index
        && !((already_saved || pinned_head)
            && text(&r, &["write-tree"], Some(&current_index), cancel)? == expected)
    {
        return Err(
            "Index changed since archive preparation; staged work retained, archive refused".into(),
        );
    }
    fs::copy(&working, &verify).map_err(|e| e.to_string())?;
    if inventory(&r, &verify, cancel)? != expected {
        return Err("Files changed since archive preparation; archive refused".into());
    }
    // Git refs can pin trees directly. This preserves staged-only blobs independently
    // of transcripts and of the combined checkpoint, without manufacturing a commit.
    pin(
        &r,
        &format!("{}-index", r.reference),
        r.index_tree.as_deref().ok_or("Missing index tree")?,
        cancel,
    )?;
    let old_tree = text(
        &r,
        &["rev-parse", &format!("{before}^{{tree}}")],
        None,
        cancel,
    )?;
    let saved = if already_saved {
        head
    } else if old_tree == expected {
        before.into()
    } else {
        fs::copy(&working, &committing).map_err(|e| e.to_string())?;
        if let Err(error) = git(&r, &["commit", "-m", &message], Some(&committing), cancel) {
            return Err(format!(
                "Checkpoint command failed or outcome uncertain: {error}. Retry this archive request to reconcile HEAD; files/index retained"
            ));
        }
        text(&r, &["rev-parse", "HEAD"], None, cancel)?
    };
    pin(&r, &r.reference, &saved, cancel)?;
    if saved != before
        && (text(&r, &["show", "-s", "--format=%P", &saved], None, cancel)? != before
            || text(&r, &["show", "-s", "--format=%B", &saved], None, cancel)? != message)
    {
        return Err(format!(
            "Local commit {saved} protected, but hooks or concurrent activity changed checkpoint identity; archive incomplete, original index retained"
        ));
    }
    if text(
        &r,
        &["rev-parse", &format!("{saved}^{{tree}}")],
        None,
        cancel,
    )? != expected
    {
        return Err(format!(
            "Checkpoint {saved} exists locally but hooks changed its tree; archive incomplete, original index retained"
        ));
    }
    fs::copy(&working, &verify).map_err(|e| e.to_string())?;
    if inventory(&r, &verify, cancel)? != expected {
        return Err(format!(
            "Checkpoint {saved} protected locally, but files changed during save; workspace remains unarchived"
        ));
    }
    if text(&r, &["rev-parse", "HEAD"], None, cancel)? != saved {
        return Err(format!(
            "Checkpoint {saved} protected locally; HEAD moved, archive incomplete"
        ));
    }
    same(&r, cancel)?;
    if text(&r, &["symbolic-ref", "--quiet", "HEAD"], None, cancel)?.as_str()
        != r.branch.as_deref().ok_or("Missing prepared branch")?
    {
        return Err(format!(
            "Checkpoint {saved} protected locally; branch changed during save, archive incomplete"
        ));
    }
    // Commit combines staging, as ADR0007 specifies. Replace only our locked
    // original index with a tree matching saved HEAD; failure paths never restore it.
    git(&r, &["read-tree", &saved], Some(&verify), cancel)?;
    if index_hash(&r, cancel)? != admitted_index_hash {
        return Err(format!(
            "Checkpoint {saved} protected locally; index changed externally during save, archive incomplete"
        ));
    }
    fs::copy(&verify, &lock.path).map_err(|e| e.to_string())?;
    fs::File::open(&lock.path)
        .and_then(|f| f.sync_all())
        .map_err(|e| e.to_string())?;
    fs::rename(&lock.path, index_path(&r)).map_err(|e| e.to_string())?;
    lock.held = false;
    fs::File::open(&r.checkout.git_dir)
        .and_then(|f| f.sync_all())
        .map_err(|e| e.to_string())?;
    r.saved = Some(saved.clone());
    r.phase = Phase::Saved;
    r.message = format!(
        "{} {saved}; saved locally, not pushed. Tracked + new nonignored files; {} ignored paths excluded and retained. Original staged tree: {}-index",
        if saved == before {
            "Clean HEAD retained"
        } else {
            "Archive checkpoint"
        },
        r.excluded.len(),
        r.reference
    );
    Ok(r)
}

#[cfg(test)]
#[path = "../tests/support/archive_git.rs"]
mod tests;
