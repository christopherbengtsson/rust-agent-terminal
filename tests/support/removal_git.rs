use super::*;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
fn git(path: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().into()
}
struct Fixture {
    temp: tempfile::TempDir,
    root: PathBuf,
    linked: PathBuf,
    workspace: crate::registry::Workspace,
}
impl Fixture {
    fn new(detached: bool) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("main");
        fs::create_dir(&root).unwrap();
        git(&root, &["init", "-b", "main"]);
        git(&root, &["config", "user.name", "Removal Test"]);
        git(&root, &["config", "user.email", "removal@example.invalid"]);
        fs::write(root.join("tracked"), "main commit\n").unwrap();
        fs::write(root.join(".gitignore"), "ignored\n").unwrap();
        git(&root, &["add", "."]);
        git(&root, &["commit", "-m", "main fixture"]);
        let linked = temp.path().join("linked");
        let flag = if detached { "--detach" } else { "-b" };
        if detached {
            git(&root, &["worktree", "add", flag, linked.to_str().unwrap()]);
        } else {
            git(
                &root,
                &[
                    "worktree",
                    "add",
                    flag,
                    "workspace",
                    linked.to_str().unwrap(),
                ],
            );
        }
        fs::write(linked.join("tracked"), "saved code only in workspace\n").unwrap();
        git(&linked, &["commit", "-am", "workspace code"]);
        let checkout = crate::git::resolve_checkout(&linked, &AtomicBool::new(false)).unwrap();
        let workspace = crate::registry::Workspace {
            creation: None,
            id: uuid::Uuid::new_v4().to_string(),
            task: "removal-fixture".into(),
            legacy_tasks: vec![],
            name: "Removal".into(),
            project: None,
            path: None,
            checkout: Some(checkout),
            sessions: vec![],
        };
        Self {
            temp,
            root,
            linked,
            workspace,
        }
    }
    fn preview(&self) -> Snapshot {
        preview(
            &self.workspace,
            &uuid::Uuid::new_v4().to_string(),
            &AtomicBool::new(false),
        )
        .unwrap()
    }
}
#[test]
fn removal_never_archived_detached_saved_code_survives_gc_and_lost_reply() {
    let f = Fixture::new(true);
    let snapshot = f.preview();
    assert!(snapshot.blockers.is_empty(), "{:?}", snapshot.blockers);
    assert!(snapshot.local_only);
    assert!(snapshot.branch.is_none());
    assert!(!reconcile(&snapshot, &AtomicBool::new(false)).unwrap());
    protect(&snapshot, &AtomicBool::new(false)).unwrap();
    remove(&snapshot, &AtomicBool::new(false)).unwrap();
    assert!(!f.linked.exists());
    assert!(reconcile(&snapshot, &AtomicBool::new(false)).unwrap());
    git(&f.root, &["reflog", "expire", "--expire=now", "--all"]);
    git(&f.root, &["gc", "--prune=now"]);
    assert_eq!(
        git(
            &f.root,
            &["show", &format!("{}:tracked", snapshot.reference)]
        ),
        "saved code only in workspace"
    );
    assert_eq!(git(&f.root, &["show", "main:tracked"]), "main commit");
    // Read-only reconciliation after a lost acknowledgement never touches a replacement.
    fs::create_dir(&f.linked).unwrap();
    fs::write(f.linked.join("valuable"), "replacement data").unwrap();
    assert!(reconcile(&snapshot, &AtomicBool::new(false)).is_err());
    assert_eq!(
        fs::read_to_string(f.linked.join("valuable")).unwrap(),
        "replacement data"
    );
}

