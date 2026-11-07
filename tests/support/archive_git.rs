use crate::archive::{Job, Phase, Record};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::AtomicBool,
    time::Duration,
};
use tempfile::TempDir;

struct Repo {
    _temp: TempDir,
    root: PathBuf,
}
impl Repo {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("repo");
        fs::create_dir(&root).unwrap();
        git(&root, &["init", "-b", "workspace"]);
        git(&root, &["config", "user.name", "Archive Test"]);
        git(&root, &["config", "user.email", "archive@example.invalid"]);
        fs::write(root.join("tracked"), "base\n").unwrap();
        fs::write(root.join(".gitignore"), "ignored\n").unwrap();
        git(&root, &["add", "."]);
        git(&root, &["commit", "-m", "base"]);
        Self { _temp: temp, root }
    }
    fn record(&self) -> Record {
        let identity = crate::git::resolve_checkout(&self.root, &AtomicBool::new(false)).unwrap();
        let workspace = crate::registry::Workspace {
            creation: None,
            id: uuid::Uuid::new_v4().to_string(),
            task: "test".into(),
            legacy_tasks: vec![],
            name: "Archive fixture".into(),
            project: None,
            path: None,
            checkout: Some(identity),
            sessions: vec![],
        };
        Record::new(&workspace, &uuid::Uuid::new_v4().to_string()).unwrap()
    }
    fn file(&self, path: &str, bytes: &str) {
        fs::write(self.root.join(path), bytes).unwrap()
    }
    fn read(&self, path: &str) -> String {
        fs::read_to_string(self.root.join(path)).unwrap()
    }
    fn prepare(&self) -> Record {
        work(self.record()).unwrap()
    }
}
fn git(root: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim().into()
}
fn work(record: Record) -> Result<Record, String> {
    let job = Job::start(record).unwrap();
    job.result.recv_timeout(Duration::from_secs(40)).unwrap()
}

#[test]
fn archive_job_preserves_staged_only_work_and_combines_partial_staging() {
    let repo = Repo::new();
    repo.file("tracked", "staged-only\n");
    git(&repo.root, &["add", "tracked"]);
    repo.file("tracked", "working\n");
    repo.file("new", "new file\n");
    repo.file("ignored", "secret\n");
    let before = git(&repo.root, &["rev-parse", "HEAD"]);
    let prepared = repo.prepare();
    let result = work(prepared).unwrap();
    assert_eq!(result.phase, Phase::Saved);
    assert_ne!(result.saved.as_deref(), Some(before.as_str()));
    assert_eq!(git(&repo.root, &["show", "HEAD:tracked"]), "working");
    assert_eq!(git(&repo.root, &["show", "HEAD:new"]), "new file");
    assert_eq!(
        git(
            &repo.root,
            &["show", &format!("{}-index:tracked", result.reference)]
        ),
        "staged-only"
    );
    assert_eq!(git(&repo.root, &["status", "--porcelain"]), "");
    assert_eq!(repo.read("ignored"), "secret\n");
    assert_eq!(result.excluded, vec![b"ignored".to_vec()]);
    assert!(result.message.contains("saved locally, not pushed"));
}

#[test]
fn archive_job_clean_head_creates_no_commit_and_lost_reply_reconciles() {
    let repo = Repo::new();
    let before = git(&repo.root, &["rev-parse", "HEAD"]);
    // Staged-only content needs recovery even when working files equal HEAD.
    repo.file("tracked", "staged-only\n");
    git(&repo.root, &["add", "tracked"]);
    repo.file("tracked", "base\n");
    let prepared = repo.prepare();
    let result = work(prepared.clone()).unwrap();
    assert_eq!(result.saved.as_deref(), Some(before.as_str()));
    assert_eq!(git(&repo.root, &["rev-list", "--count", "HEAD"]), "1");
    let reconciled = work(prepared).unwrap();
    assert_eq!(reconciled.saved, result.saved);
    assert_eq!(git(&repo.root, &["status", "--porcelain"]), "");
    assert_eq!(
        git(
            &repo.root,
            &["show", &format!("{}-index:tracked", result.reference)]
        ),
        "staged-only"
    );
}

