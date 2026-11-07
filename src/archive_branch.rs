//! Branch cleanup is separate from durable code saving. Ordinary Git owns checkout
//! refusal; a command-local guard compares the saved tip under Git's ref lock.
use crate::archive::Record;
use std::sync::atomic::AtomicBool;
fn git(r: &Record, args: &[&str], cancel: &AtomicBool) -> Result<String, String> {
    let bytes = crate::git::run_git(&r.checkout.root, args, cancel)?;
    String::from_utf8(bytes)
        .map(|s| s.trim().into())
        .map_err(|_| "Unsupported Git identity".into())
}
pub fn cleanup(r: &Record, cancel: &AtomicBool) -> Result<String, String> {
    use std::{fs, os::unix::fs::PermissionsExt, process::Command, time::Duration};
    let _lease = crate::git::RepositoryMutation::acquire(&r.checkout.common_dir, cancel)?;
    if crate::git::resolve_checkout(&r.checkout.root, cancel)? != r.checkout {
        return Err("Checkout identity changed".into());
    }
    if r.phase != crate::archive::Phase::Archived {
        return Err("Durable archive confirmation required before cleanup".into());
    }
    let branch = r.delete_target.as_deref().ok_or("Deletion not selected")?;
    if r.owned_branch.as_deref().is_some_and(|b| b != branch) {
        return Err("Observed external branch rename/switch; creation provenance retained".into());
    }
    if r.branch.as_deref() != Some(branch) {
        return Err(
            "Branch renamed/switched since product creation or exact deletion selection".into(),
        );
    }
    let short = branch
        .strip_prefix("refs/heads/")
        .ok_or("Only local branch deletion is supported")?;
    git(r, &["check-ref-format", "--branch", short], cancel)?;
    if matches!(short, "main" | "master" | "trunk")
        || r.protected_base.as_deref() == Some(branch)
        || r.protected_base.as_deref() == Some(short)
        || r.protected_base
            .as_deref()
            .and_then(|b| b.strip_prefix("refs/remotes/"))
            .and_then(|b| b.split_once('/'))
            .is_some_and(|(_, b)| b == short)
    {
        return Err("Main/default/base branch is protected".into());
    }
    let refs = git(
        r,
        &["for-each-ref", "--format=%(symref)", "refs/remotes"],
        cancel,
    )?;
    if refs.lines().any(|f| {
        f.strip_prefix("refs/remotes/")
            .and_then(|s| s.split_once('/'))
            .is_some_and(|(_, b)| b == short)
    }) {
        return Err("Repository default branch is protected".into());
    }
    let saved = r.saved.as_deref().ok_or("No saved commit")?;
    if git(r, &["rev-parse", "--verify", &r.reference], cancel)? != saved {
        return Err("Saved recovery ref changed or unavailable".into());
    }
    // Missing after a crash is observation, never proof this operation deleted it.
    let tip = git(r, &["rev-parse", "--verify", branch], cancel)
        .map_err(|_| "Branch absent; prior cleanup outcome uncertain; recovery retained")?;
    if tip != saved {
        return Err("Branch advanced/replaced; external commits retained".into());
    }
    if git(r, &["symbolic-ref", "--quiet", branch], cancel).is_ok() {
        return Err("Symbolic branch reference is unsupported".into());
    }
    if !git(
        r,
        &["status", "--porcelain", "--untracked-files=all"],
        cancel,
    )?
    .is_empty()
    {
        return Err("Unsaved files/index changes block safe detach".into());
    }
    let head = git(r, &["rev-parse", "HEAD"], cancel)?;
    if head != saved {
        return Err("Checkout HEAD changed".into());
    }
    let symbolic = git(r, &["symbolic-ref", "--quiet", "HEAD"], cancel).ok();
    if symbolic.as_deref().is_some_and(|s| s != branch) {
        return Err("Checkout branch switched externally".into());
    }
    // A custom hooksPath or executable transaction hook cannot be bypassed.
    // Supporting delegation is deliberately outside this narrowly qualified path.
    if crate::git::run_git(
        &r.checkout.root,
        &["config", "--get", "core.hooksPath"],
        cancel,
    )
    .is_ok()
    {
        return Err("Custom hooksPath requires manual branch cleanup; hooks preserved".into());
    }
    let hook = r.checkout.common_dir.join("hooks/reference-transaction");
    match fs::metadata(&hook) {
        Ok(m) if m.permissions().mode() & 0o111 != 0 => {
            return Err(
                "Reference-transaction hook requires manual cleanup; hook preserved".into(),
            );
        }
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            return Err(format!("Hook inspection uncertain: {e}"));
        }
        _ => {}
    }
    if git(r, &["config", "--get", "extensions.refStorage"], cancel).is_ok_and(|s| s != "files") {
        return Err("Unsupported Git ref backend; branch retained".into());
    }
    if !git(r, &["--version"], cancel)?.starts_with("git version 2.50.") {
        return Err("Branch guard qualified only on Git 2.50 files backend".into());
    }
    if symbolic.is_some() {
        git(
            r,
            &[
                "switch",
                "--no-overwrite-ignore",
                "--no-recurse-submodules",
                "--detach",
                saved,
            ],
            cancel,
        )?;
    }
    if crate::git::resolve_checkout(&r.checkout.root, cancel)? != r.checkout
        || git(r, &["rev-parse", "HEAD"], cancel)? != saved
        || git(r, &["symbolic-ref", "--quiet", "HEAD"], cancel).is_ok()
        || !git(
            r,
            &["status", "--porcelain", "--untracked-files=all"],
            cancel,
        )?
        .is_empty()
    {
        return Err("Safe detach did not retain expected clean checkout".into());
    }
    if git(r, &["rev-parse", "--verify", branch], cancel)? != saved
        || git(r, &["rev-parse", "--verify", &r.reference], cancel)? != saved
    {
        return Err("Branch/recovery changed after detach".into());
    }
    if crate::git::run_git(
        &r.checkout.root,
        &["config", "--get", "core.hooksPath"],
        cancel,
    )
    .is_ok()
        || fs::metadata(&hook).is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
    {
        return Err("Hook policy changed during detach; hooks preserved, branch retained".into());
    }
    if !git(r, &["for-each-ref", "--format=%(symref)", branch], cancel)?.is_empty() {
        return Err("Branch replaced with symbolic reference during detach; retained".into());
    }
    let defaults = git(
        r,
        &["for-each-ref", "--format=%(symref)", "refs/remotes"],
        cancel,
    )?;
    if defaults.lines().any(|f| {
        f.strip_prefix("refs/remotes/")
            .and_then(|s| s.split_once('/'))
            .is_some_and(|(_, b)| b == short)
    }) {
        return Err(
            "Repository default branch changed during detach; protected branch retained".into(),
        );
    }
    let worktrees = crate::git::run_git(
        &r.checkout.root,
        &["worktree", "list", "--porcelain", "-z"],
        cancel,
    )?;
    let occupied = format!("branch {branch}");
    if worktrees
        .split(|b| *b == 0)
        .any(|field| field == occupied.as_bytes())
    {
        return Err("Branch in use in another worktree".into());
    }
    let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    let guard = dir.path().join("reference-transaction");
    fs::write(&guard, GUARD).map_err(|e| e.to_string())?;
    fs::set_permissions(&guard, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
    let mut hooks_option = std::ffi::OsString::from("core.hooksPath=");
    hooks_option.push(dir.path());
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(&r.checkout.root)
        .arg("-c")
        .arg(hooks_option)
        .args([
            "-c",
            "core.fsync=committed,reference",
            "-c",
            "core.fsyncMethod=fsync",
            "branch",
            "-D",
            "--",
            short,
        ]);
    cmd.env("RAT_ARCHIVE_SHORT", short)
        .env("RAT_ARCHIVE_HOOK", &hook)
        .env("RAT_ARCHIVE_GUARD_DIR", dir.path())
        .env("RAT_ARCHIVE_BRANCH", branch)
        .env("RAT_ARCHIVE_SAVED", saved)
        .env("RAT_ARCHIVE_REF", &r.reference);
    crate::git::run_git_command_deadline(&mut cmd, cancel, Duration::from_secs(30)).map_err(
        |e| format!("Deletion refused or outcome uncertain: {e}; retry never targets a newer tip"),
    )?;
    if git(r, &["rev-parse", "--verify", branch], cancel).is_ok() {
        return Err("Branch still present or replaced after delete; preserved".into());
    }
    Ok(format!(
        "Local branch {short} deleted; folder and recovery retained. Git worktree-use checks are immediate snapshots."
    ))
}
// Git 2.50 unconditional branch deletion reports zero old OID. Read actual tip
// while prepared holds the ref lock. Ref housekeeping may cause empty callbacks.
const GUARD: &str = r#"#!/bin/sh
[ "$1" = prepared ] || exit 0
[ ! -x "$RAT_ARCHIVE_HOOK" ] || exit 1
[ "$(git config --get-all core.hooksPath)" = "$RAT_ARCHIVE_GUARD_DIR" ] || exit 1
seen=0
while read -r old new ref; do
  case "$ref" in
    "$RAT_ARCHIVE_BRANCH")
      case "$new" in *[!0]*) exit 1;; esac
      git symbolic-ref --quiet "$ref" >/dev/null 2>&1
      [ "$?" = 1 ] || exit 1
      [ "$(git rev-parse --verify "$ref")" = "$RAT_ARCHIVE_SAVED" ] || exit 1
      [ "$(git rev-parse --verify "$RAT_ARCHIVE_REF")" = "$RAT_ARCHIVE_SAVED" ] || exit 1
      defaults=$(git for-each-ref --format='%(symref)' refs/remotes) || exit 1
      while IFS= read -r default; do
        case "$default" in refs/remotes/*/*) name=${default#refs/remotes/}; name=${name#*/}; [ "$name" != "$RAT_ARCHIVE_SHORT" ] || exit 1;; esac
      done <<DEFAULTS