#[test]
fn removal_blocks_unsaved_tracked_untracked_and_ignored_files_at_preview_and_confirmation() {
    for name in ["tracked", "new-file", "ignored"] {
        let f = Fixture::new(false);
        fs::write(f.linked.join(name), "unsaved work\n").unwrap();
        let blocked = f.preview();
        assert!(
            blocked.blockers.iter().any(|b| b.contains(name)),
            "{name}: {:?}",
            blocked.blockers
        );
        assert!(protect(&blocked, &AtomicBool::new(false)).is_err());
        assert!(f.linked.exists());
        if name == "tracked" {
            git(&f.linked, &["restore", "tracked"]);
        } else {
            fs::remove_file(f.linked.join(name)).unwrap();
        }
        let clean = f.preview();
        assert!(clean.blockers.is_empty(), "{:?}", clean.blockers);
        protect(&clean, &AtomicBool::new(false)).unwrap();
        // Git ordinarily removes ignored files. The independent late inventory must refuse.
        fs::write(f.linked.join(name), "late unsaved work\n").unwrap();
        assert!(
            remove(&clean, &AtomicBool::new(false)).is_err(),
            "late {name} must block ordinary Git removal"
        );
        assert_eq!(
            fs::read_to_string(f.linked.join(name)).unwrap(),
            "late unsaved work\n"
        );
    }
}