#[test]
fn archive_job_hook_failure_preserves_real_index_and_retries_original_snapshot() {
    use std::os::unix::fs::PermissionsExt;
    let repo = Repo::new();
    repo.file("tracked", "stage\n");
    git(&repo.root, &["add", "tracked"]);
    repo.file("tracked", "working\n");
    let original = fs::read(repo.root.join(".git/index")).unwrap();
    let before = git(&repo.root, &["rev-parse", "HEAD"]);
    let hook = repo.root.join(".git/hooks/pre-commit");
    fs::write(
        &hook,
        "#!/bin/sh\nprintf injected > hook-only\ngit add hook-only\nexit 1\n",
    )
    .unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    let prepared = repo.prepare();
    assert!(
        work(prepared.clone())
            .unwrap_err()
            .contains("Checkpoint command failed")
    );
    assert_eq!(fs::read(repo.root.join(".git/index")).unwrap(), original);
    assert_eq!(git(&repo.root, &["rev-parse", "HEAD"]), before);
    fs::remove_file(&hook).unwrap();
    fs::remove_file(repo.root.join("hook-only")).unwrap();
    let result = work(prepared).unwrap();
    assert_eq!(git(&repo.root, &["show", "HEAD:tracked"]), "working");
    assert_eq!(git(&repo.root, &["status", "--porcelain"]), "");
    assert!(result.saved.is_some());
}

#[test]
fn archive_job_saved_checkpoint_survives_gc_and_lost_reply_without_duplicate_commit() {
    let repo = Repo::new();
    repo.file("tracked", "staged-only\n");
    git(&repo.root, &["add", "tracked"]);
    repo.file("tracked", "working\n");
    let prepared = repo.prepare();
    let result = work(prepared.clone()).unwrap();
    let retry = work(prepared).unwrap();
    assert_eq!(retry.saved, result.saved);
    assert_eq!(git(&repo.root, &["rev-list", "--count", "HEAD"]), "2");
    git(
        &repo.root,
        &["checkout", "--detach", result.before.as_deref().unwrap()],
    );
    git(&repo.root, &["branch", "-D", "workspace"]);
    fs::remove_dir_all(repo.root.join(".git/rat-archive")).unwrap();
    git(&repo.root, &["reflog", "expire", "--expire=now", "--all"]);
    git(&repo.root, &["gc", "--prune=now"]);
    assert_eq!(
        git(
            &repo.root,
            &["show", &format!("{}:tracked", result.reference)]
        ),
        "working"
    );
    assert_eq!(
        git(
            &repo.root,
            &["show", &format!("{}-index:tracked", result.reference)]
        ),
        "staged-only"
    );
}

#[test]
fn archive_job_observed_external_edit_blocks_save_without_overwriting_index() {
    let repo = Repo::new();
    repo.file("tracked", "first\n");
    let prepared = repo.prepare();
    let index = fs::read(repo.root.join(".git/index")).unwrap();
    repo.file("tracked", "later\n");
    assert!(work(prepared).unwrap_err().contains("Files changed"));
    assert_eq!(repo.read("tracked"), "later\n");
    assert_eq!(fs::read(repo.root.join(".git/index")).unwrap(), index);
    assert_eq!(git(&repo.root, &["rev-list", "--count", "HEAD"]), "1");
}

#[test]
fn archive_job_external_staging_blocks_save_and_preserves_unrelated_work() {
    let repo = Repo::new();
    repo.file("tracked", "first\n");
    let prepared = repo.prepare();
    repo.file("unrelated", "external staged\n");
    git(&repo.root, &["add", "unrelated"]);
    let index = fs::read(repo.root.join(".git/index")).unwrap();
    assert!(work(prepared).unwrap_err().contains("Index changed"));
    assert_eq!(fs::read(repo.root.join(".git/index")).unwrap(), index);
    assert_eq!(git(&repo.root, &["show", ":unrelated"]), "external staged");
}

#[test]
fn archive_job_hook_concurrent_edit_leaves_pinned_checkpoint_and_original_index() {
    use std::os::unix::fs::PermissionsExt;
    let repo = Repo::new();
    repo.file("tracked", "save\n");
    let prepared = repo.prepare();
    let index = fs::read(repo.root.join(".git/index")).unwrap();
    let hook = repo.root.join(".git/hooks/post-commit");
    fs::write(&hook, "#!/bin/sh\nprintf 'hook changed\\n' > tracked\n").unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(
        work(prepared.clone())
            .unwrap_err()
            .contains("files changed during save")
    );
    assert_eq!(
        git(
            &repo.root,
            &["show", &format!("{}:tracked", prepared.reference)]
        ),
        "save"
    );
    assert_eq!(repo.read("tracked"), "hook changed\n");
    assert_eq!(fs::read(repo.root.join(".git/index")).unwrap(), index);
}