$defaults
DEFAULTS
      seen=1;;
    refs/heads/*) exit 1;;
  esac
done
if [ "$seen" = 1 ]; then
  trees=$(git worktree list --porcelain) || exit 1
  if printf '%s\n' "$trees" | git stripspace | while IFS= read -r line; do
      [ "$line" != "branch $RAT_ARCHIVE_BRANCH" ] || exit 1
    done; then :; else exit 1; fi
fi
exit 0
"#;
#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::Path, process::Command};
    fn git(root: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{:?}: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap().trim().into()
    }
    #[test]
    fn cleanup_detaches_and_deletes_only_saved_branch_retaining_ignored_files() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        git(root, &["init", "-b", "work"]);
        git(root, &["config", "user.name", "Test"]);
        git(root, &["config", "user.email", "test@example.invalid"]);
        fs::write(root.join(".gitignore"), "ignored\n").unwrap();
        git(root, &["add", "."]);
        git(root, &["commit", "-m", "base"]);
        fs::write(root.join("ignored"), "keep").unwrap();
        let checkout = crate::git::resolve_checkout(root, &AtomicBool::new(false)).unwrap();
        let w = crate::registry::Workspace {
            creation: None,
            id: uuid::Uuid::new_v4().to_string(),
            task: "fixture".into(),
            legacy_tasks: vec![],
            name: "fixture".into(),
            project: None,
            path: None,
            checkout: Some(checkout),
            sessions: vec![],
        };
        let mut r = Record::new(&w, &uuid::Uuid::new_v4().to_string()).unwrap();
        r.branch = Some("refs/heads/work".into());
        r.delete_target = r.branch.clone();
        r.phase = crate::archive::Phase::Archived;
        r.saved = Some(git(root, &["rev-parse", "HEAD"]));
        git(
            root,
            &["update-ref", &r.reference, r.saved.as_deref().unwrap()],
        );
        if !git(root, &["--version"]).starts_with("git version 2.50.") {
            assert!(
                cleanup(&r, &AtomicBool::new(false))
                    .unwrap_err()
                    .contains("qualified only")
            );
            return;
        }
        cleanup(&r, &AtomicBool::new(false)).unwrap();
        assert!(
            !Command::new("git")
                .arg("-C")
                .arg(root)
                .args(["show-ref", "--verify", "refs/heads/work"])
                .output()
                .unwrap()
                .status
                .success()
        );
        assert_eq!(fs::read_to_string(root.join("ignored")).unwrap(), "keep");
        assert_eq!(git(root, &["rev-parse", "HEAD"]), r.saved.unwrap());
    }
}

#[cfg(test)]
#[path = "../tests/support/archive_branch.rs"]
mod behavior_tests;
