//! Real Git checks at the public branch cleanup seam.
use crate::archive::{Phase, Record};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::AtomicBool,
};

struct Repo {
    _temp: tempfile::TempDir,
    root: PathBuf,
}
impl Repo {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("repo");
        fs::create_dir(&root).unwrap();
        git(&root, &["init", "-b", "work"]);
        git(&root, &["config", "user.name", "Cleanup fixture"]);
        git(&root, &["config", "user.email", "cleanup@example.invalid"]);
        fs::write(root.join("tracked"), "saved\n").unwrap();
        git(&root, &["add", "."]);
        git(&root, &["commit", "-m", "saved"]);
        Self { _temp: temp, root }
    }
    fn record(&self) -> Record {
        let w = crate::registry::Workspace {
            creation: None,
            id: uuid::Uuid::new_v4().to_string(),
            task: "cleanup fixture".into(),
            legacy_tasks: vec![],
            name: "Cleanup fixture".into(),
            project: None,
            path: None,
            checkout: Some(
                crate::git::resolve_checkout(&self.root, &AtomicBool::new(false)).unwrap(),
            ),
            sessions: vec![],
        };
        let mut record = Record::new(&w, &uuid::Uuid::new_v4().to_string()).unwrap();
        record.phase = Phase::Archived;
        record.branch = Some("refs/heads/work".into());
        record.delete_target = record.branch.clone();
        record.saved = Some(git(&self.root, &["rev-parse", "HEAD"]));
        git(
            &self.root,
            &[
                "update-ref",
                &record.reference,
                record.saved.as_deref().unwrap(),
            ],
        );
        record
    }
    fn tip(&self) -> String {
        git(&self.root, &["rev-parse", "refs/heads/work"])
    }
    fn cleanup(&self, r: &Record) -> Result<String, String> {
        super::cleanup(r, &AtomicBool::new(false))
    }
    fn advance(&self) -> String {
        fs::write(self.root.join("tracked"), "external\n").unwrap();
        git(&self.root, &["commit", "-am", "external"]);
        git(&self.root, &["rev-parse", "HEAD"])
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

#[test]
fn externally_advanced_branch_is_retained_with_new_commits() {
    let repo = Repo::new();
    let r = repo.record();
    let new = repo.advance();
    assert!(repo.cleanup(&r).unwrap_err().contains("advanced/replaced"));
    assert_eq!(repo.tip(), new);
    assert_eq!(
        git(&repo.root, &["rev-parse", &r.reference]),
        r.saved.unwrap()
    );
}

#[test]
fn creation_provenance_refuses_renamed_or_switched_branch_target() {
    let repo = Repo::new();
    let mut r = repo.record();
    r.owned_branch = Some("refs/heads/original-product-branch".into());
    assert!(repo.cleanup(&r).unwrap_err().contains("provenance"));
    assert_eq!(repo.tip(), r.saved.unwrap());
    assert_eq!(
        git(&repo.root, &["symbolic-ref", "HEAD"]),
        "refs/heads/work"
    );
}

#[test]
fn protected_main_master_trunk_and_selected_base_are_retained() {
    for branch in ["main", "master", "trunk"] {
        let repo = Repo::new();
        git(&repo.root, &["branch", "-m", branch]);
        let mut r = repo.record();
        r.branch = Some(format!("refs/heads/{branch}"));
        r.delete_target = r.branch.clone();
        assert!(repo.cleanup(&r).unwrap_err().contains("protected"));
        assert_eq!(
            git(&repo.root, &["symbolic-ref", "HEAD"]),
            r.branch.unwrap()
        );
    }
    for base in ["work", "refs/heads/work", "refs/remotes/origin/work"] {
        let repo = Repo::new();
        let mut r = repo.record();
        r.protected_base = Some(base.into());
        assert!(repo.cleanup(&r).unwrap_err().contains("protected"));
        assert_eq!(repo.tip(), r.saved.unwrap());
    }
}

#[test]
fn remote_default_branch_name_is_protected_without_touching_remote_refs() {
    let repo = Repo::new();
    let r = repo.record();
    let saved = r.saved.as_deref().unwrap();
    git(
        &repo.root,
        &["update-ref", "refs/remotes/origin/work", saved],
    );
    git(
        &repo.root,
        &[
            "symbolic-ref",
            "refs/remotes/origin/HEAD",
            "refs/remotes/origin/work",
        ],
    );
    assert!(repo.cleanup(&r).unwrap_err().contains("default branch"));
    assert_eq!(repo.tip(), saved);
    assert_eq!(
        git(&repo.root, &["rev-parse", "refs/remotes/origin/work"]),
        saved
    );
}

#[test]
fn branch_used_elsewhere_is_retained_after_safe_detach() {
    let repo = Repo::new();
    let r = repo.record();
    if !git(&repo.root, &["--version"]).starts_with("git version 2.50.") {
        assert!(repo.cleanup(&r).unwrap_err().contains("qualified only"));
        assert_eq!(repo.tip(), r.saved.unwrap());
        return;
    }
    let other = repo._temp.path().join("other");
    git(
        &repo.root,
        &[
            "worktree",
            "add",
            "--force",
            other.to_str().unwrap(),
            "work",
        ],
    );
    assert!(repo.cleanup(&r).unwrap_err().contains("another worktree"));
    assert_eq!(repo.tip(), r.saved.clone().unwrap());
    assert_eq!(git(&other, &["symbolic-ref", "HEAD"]), "refs/heads/work");
    assert_eq!(
        fs::read_to_string(other.join("tracked")).unwrap(),
        "saved\n"
    );
}

#[test]
fn custom_hooks_and_reference_transaction_hook_are_preserved_and_not_bypassed() {
    let repo = Repo::new();
    let r = repo.record();
    let hook = repo.root.join(".git/hooks/reference-transaction");
    let contents = b"#!/bin/sh\nprintf invoked > hook-ran\nexit 1\n";
    fs::write(&hook, contents).unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(repo.cleanup(&r).unwrap_err().contains("hook preserved"));
    assert_eq!(fs::read(&hook).unwrap(), contents);
    assert!(!repo.root.join("hook-ran").exists());
    assert_eq!(repo.tip(), r.saved.clone().unwrap());
    fs::remove_file(&hook).unwrap();
    git(
        &repo.root,
        &["config", "core.hooksPath", "relative-custom-hooks"],
    );
    assert!(repo.cleanup(&r).unwrap_err().contains("hooks preserved"));
    assert_eq!(
        git(&repo.root, &["config", "--get", "core.hooksPath"]),
        "relative-custom-hooks"
    );
    assert_eq!(repo.tip(), r.saved.unwrap());
}

#[test]
fn absent_or_changed_recovery_prevents_branch_detach_or_deletion() {
    let repo = Repo::new();
    let r = repo.record();
    git(&repo.root, &["update-ref", "-d", &r.reference]);
    assert!(repo.cleanup(&r).is_err());
    assert_eq!(repo.tip(), r.saved.clone().unwrap());
    assert_eq!(
        git(&repo.root, &["symbolic-ref", "HEAD"]),
        "refs/heads/work"
    );
    let changed = repo.advance();
    git(&repo.root, &["update-ref", &r.reference, &changed]);
    assert!(
        repo.cleanup(&r)
            .unwrap_err()
            .contains("recovery ref changed")
    );
    assert_eq!(repo.tip(), changed);
}

#[test]
fn external_unstaged_staged_and_new_files_block_detach_and_remain_intact() {
    for mode in ["unstaged", "staged", "new"] {
        let repo = Repo::new();
        let r = repo.record();
        let path = if mode == "new" { "new" } else { "tracked" };
        fs::write(repo.root.join(path), "external contents\n").unwrap();
        if mode == "staged" {
            git(&repo.root, &["add", path]);
        }
        let index = fs::read(repo.root.join(".git/index")).unwrap();
        assert!(repo.cleanup(&r).unwrap_err().contains("Unsaved files"));
        assert_eq!(fs::read(repo.root.join(".git/index")).unwrap(), index);
        assert_eq!(
            fs::read_to_string(repo.root.join(path)).unwrap(),
            "external contents\n"
        );
        assert_eq!(repo.tip(), r.saved.unwrap());
        assert_eq!(
            git(&repo.root, &["symbolic-ref", "HEAD"]),
            "refs/heads/work"
        );
    }
}

#[test]
fn packed_branch_cleanup_and_lost_reply_retry_preserve_recovery_and_replacement() {
    let repo = Repo::new();
    let r = repo.record();
    if !git(&repo.root, &["--version"]).starts_with("git version 2.50.") {
        assert!(repo.cleanup(&r).unwrap_err().contains("qualified only"));
        assert_eq!(repo.tip(), r.saved.unwrap());
        return;
    }
    git(&repo.root, &["pack-refs", "--all", "--prune"]);
    repo.cleanup(&r).unwrap();
    let uncertain = repo.cleanup(&r).unwrap_err();
    assert!(uncertain.contains("Branch absent") && uncertain.contains("uncertain"));
    assert_eq!(
        git(&repo.root, &["rev-parse", &r.reference]),
        r.saved.clone().unwrap()
    );
    git(&repo.root, &["switch", "-c", "external"]);
    let replacement = repo.advance();
    git(&repo.root, &["branch", "work", &replacement]);
    assert!(repo.cleanup(&r).unwrap_err().contains("advanced/replaced"));
    assert_eq!(repo.tip(), replacement);
    assert_eq!(
        git(&repo.root, &["symbolic-ref", "HEAD"]),
        "refs/heads/external"
    );
}

#[test]
fn prepared_not_durable_archive_and_unselected_deletion_do_not_mutate_branch() {
    let repo = Repo::new();
    let mut r = repo.record();
    r.phase = Phase::Saved;
    assert!(repo.cleanup(&r).unwrap_err().contains("Durable archive"));
    r.phase = Phase::Archived;
    r.delete_target = None;
    assert!(repo.cleanup(&r).unwrap_err().contains("not selected"));
    assert_eq!(repo.tip(), r.saved.unwrap());
}

#[test]
fn post_checkout_hook_external_advance_is_preserved_after_detach() {
    let repo = Repo::new();
    let r = repo.record();
    if !git(&repo.root, &["--version"]).starts_with("git version 2.50.") {
        assert!(repo.cleanup(&r).unwrap_err().contains("qualified only"));
        assert_eq!(repo.tip(), r.saved.unwrap());
        return;
    }
    git(
        &repo.root,
        &["commit", "--allow-empty", "-m", "external commit"],
    );
    let external = git(&repo.root, &["rev-parse", "HEAD"]);
    git(
        &repo.root,
        &["reset", "--hard", r.saved.as_deref().unwrap()],
    );
    let hook = repo.root.join(".git/hooks/post-checkout");
    fs::write(
        &hook,
        format!("#!/bin/sh\ngit update-ref refs/heads/work {external}\n"),
    )
    .unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(
        repo.cleanup(&r)
            .unwrap_err()
            .contains("changed after detach")
    );
    assert_eq!(repo.tip(), external);
    assert_eq!(
        git(&repo.root, &["rev-parse", "HEAD"]),
        r.saved.clone().unwrap()
    );
    assert_eq!(
        git(&repo.root, &["rev-parse", &r.reference]),
        r.saved.unwrap()
    );
    assert!(hook.exists());
}

#[test]
fn post_checkout_hook_installing_reference_transaction_policy_is_not_bypassed() {
    let repo = Repo::new();
    let r = repo.record();
    if !git(&repo.root, &["--version"]).starts_with("git version 2.50.") {
        assert!(repo.cleanup(&r).unwrap_err().contains("qualified only"));
        assert_eq!(repo.tip(), r.saved.unwrap());
        return;
    }
    let post_checkout = repo.root.join(".git/hooks/post-checkout");
    fs::write(
        &post_checkout,
        "#!/bin/sh\nprintf '#!/bin/sh\\nexit 1\\n' > .git/hooks/reference-transaction\nchmod +x .git/hooks/reference-transaction\n",
    ).unwrap();
    fs::set_permissions(&post_checkout, fs::Permissions::from_mode(0o755)).unwrap();
    let result = repo.cleanup(&r);
    assert!(
        result.is_err(),
        "new user transaction policy was bypassed: {result:?}"
    );
    assert_eq!(repo.tip(), r.saved.unwrap());
    assert!(repo.root.join(".git/hooks/reference-transaction").exists());
}

#[test]
fn post_checkout_hook_installing_custom_hooks_path_is_not_bypassed() {
    let repo = Repo::new();
    let r = repo.record();
    if !git(&repo.root, &["--version"]).starts_with("git version 2.50.") {
        assert!(repo.cleanup(&r).unwrap_err().contains("qualified only"));
        assert_eq!(repo.tip(), r.saved.unwrap());
        return;
    }
    let post_checkout = repo.root.join(".git/hooks/post-checkout");
    fs::write(
        &post_checkout,
        "#!/bin/sh\ngit config core.hooksPath external-policy-hooks\n",
    )
    .unwrap();
    fs::set_permissions(&post_checkout, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(repo.cleanup(&r).unwrap_err().contains("hooks preserved"));
    assert_eq!(repo.tip(), r.saved.unwrap());
    assert_eq!(
        git(&repo.root, &["config", "--get", "core.hooksPath"]),
        "external-policy-hooks"
    );
}

#[test]
fn post_checkout_hook_changing_remote_default_to_target_retains_branch_and_default() {
    let repo = Repo::new();
    let r = repo.record();
    if !git(&repo.root, &["--version"]).starts_with("git version 2.50.") {
        assert!(repo.cleanup(&r).unwrap_err().contains("qualified only"));
        assert_eq!(repo.tip(), r.saved.unwrap());
        return;
    }
    let saved = r.saved.as_deref().unwrap();
    git(
        &repo.root,
        &["update-ref", "refs/remotes/origin/work", saved],
    );
    git(
        &repo.root,
        &["update-ref", "refs/remotes/origin/base", saved],
    );
    git(
        &repo.root,
        &[
            "symbolic-ref",
            "refs/remotes/origin/HEAD",
            "refs/remotes/origin/base",
        ],
    );
    let post_checkout = repo.root.join(".git/hooks/post-checkout");
    fs::write(
        &post_checkout,
        "#!/bin/sh\ngit symbolic-ref refs/remotes/origin/HEAD refs/remotes/origin/work\n",
    )
    .unwrap();
    fs::set_permissions(&post_checkout, fs::Permissions::from_mode(0o755)).unwrap();
    let result = repo.cleanup(&r);
    assert!(
        result.is_err(),
        "new default branch was deleted: {result:?}"
    );
    assert_eq!(repo.tip(), saved);
    assert_eq!(
        git(&repo.root, &["symbolic-ref", "refs/remotes/origin/HEAD"]),
        "refs/remotes/origin/work"
    );
    assert_eq!(
        git(&repo.root, &["rev-parse", "refs/remotes/origin/work"]),
        saved
    );
    assert_eq!(git(&repo.root, &["rev-parse", &r.reference]), saved);
}

#[test]
fn post_checkout_hook_replacing_target_with_same_tip_symbolic_alias_retains_both_refs() {
    let repo = Repo::new();
    let r = repo.record();
    if !git(&repo.root, &["--version"]).starts_with("git version 2.50.") {
        assert!(repo.cleanup(&r).unwrap_err().contains("qualified only"));
        assert_eq!(repo.tip(), r.saved.unwrap());
        return;
    }
    let saved = r.saved.as_deref().unwrap();
    git(&repo.root, &["branch", "external-owner", saved]);
    let post_checkout = repo.root.join(".git/hooks/post-checkout");
    fs::write(
        &post_checkout,
        "#!/bin/sh\ngit symbolic-ref refs/heads/work refs/heads/external-owner\n",
    )
    .unwrap();
    fs::set_permissions(&post_checkout, fs::Permissions::from_mode(0o755)).unwrap();
    let result = repo.cleanup(&r);
    assert!(
        result.is_err(),
        "replacement symbolic branch was deleted: {result:?}"
    );
    assert_eq!(
        git(&repo.root, &["symbolic-ref", "refs/heads/work"]),
        "refs/heads/external-owner"
    );
    assert_eq!(repo.tip(), saved);
    assert_eq!(
        git(&repo.root, &["rev-parse", "refs/heads/external-owner"]),
        saved
    );
    assert_eq!(git(&repo.root, &["rev-parse", &r.reference]), saved);
}