#[test]
fn removal_distinguishes_local_only_and_live_remote_backup_from_uncertain_missing_remote() {
    let f = Fixture::new(false);
    let remote = f.temp.path().join("remote.git");
    fs::create_dir(&remote).unwrap();
    git(&remote, &["init", "--bare"]);
    git(
        &f.root,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    // Existing remote with no upstream and an empty advertisement is known local-only.
    let local = f.preview();
    assert!(local.blockers.is_empty(), "{:?}", local.blockers);
    assert!(local.local_only);
    git(&f.linked, &["push", "-u", "origin", "workspace"]);
    let pushed = f.preview();
    assert!(pushed.blockers.is_empty(), "{:?}", pushed.blockers);
    assert!(
        !pushed.local_only,
        "live exact upstream commit establishes remote evidence"
    );
    protect(&pushed, &AtomicBool::new(false)).unwrap();
    git(
        &f.root,
        &[
            "remote",
            "set-url",
            "origin",
            "/definitely-missing-removal-test-remote.git",
        ],
    );
    assert!(remove(&pushed, &AtomicBool::new(false)).is_err());
    assert!(f.linked.exists());
    let missing = f.preview();
    assert!(
        missing
            .blockers
            .iter()
            .any(|b| b.contains("remote") || b.contains("Remote")),
        "{:?}",
        missing.blockers
    );
    assert!(protect(&missing, &AtomicBool::new(false)).is_err());
    // A branch whose configured upstream refers to a removed remote is also uncertain.
    git(&f.root, &["remote", "remove", "origin"]);
    git(&f.root, &["config", "branch.workspace.remote", "gone"]);
    git(
        &f.root,
        &["config", "branch.workspace.merge", "refs/heads/workspace"],
    );
    let gone = f.preview();
    assert!(
        gone.blockers
            .iter()
            .any(|b| b.contains("upstream") || b.contains("Upstream")),
        "{:?}",
        gone.blockers
    );
}

#[test]
fn removal_rejects_hidden_index_changes_and_in_progress_git_operations() {
    let f = Fixture::new(false);
    git(
        &f.linked,
        &["update-index", "--assume-unchanged", "tracked"],
    );
    fs::write(f.linked.join("tracked"), "hidden unsaved work\n").unwrap();
    let hidden = f.preview();
    assert!(
        hidden
            .blockers
            .iter()
            .any(|b| b.contains("index") || b.contains("Index")),
        "{:?}",
        hidden.blockers
    );
    assert!(protect(&hidden, &AtomicBool::new(false)).is_err());
    git(
        &f.linked,
        &["update-index", "--no-assume-unchanged", "tracked"],
    );
    git(&f.linked, &["restore", "tracked"]);
    for name in ["index.lock", "HEAD.lock", "MERGE_HEAD", "rebase-merge"] {
        fs::write(
            f.workspace.checkout.as_ref().unwrap().git_dir.join(name),
            "external operation",
        )
        .unwrap();
        let blocked = f.preview();
        assert!(
            blocked.blockers.iter().any(|b| b.contains(name)),
            "{name}: {:?}",
            blocked.blockers
        );
        fs::remove_file(f.workspace.checkout.as_ref().unwrap().git_dir.join(name)).unwrap();
    }
}

#[test]
fn removal_retains_visible_branch_prior_archive_refs_and_refuses_stale_preview() {
    let f = Fixture::new(false);
    let archive = format!(
        "refs/rat/archive/{}/{}",
        f.workspace.id,
        uuid::Uuid::new_v4()
    );
    let old = git(&f.root, &["rev-parse", "main"]);
    git(&f.root, &["update-ref", &archive, &old]);
    let snapshot = f.preview();
    protect(&snapshot, &AtomicBool::new(false)).unwrap();
    git(&f.root, &["update-ref", &snapshot.reference, &old]);
    assert!(remove(&snapshot, &AtomicBool::new(false)).is_err());
    assert!(f.linked.exists());
    git(
        &f.root,
        &["update-ref", &snapshot.reference, &snapshot.head],
    );
    remove(&snapshot, &AtomicBool::new(false)).unwrap();
    assert_eq!(git(&f.root, &["rev-parse", "workspace"]), snapshot.head);
    assert_eq!(git(&f.root, &["rev-parse", &archive]), old);
    git(&f.root, &["reflog", "expire", "--expire=now", "--all"]);
    git(&f.root, &["gc", "--prune=now"]);
    assert_eq!(
        git(
            &f.root,
            &["show", &format!("{}:tracked", snapshot.reference)]
        ),
        "saved code only in workspace"
    );
    assert_eq!(
        git(&f.root, &["show", &format!("{archive}:tracked")]),
        "main commit"
    );
}

#[test]
fn removal_excludes_main_locked_prunable_and_replaced_checkout_identity() {
    let f = Fixture::new(false);
    let mut main = f.workspace.clone();
    main.checkout = Some(crate::git::resolve_checkout(&f.root, &AtomicBool::new(false)).unwrap());
    assert!(
        preview(
            &main,
            &uuid::Uuid::new_v4().to_string(),
            &AtomicBool::new(false)
        )
        .unwrap_err()
        .contains("Main checkout")
    );
    git(&f.root, &["worktree", "lock", f.linked.to_str().unwrap()]);
    let locked = f.preview();
    assert!(locked.blockers.iter().any(|b| b.contains("locked")));
    assert!(protect(&locked, &AtomicBool::new(false)).is_err());
    git(&f.root, &["worktree", "unlock", f.linked.to_str().unwrap()]);
    let frozen = f.preview();
    protect(&frozen, &AtomicBool::new(false)).unwrap();
    git(&f.root, &["worktree", "lock", f.linked.to_str().unwrap()]);
    assert!(remove(&frozen, &AtomicBool::new(false)).is_err());
    git(&f.root, &["worktree", "unlock", f.linked.to_str().unwrap()]);
    let retained = f.temp.path().join("retained-original");
    fs::rename(&f.linked, &retained).unwrap();
    // Missing/prunable registration never authorizes recursive recorded-path deletion.
    assert!(
        preview(
            &f.workspace,
            &uuid::Uuid::new_v4().to_string(),
            &AtomicBool::new(false)
        )
        .is_err()
    );
    assert!(reconcile(&frozen, &AtomicBool::new(false)).is_err());
    fs::create_dir(&f.linked).unwrap();
    fs::copy(retained.join(".git"), f.linked.join(".git")).unwrap();
    fs::write(f.linked.join("precious"), "replacement must remain").unwrap();
    assert!(remove(&frozen, &AtomicBool::new(false)).is_err());
    assert!(reconcile(&frozen, &AtomicBool::new(false)).is_err());
    assert_eq!(
        fs::read_to_string(f.linked.join("precious")).unwrap(),
        "replacement must remain"
    );
    assert_eq!(
        fs::read_to_string(retained.join("tracked")).unwrap(),
        "saved code only in workspace\n"
    );
}

#[test]
fn removal_reference_hook_cannot_sneak_ignored_files_past_protection() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new(false);
    let snapshot = f.preview();
    let hook = snapshot
        .checkout
        .common_dir
        .join("hooks/reference-transaction");
    fs::write(
        &hook,
        format!(
            "#!/bin/sh\nif test \"$1\" = committed; then printf 'hook ignored data' > '{}'; fi\n",
            f.linked.join("ignored").display()
        ),
    )
    .unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    let error = protect(&snapshot, &AtomicBool::new(false)).unwrap_err();
    assert!(error.contains("Ignored"), "{error}");
    assert_eq!(
        git(&f.root, &["rev-parse", &snapshot.reference]),
        snapshot.head
    );
    assert_eq!(
        fs::read_to_string(f.linked.join("ignored")).unwrap(),
        "hook ignored data"
    );
    assert!(remove(&snapshot, &AtomicBool::new(false)).is_err());
    assert!(!reconcile(&snapshot, &AtomicBool::new(false)).unwrap());
    fs::remove_file(hook).unwrap();
    fs::remove_file(f.linked.join("ignored")).unwrap();
    protect(&snapshot, &AtomicBool::new(false)).unwrap();
    remove(&snapshot, &AtomicBool::new(false)).unwrap();
}