#[test]
fn archive_job_missing_identity_and_nested_repository_fail_without_file_loss() {
    let repo = Repo::new();
    repo.file("tracked", "save\n");
    git(&repo.root, &["config", "user.name", ""]);
    git(&repo.root, &["config", "user.email", ""]);
    let prepared = repo.prepare();
    assert!(
        work(prepared)
            .unwrap_err()
            .contains("Checkpoint command failed")
    );
    assert_eq!(git(&repo.root, &["rev-list", "--count", "HEAD"]), "1");
    assert_eq!(repo.read("tracked"), "save\n");
    let nested = repo.root.join("nested");
    fs::create_dir(&nested).unwrap();
    git(&nested, &["init"]);
    fs::write(nested.join("new"), "nested secret").unwrap();
    assert!(
        work(repo.record())
            .unwrap_err()
            .contains("Nested repository")
    );
    assert_eq!(
        fs::read_to_string(nested.join("new")).unwrap(),
        "nested secret"
    );
}

#[test]
fn archive_job_conflicted_index_is_rejected_unchanged() {
    let repo = Repo::new();
    git(&repo.root, &["checkout", "-b", "other"]);
    repo.file("tracked", "other\n");
    git(&repo.root, &["commit", "-am", "other"]);
    git(&repo.root, &["checkout", "workspace"]);
    repo.file("tracked", "workspace\n");
    git(&repo.root, &["commit", "-am", "workspace"]);
    let output = Command::new("git")
        .arg("-C")
        .arg(&repo.root)
        .args(["merge", "other"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let before = fs::read(repo.root.join(".git/index")).unwrap();
    assert!(work(repo.record()).unwrap_err().contains("MERGE_HEAD"));
    assert_eq!(fs::read(repo.root.join(".git/index")).unwrap(), before);
    assert!(repo.read("tracked").contains("<<<<<<<"));
}

#[test]
fn archive_job_hook_altered_checkpoint_identity_does_not_claim_success() {
    use std::os::unix::fs::PermissionsExt;
    let repo = Repo::new();
    repo.file("tracked", "save\n");
    let prepared = repo.prepare();
    let index = fs::read(repo.root.join(".git/index")).unwrap();
    let hook = repo.root.join(".git/hooks/commit-msg");
    fs::write(&hook, "#!/bin/sh\nprintf 'rewritten by hook\\n' > \"$1\"\n").unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(
        work(prepared.clone())
            .unwrap_err()
            .contains("checkpoint identity")
    );
    assert_eq!(
        git(&repo.root, &["show", "-s", "--format=%B", "HEAD"]),
        "rewritten by hook"
    );
    assert_eq!(fs::read(repo.root.join(".git/index")).unwrap(), index);
    assert_eq!(
        git(&repo.root, &["rev-parse", &prepared.reference]),
        git(&repo.root, &["rev-parse", "HEAD"])
    );
    assert!(work(prepared).is_err());
    assert_eq!(git(&repo.root, &["rev-list", "--count", "HEAD"]), "2");
}

#[test]
fn archive_job_preserves_raw_filenames_deletions_and_binary_content() {
    use std::os::unix::ffi::OsStringExt;
    let repo = Repo::new();
    let raw = std::ffi::OsString::from_vec(b"raw-\nfile\t name".to_vec());
    fs::write(repo.root.join(&raw), [0u8, 255, 17, 10]).unwrap();
    fs::remove_file(repo.root.join("tracked")).unwrap();
    let result = work(repo.prepare()).unwrap();
    let restored = repo._temp.path().join("restored");
    fs::create_dir(&restored).unwrap();
    let mut cmd = Command::new("git");
    let output = cmd
        .arg("-C")
        .arg(&repo.root)
        .arg("--work-tree")
        .arg(&restored)
        .args(["checkout", result.saved.as_deref().unwrap(), "--", "."])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(fs::read(restored.join(raw)).unwrap(), [0u8, 255, 17, 10]);
    assert!(!restored.join("tracked").exists());
    assert_eq!(git(&repo.root, &["status", "--porcelain"]), "");
}

#[test]
fn archive_job_refuses_split_and_sparse_index_without_altering_it() {
    let repo = Repo::new();
    git(&repo.root, &["update-index", "--split-index"]);
    let bytes = fs::read(repo.root.join(".git/index")).unwrap();
    assert!(work(repo.record()).unwrap_err().contains("Split index"));
    assert_eq!(fs::read(repo.root.join(".git/index")).unwrap(), bytes);
    git(&repo.root, &["update-index", "--no-split-index"]);
    git(&repo.root, &["update-index", "--skip-worktree", "tracked"]);
    let bytes = fs::read(repo.root.join(".git/index")).unwrap();
    assert!(
        work(repo.record())
            .unwrap_err()
            .contains("Sparse/skip-worktree")
    );
    assert_eq!(fs::read(repo.root.join(".git/index")).unwrap(), bytes);
}

#[test]
fn archive_job_cancel_after_git_commit_reconciles_partial_success_without_duplicate() {
    use std::os::unix::fs::PermissionsExt;
    let repo = Repo::new();
    repo.file("tracked", "saved before cancellation\n");
    let prepared = repo.prepare();
    let original_index = fs::read(repo.root.join(".git/index")).unwrap();
    let hook = repo.root.join(".git/hooks/post-commit");
    fs::write(
        &hook,
        "#!/bin/sh\nprintf ready > .git/commit-reached\nsleep 15\n",
    )
    .unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    let job = Job::start(prepared.clone()).unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !repo.root.join(".git/commit-reached").exists() {
        assert!(
            std::time::Instant::now() < deadline,
            "Commit hook did not start"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    job.cancel();
    assert!(
        job.result
            .recv_timeout(Duration::from_secs(3))
            .unwrap()
            .unwrap_err()
            .contains("cancelled")
    );
    assert_eq!(
        fs::read(repo.root.join(".git/index")).unwrap(),
        original_index
    );
    assert!(!repo.root.join(".git/index.lock").exists());
    let saved = git(&repo.root, &["rev-parse", "HEAD"]);
    assert_eq!(git(&repo.root, &["rev-list", "--count", "HEAD"]), "2");
    fs::remove_file(hook).unwrap();
    let result = work(prepared).unwrap();
    assert_eq!(result.saved.as_deref(), Some(saved.as_str()));
    assert_eq!(git(&repo.root, &["status", "--porcelain"]), "");
    assert_eq!(git(&repo.root, &["rev-list", "--count", "HEAD"]), "2");
}

#[test]
fn archive_job_empty_head_without_index_is_clean_and_reconciles() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("empty");
    fs::create_dir(&root).unwrap();
    git(&root, &["init", "-b", "workspace"]);
    git(&root, &["config", "user.name", "Archive Test"]);
    git(&root, &["config", "user.email", "archive@example.invalid"]);
    git(&root, &["commit", "--allow-empty", "-m", "empty base"]);
    if root.join(".git/index").exists() {
        fs::remove_file(root.join(".git/index")).unwrap()
    }
    let repo = Repo { _temp: temp, root };
    let before = git(&repo.root, &["rev-parse", "HEAD"]);
    let prepared = repo.prepare();
    let saved = work(prepared.clone()).unwrap();
    assert_eq!(saved.saved.as_deref(), Some(before.as_str()));
    assert_eq!(work(prepared).unwrap().saved, saved.saved);
    assert_eq!(git(&repo.root, &["rev-list", "--count", "HEAD"]), "1");
    assert_eq!(git(&repo.root, &["status", "--porcelain"]), "");
}

#[test]
fn archive_job_intent_to_add_is_explicitly_refused_without_changing_index() {
    let repo = Repo::new();
    repo.file("intent", "content not staged\n");
    git(&repo.root, &["add", "-N", "intent"]);
    let bytes = fs::read(repo.root.join(".git/index")).unwrap();
    assert!(work(repo.record()).unwrap_err().contains("Intent-to-add"));
    assert_eq!(fs::read(repo.root.join(".git/index")).unwrap(), bytes);
    assert_eq!(repo.read("intent"), "content not staged\n");
}
