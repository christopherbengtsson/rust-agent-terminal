use super::restore;
use crate::archive::{Phase, Record};
use std::{fs, path::Path, process::Command, sync::atomic::AtomicBool};

fn git(root: &Path, args: &[&str]) -> String {
    let result = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout).unwrap().trim().into()
}
struct Fixture {
    temp: tempfile::TempDir,
    record: Record,
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        git(root, &["init", "-b", "workspace"]);
        git(root, &["config", "user.name", "Restore Test"]);
        git(root, &["config", "user.email", "restore@example.invalid"]);
        fs::write(root.join("tracked"), "archived work\n").unwrap();
        fs::write(root.join(".gitignore"), "ignored\n").unwrap();
        git(root, &["add", "."]);
        git(root, &["commit", "-m", "archive fixture"]);
        let workspace = crate::registry::Workspace {
            creation: None,
            id: uuid::Uuid::new_v4().to_string(),
            task: "fixture".into(),
            legacy_tasks: vec![],
            name: "Restore".into(),
            project: None,
            path: None,
            checkout: Some(crate::git::resolve_checkout(root, &AtomicBool::new(false)).unwrap()),
            sessions: vec![],
        };
        let mut record = Record::new(&workspace, &uuid::Uuid::new_v4().to_string()).unwrap();
        record.phase = Phase::Archived;
        record.branch = Some("refs/heads/workspace".into());
        record.saved = Some(git(root, &["rev-parse", "HEAD"]));
        git(
            root,
            &[
                "update-ref",
                &record.reference,
                record.saved.as_deref().unwrap(),
            ],
        );
        git(root, &["switch", "--detach"]);
        git(root, &["branch", "-D", "workspace"]);
        Self { temp, record }
    }
    fn restore(&self) -> Result<String, String> {
        restore(&self.record, &AtomicBool::new(false))
    }
}
#[test]
fn archive_restore_recreates_branch_and_preserves_ignored_files_without_launch() {
    let f = Fixture::new();
    fs::write(f.temp.path().join("ignored"), "retained local data").unwrap();
    assert_eq!(f.restore().unwrap(), "workspace");
    assert_eq!(
        git(f.temp.path(), &["symbolic-ref", "--short", "HEAD"]),
        "workspace"
    );
    assert_eq!(
        git(f.temp.path(), &["rev-parse", "HEAD"]),
        f.record.saved.clone().unwrap()
    );
    assert_eq!(
        fs::read_to_string(f.temp.path().join("ignored")).unwrap(),
        "retained local data"
    );
    assert_eq!(f.restore().unwrap(), "workspace");
}

#[test]
fn archive_restore_resolves_reused_names_without_overwriting_even_same_tip_refs() {
    let f = Fixture::new();
    git(f.temp.path(), &["branch", "workspace"]);
    let collision = format!("rat-restored-{}", f.record.request);
    git(f.temp.path(), &["branch", &collision]);
    let branch = f.restore().unwrap();
    assert_ne!(branch, "workspace");
    assert_ne!(branch, collision);
    assert_eq!(
        git(f.temp.path(), &["rev-parse", "workspace"]),
        f.record.saved.clone().unwrap()
    );
    assert_eq!(
        git(f.temp.path(), &["rev-parse", &collision]),
        f.record.saved.clone().unwrap()
    );
    assert_eq!(f.restore().unwrap(), branch);
}

#[test]
fn archive_restore_rejects_in_progress_git_operation_before_creating_refs() {
    let f = Fixture::new();
    fs::write(
        f.record.checkout.git_dir.join("MERGE_HEAD"),
        format!("{}\n", f.record.saved.as_deref().unwrap()),
    )
    .unwrap();
    assert!(f.restore().unwrap_err().contains("finish or abort"));
    assert_eq!(
        git(
            f.temp.path(),
            &["for-each-ref", "--format=%(refname)", "refs/heads"]
        ),
        ""
    );
}

#[test]
fn archive_restore_hook_edits_preserve_detached_checkout_and_created_branch() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    let hook = f
        .record
        .checkout
        .common_dir
        .join("hooks/reference-transaction");
    fs::write(
        &hook,
        "#!/bin/sh\nif test \"$1\" = prepared; then printf 'hook work\\n' > tracked; fi\n",
    )
    .unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(f.restore().is_err());
    assert_eq!(
        git(
            f.temp.path(),
            &["rev-parse", "--symbolic-full-name", "HEAD"]
        ),
        "HEAD"
    );
    assert_eq!(
        fs::read_to_string(f.temp.path().join("tracked")).unwrap(),
        "hook work\n"
    );
    assert_eq!(
        git(f.temp.path(), &["rev-parse", "workspace"]),
        f.record.saved.clone().unwrap()
    );
}