#[test]
fn removal_configured_local_upstream_is_not_a_remote_backup_and_missing_is_uncertain() {
    let f = Fixture::new(false);
    git(&f.root, &["config", "branch.workspace.remote", "."]);
    git(
        &f.root,
        &["config", "branch.workspace.merge", "refs/heads/main"],
    );
    let local = f.preview();
    assert!(local.blockers.is_empty(), "{:?}", local.blockers);
    assert!(local.local_only);
    assert!(local.warning.contains("locally"));
    git(
        &f.root,
        &[
            "config",
            "branch.workspace.merge",
            "refs/heads/absent-upstream",
        ],
    );
    let missing = f.preview();
    assert!(
        missing
            .blockers
            .iter()
            .any(|b| b.contains("local upstream") && b.contains("missing")),
        "{:?}",
        missing.blockers
    );
    assert!(protect(&missing, &AtomicBool::new(false)).is_err());
}

#[test]
fn removal_preserves_opaque_unix_checkout_path_identity_through_receipt_and_git() {
    use std::os::unix::ffi::OsStringExt;
    let mut f = Fixture::new(true);
    // macOS's filesystem rejects invalid UTF-8 names. Newline/tab/space still
    // exercise opaque argument and NUL-record handling on the native filesystem.
    let raw = f.temp.path().join(std::ffi::OsString::from_vec(
        b"linked-\n\t checkout".to_vec(),
    ));
    let output = Command::new("git")
        .arg("-C")
        .arg(&f.root)
        .args(["worktree", "move"])
        .arg(&f.linked)
        .arg(&raw)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    f.linked = raw;
    f.workspace.checkout =
        Some(crate::git::resolve_checkout(&f.linked, &AtomicBool::new(false)).unwrap());
    let frozen = f.preview();
    let snapshot: Snapshot = serde_json::from_slice(&serde_json::to_vec(&frozen).unwrap()).unwrap();
    assert_eq!(snapshot.checkout, f.workspace.checkout.clone().unwrap());
    protect(&snapshot, &AtomicBool::new(false)).unwrap();
    remove(&snapshot, &AtomicBool::new(false)).unwrap();
    assert!(!f.linked.exists());
    assert!(reconcile(&snapshot, &AtomicBool::new(false)).unwrap());
    assert_eq!(
        git(
            &f.root,
            &["show", &format!("{}:tracked", snapshot.reference)]
        ),
        "saved code only in workspace"
    );
}

