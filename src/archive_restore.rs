//! Conservative branch recreation from a durable archive recovery point.
use crate::{archive::Record, git::RepositoryMutation};
use std::{process::Command, sync::atomic::AtomicBool, time::Duration};

fn command(r: &Record) -> Command {
    let mut c = Command::new("git");
    c.arg("-C").arg(&r.checkout.root).args([
        "-c",
        "core.fsync=committed,reference",
        "-c",
        "core.fsyncMethod=fsync",
    ]);
    c
}
fn git(r: &Record, args: &[&str], cancel: &AtomicBool) -> Result<String, String> {
    let bytes = crate::git::run_git_command_deadline(
        command(r).args(args),
        cancel,
        Duration::from_secs(30),
    )?;
    String::from_utf8(bytes)
        .map(|s| s.trim().into())
        .map_err(|_| "Invalid Git object identity".into())
}
fn reference(r: &Record, name: &str, cancel: &AtomicBool) -> Result<Option<String>, String> {
    let lines = git(
        r,
        &[
            "for-each-ref",
            "--format=%(refname) %(objectname) %(symref)",
            name,
        ],
        cancel,
    )?;
    for line in lines.lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.first() == Some(&name) {
            if fields.len() != 2 {
                return Err(
                    "Symbolic or malformed recovery/branch reference; restore refused".into(),
                );
            }
            return Ok(Some(fields[1].into()));
        }
    }
    Ok(None)
}
fn verify<'a>(r: &'a Record, cancel: &AtomicBool) -> Result<&'a str, String> {
    if crate::git::resolve_checkout(&r.checkout.root, cancel)? != r.checkout {
        return Err("Checkout identity changed; restoration unavailable".into());
    }
    let saved = r
        .saved
        .as_deref()
        .ok_or("No saved archive commit; restoration unavailable")?;
    if reference(r, &r.reference, cancel)?.as_deref() != Some(saved)
        || git(r, &["cat-file", "-t", saved], cancel)? != "commit"
    {
        return Err(
            "Archive recovery reference no longer confirms saved commit; restore refused".into(),
        );
    }
    Ok(saved)
}