#[test]
fn archive_restore_survives_gc_and_reconciles_lost_switch_acknowledgement() {
    let f = Fixture::new();
    git(
        f.temp.path(),
        &["reflog", "expire", "--expire=now", "--all"],
    );
    git(f.temp.path(), &["gc", "--prune=now"]);
    let branch = f.restore().unwrap();
    // Saved branch and operation marker are durable Git facts, not a reply cache.
    git(f.temp.path(), &["switch", "--detach"]);
    let restored_record: Record =
        serde_json::from_str(&serde_json::to_string(&f.record).unwrap()).unwrap();
    assert_eq!(
        restore(&restored_record, &AtomicBool::new(false)).unwrap(),
        branch
    );
    assert_eq!(git(f.temp.path(), &["rev-list", "--count", "HEAD"]), "1");
    assert_eq!(
        git(f.temp.path(), &["show", "HEAD:tracked"]),
        "archived work"
    );
}

#[test]
fn archive_restore_preserves_user_hook_rejection_without_creating_branch() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    let hook = f
        .record
        .checkout
        .common_dir
        .join("hooks/reference-transaction");
    fs::write(&hook, "#!/bin/sh\ntest \"$1\" != prepared\n").unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(
        f.restore()
            .unwrap_err()
            .contains("Branch creation failed or outcome uncertain")
    );
    assert_eq!(
        git(
            f.temp.path(),
            &["for-each-ref", "--format=%(refname)", "refs/heads"]
        ),
        ""
    );
    fs::remove_file(hook).unwrap();
    assert_eq!(f.restore().unwrap(), "workspace");
}

#[test]
fn archive_restore_reconciles_created_branch_when_ordinary_switch_refuses_other_checkout() {
    let f = Fixture::new();
    assert_eq!(f.restore().unwrap(), "workspace");
    git(f.temp.path(), &["switch", "--detach"]);
    let other = f.temp.path().join("other-checkout");
    // Keep another checkout outside this worktree so it is not an untracked file.
    let other_temp = tempfile::tempdir().unwrap();
    let other = other_temp.path().join(other.file_name().unwrap());
    git(
        f.temp.path(),
        &["worktree", "add", other.to_str().unwrap(), "workspace"],
    );
    assert!(
        f.restore()
            .unwrap_err()
            .contains("created and retained; switch failed")
    );
    assert_eq!(
        git(
            f.temp.path(),
            &["rev-parse", "--symbolic-full-name", "HEAD"]
        ),
        "HEAD"
    );
    assert_eq!(git(&other, &["show", "HEAD:tracked"]), "archived work");
    git(
        f.temp.path(),
        &["worktree", "remove", other.to_str().unwrap()],
    );
    assert_eq!(f.restore().unwrap(), "workspace");
}

#[test]
fn archive_restore_refuses_missing_folder_changed_recovery_and_external_work() {
    let f = Fixture::new();
    fs::write(f.temp.path().join("new"), "external unsaved").unwrap();
    assert!(f.restore().unwrap_err().contains("Unsaved files"));
    fs::remove_file(f.temp.path().join("new")).unwrap();
    fs::write(f.temp.path().join("tracked"), "external commit\n").unwrap();
    git(f.temp.path(), &["commit", "-am", "external"]);
    let external = git(f.temp.path(), &["rev-parse", "HEAD"]);
    assert!(
        f.restore()
            .unwrap_err()
            .contains("HEAD advanced or changed")
    );
    assert_eq!(git(f.temp.path(), &["rev-parse", "HEAD"]), external);
    git(
        f.temp.path(),
        &["update-ref", &f.record.reference, &external],
    );
    assert!(f.restore().unwrap_err().contains("recovery reference"));
    fs::remove_dir_all(f.temp.path()).unwrap();
    assert!(f.restore().unwrap_err().contains("unavailable"));
}

#[test]
fn archive_restore_cancellation_after_ref_commit_reconciles_same_created_branch() {
    use std::{
        os::unix::fs::PermissionsExt,
        sync::{Arc, atomic::Ordering},
        time::{Duration, Instant},
    };
    let f = Fixture::new();
    let hook = f
        .record
        .checkout
        .common_dir
        .join("hooks/reference-transaction");
    fs::write(&hook, "#!/bin/sh\nif test \"$1\" = committed; then printf ready > .git/restore-created; sleep 15; fi\n").unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    let cancel = Arc::new(AtomicBool::new(false));
    let flag = cancel.clone();
    let record = f.record.clone();
    let worker = std::thread::spawn(move || restore(&record, &flag));
    let deadline = Instant::now() + Duration::from_secs(5);
    while !f
        .record
        .checkout
        .common_dir
        .join("restore-created")
        .exists()
    {
        assert!(Instant::now() < deadline, "Ref commit hook did not run");
        std::thread::sleep(Duration::from_millis(10));
    }
    cancel.store(true, Ordering::Release);
    assert!(
        worker
            .join()
            .unwrap()
            .unwrap_err()
            .contains("outcome uncertain")
    );
    fs::remove_file(hook).unwrap();
    assert_eq!(
        git(
            f.temp.path(),
            &["rev-parse", "--symbolic-full-name", "HEAD"]
        ),
        "HEAD"
    );
    assert_eq!(f.restore().unwrap(), "workspace");
    assert_eq!(
        git(
            f.temp.path(),
            &["for-each-ref", "--format=%(refname)", "refs/heads"]
        ),
        "refs/heads/workspace"
    );
}