#[test]
fn removal_cancelled_after_durable_git_ref_creation_never_removes_and_reconciles_same_ref() {
    use std::{
        os::unix::fs::PermissionsExt,
        sync::{Arc, atomic::Ordering},
        time::{Duration, Instant},
    };
    let f = Fixture::new(true);
    let snapshot = f.preview();
    let marker = f.temp.path().join("reference-created");
    let hook = snapshot
        .checkout
        .common_dir
        .join("hooks/reference-transaction");
    fs::write(
        &hook,
        format!(
            "#!/bin/sh\nif test \"$1\" = committed; then printf ready > '{}'; sleep 15; fi\n",
            marker.display()
        ),
    )
    .unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    let cancellation = Arc::new(AtomicBool::new(false));
    let cancel = cancellation.clone();
    let clone = snapshot.clone();
    let worker = std::thread::spawn(move || protect(&clone, &cancel));
    let deadline = Instant::now() + Duration::from_secs(5);
    while !marker.exists() {
        assert!(
            Instant::now() < deadline,
            "Committed reference hook did not run"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    cancellation.store(true, Ordering::Release);
    assert!(worker.join().unwrap().is_err());
    assert!(f.linked.exists());
    assert_eq!(
        git(&f.root, &["rev-parse", &snapshot.reference]),
        snapshot.head
    );
    assert!(!reconcile(&snapshot, &AtomicBool::new(false)).unwrap());
    fs::remove_file(hook).unwrap();
    // Explicit retry validates the same protected ref; it creates no second point.
    protect(&snapshot, &AtomicBool::new(false)).unwrap();
    remove(&snapshot, &AtomicBool::new(false)).unwrap();
    assert!(reconcile(&snapshot, &AtomicBool::new(false)).unwrap());
}

#[test]
fn removal_refuses_uncertain_local_object_recovery_coverage() {
    let f = Fixture::new(true);
    let common = &f.workspace.checkout.as_ref().unwrap().common_dir;
    // A ref in this repository cannot protect objects owned by another repository's GC.
    let foreign = f.temp.path().join("foreign-object-database");
    fs::create_dir(&foreign).unwrap();
    let alternates = common.join("objects/info/alternates");
    fs::write(&alternates, format!("{}\n", foreign.display())).unwrap();
    let snapshot = f.preview();
    assert!(
        snapshot.blockers.iter().any(|b| b.contains("alternate")),
        "{:?}",
        snapshot.blockers
    );
    assert!(protect(&snapshot, &AtomicBool::new(false)).is_err());
    fs::remove_file(alternates).unwrap();
    git(&f.root, &["config", "remote.promisor.promisor", "true"]);
    let partial = f.preview();
    assert!(
        partial
            .blockers
            .iter()
            .any(|b| b.contains("partial") || b.contains("promisor")),
        "{:?}",
        partial.blockers
    );
}

#[test]
fn removal_refuses_changed_head_or_branch_after_preview_and_retains_each_recovery_point() {
    let f = Fixture::new(false);
    let first = f.preview();
    protect(&first, &AtomicBool::new(false)).unwrap();
    git(
        &f.linked,
        &["commit", "--allow-empty", "-m", "external advance"],
    );
    let advanced = git(&f.linked, &["rev-parse", "HEAD"]);
    assert_ne!(first.head, advanced);
    assert!(
        remove(&first, &AtomicBool::new(false))
            .unwrap_err()
            .contains("HEAD")
    );
    let second = f.preview();
    protect(&second, &AtomicBool::new(false)).unwrap();
    git(&f.linked, &["switch", "-c", "externally-selected-branch"]);
    assert!(
        remove(&second, &AtomicBool::new(false))
            .unwrap_err()
            .contains("branch")
    );
    assert!(f.linked.exists());
    assert_eq!(git(&f.root, &["rev-parse", &first.reference]), first.head);
    assert_eq!(git(&f.root, &["rev-parse", &second.reference]), advanced);
    assert_eq!(
        git(&f.root, &["rev-parse", "externally-selected-branch"]),
        advanced
    );
}