/// Called only by an admitted Git worker after durable restoration intent.
/// Branch and operation marker are created together. A failed switch retains both
/// so an explicit retry can reconcile without taking ownership of a reused name.
pub fn restore(r: &Record, cancel: &AtomicBool) -> Result<String, String> {
    let _lease = RepositoryMutation::acquire(&r.checkout.common_dir, cancel)?;
    let saved = verify(r, cancel)?;
    for operation in [
        "MERGE_HEAD",
        "CHERRY_PICK_HEAD",
        "REVERT_HEAD",
        "rebase-merge",
        "rebase-apply",
        "BISECT_LOG",
    ] {
        match std::fs::symlink_metadata(r.checkout.git_dir.join(operation)) {
            Ok(_) => {
                return Err(format!(
                    "{operation} is present; finish or abort the Git operation before restoring"
                ));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(format!(
                    "Git operation state unavailable; restore refused: {e}"
                ));
            }
        }
    }
    let original = r
        .branch
        .as_deref()
        .and_then(|b| b.strip_prefix("refs/heads/"))
        .ok_or("Original local branch unavailable; restore refused")?;
    git(r, &["check-ref-format", "--branch", original], cancel)?;
    if !git(
        r,
        &["status", "--porcelain=v1", "--untracked-files=all"],
        cancel,
    )?
    .is_empty()
    {
        return Err("Unsaved files present; restore refused, folder retained".into());
    }
    let head = git(r, &["rev-parse", "--verify", "HEAD"], cancel)?;
    if head != saved {
        return Err(
            "Checkout HEAD advanced or changed; restore refused, external work retained".into(),
        );
    }
    let symbolic = git(r, &["rev-parse", "--symbolic-full-name", "HEAD"], cancel)?;
    let original_ref = format!("refs/heads/{original}");
    // A retained branch already checked out at the saved point needs no mutation.
    if symbolic == original_ref && reference(r, &original_ref, cancel)?.as_deref() == Some(saved) {
        return Ok(original.into());
    }
    uuid::Uuid::parse_str(&r.workspace).map_err(|_| "Invalid workspace identity")?;
    uuid::Uuid::parse_str(&r.request).map_err(|_| "Invalid archive request identity")?;
    let prefix = format!("refs/rat/restore/{}/{}/", r.workspace, r.request);
    let markers = git(r, &["for-each-ref", "--format=%(refname)", &prefix], cancel)?;
    let names: Vec<_> = markers.lines().collect();
    if names.len() > 1 {
        return Err("Ambiguous restoration markers; branches retained".into());
    }
    let branch = if let Some(marker) = names.first() {
        let name = marker
            .strip_prefix(&prefix)
            .ok_or("Invalid restoration marker")?;
        let full = format!("refs/heads/{name}");
        if reference(r, marker, cancel)?.as_deref() != Some(saved)
            || reference(r, &full, cancel)?.as_deref() != Some(saved)
        {
            return Err(
                "Restoration branch changed or disappeared; retained references require inspection"
                    .into(),
            );
        }
        name.to_string()
    } else {
        if symbolic != "HEAD" {
            return Err("Workspace switched to another branch; restore refused".into());
        }
        let candidate = if original.len() <= 100 && reference(r, &original_ref, cancel)?.is_none() {
            original.to_string()
        } else {
            // Full operation UUID avoids dependence on a visible name's identity.
            let stem = format!("rat-restored-{}", r.request);
            let mut available = None;
            for suffix in 0..128 {
                let name = if suffix == 0 {
                    stem.clone()
                } else {
                    format!("{stem}-{suffix}")
                };
                if reference(r, &format!("refs/heads/{name}"), cancel)?.is_none() {
                    available = Some(name);
                    break;
                }
            }
            available.ok_or("Restoration names occupied; no branch overwritten")?
        };
        let full = format!("refs/heads/{candidate}");
        if reference(r, &full, cancel)?.is_some() {
            return Err("Restoration name is already in use; no branch overwritten".into());
        }
        let marker = format!("{prefix}{candidate}");
        let input =
            format!("start\ncreate {full} {saved}\ncreate {marker} {saved}\nprepare\ncommit\n");
        verify(r, cancel)?;
        crate::git::run_git_command_input(
            command(r).args(["update-ref", "--stdin"]),
            cancel,
            input.as_bytes(),
        )
        .map_err(|e| {
            format!(
                "Branch creation failed or outcome uncertain; retry same request to reconcile: {e}"
            )
        })?;
        candidate
    };
    let full = format!("refs/heads/{branch}");
    if symbolic != "HEAD" && symbolic != full {
        return Err("Workspace switched to another branch; restore refused".into());
    }
    verify(r, cancel)?;
    if reference(r, &full, cancel)?.as_deref() != Some(saved)
        || git(r, &["rev-parse", "HEAD"], cancel)? != saved
        || !git(
            r,
            &["status", "--porcelain=v1", "--untracked-files=all"],
            cancel,
        )?
        .is_empty()
    {
        return Err(
            "Restoration branch/HEAD/files moved; branch retained without switching".into(),
        );
    }
    if symbolic != full {
        git(
            r,
            &[
                "switch",
                "--no-guess",
                "--no-overwrite-ignore",
                "--no-recurse-submodules",
                "--",
                &branch,
            ],
            cancel,
        )
        .map_err(|e| {
            format!(
                "Restoration branch {branch} created and retained; switch failed or uncertain: {e}"
            )
        })?;
    }
    verify(r, cancel)?;
    if git(r, &["rev-parse", "--symbolic-full-name", "HEAD"], cancel)? != full
        || git(r, &["rev-parse", "HEAD"], cancel)? != saved
        || reference(r, &full, cancel)?.as_deref() != Some(saved)
        || !git(
            r,
            &["status", "--porcelain=v1", "--untracked-files=all"],
            cancel,
        )?
        .is_empty()
    {
        return Err(
            "Branch created, but checkout changed during restoration; inspect retained work".into(),
        );
    }
    Ok(branch)
}

#[cfg(test)]
#[path = "../tests/support/archive_restore.rs"]
mod tests;
