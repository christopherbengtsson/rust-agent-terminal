//! Identity-bound linked-worktree removal. No saved path is recursively deleted by us.
use crate::git::CheckoutIdentity;
use serde::{Deserialize, Serialize};
use std::sync::atomic::AtomicBool;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Anchor {
    pub device: u64,
    pub inode: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub checkout: CheckoutIdentity,
    pub head: String,
    pub branch: Option<String>,
    pub reference: String,
    pub local_only: bool,
    pub warning: String,
    pub blockers: Vec<String>,
    pub root_anchor: Anchor,
    pub git_anchor: Anchor,
    pub common_anchor: Anchor,
    pub dotgit_anchor: Anchor,
    pub registration_anchor: Anchor,
    pub remote_evidence: String,
}
use std::{
    fs,
    os::unix::{ffi::OsStrExt, fs::MetadataExt},
    path::Path,
    process::Command,
};

fn command(s: &CheckoutIdentity, checkout: bool) -> Command {
    let mut c = Command::new("git");
    if checkout {
        c.arg("-C").arg(&s.root);
    } else {
        c.arg("--git-dir").arg(&s.common_dir);
    }
    c.args([
        "-c",
        "core.fsync=committed,reference",
        "-c",
        "core.fsyncMethod=fsync",
    ])
    .env("GIT_OPTIONAL_LOCKS", "0");
    c
}
fn git(
    s: &CheckoutIdentity,
    checkout: bool,
    args: &[&str],
    cancel: &AtomicBool,
) -> Result<Vec<u8>, String> {
    crate::git::run_git_command(command(s, checkout).args(args), cancel)
}
fn text(
    s: &CheckoutIdentity,
    checkout: bool,
    args: &[&str],
    cancel: &AtomicBool,
) -> Result<String, String> {
    String::from_utf8(git(s, checkout, args, cancel)?)
        .map(|s| s.trim().into())
        .map_err(|_| "Git evidence is not valid UTF-8; removal refused".into())
}
fn anchor(path: &Path, directory: bool) -> Result<Anchor, String> {
    let m = fs::symlink_metadata(path)
        .map_err(|e| format!("Identity unavailable at {}: {e}", path.display()))?;
    if m.file_type().is_symlink() || (directory && !m.is_dir()) || (!directory && !m.is_file()) {
        return Err(format!(
            "Unsafe identity at {}; removal refused",
            path.display()
        ));
    }
    Ok(Anchor {
        device: m.dev(),
        inode: m.ino(),
    })
}
fn read_link(path: &Path) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let file = fs::File::open(path).map_err(|e| format!("Git link unavailable: {e}"))?;
    let mut bytes = Vec::new();
    file.take(16 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("Git link unreadable: {e}"))?;
    if bytes.len() > 16 * 1024 {
        return Err("Git link exceeds safe identity bound; removal refused".into());
    }
    Ok(bytes)
}
fn link_files(s: &CheckoutIdentity) -> Result<(), String> {
    if s.git_dir == s.common_dir
        || s.git_dir.parent() != Some(s.common_dir.join("worktrees").as_path())
    {
        return Err("Main checkout or unsafe linked-worktree identity; removal refused".into());
    }
    let bytes = read_link(&s.root.join(".git"))?;
    let raw = bytes
        .strip_prefix(b"gitdir: ")
        .ok_or("Invalid checkout Git link")?;
    let raw = raw.strip_suffix(b"\n").unwrap_or(raw);
    let path = Path::new(std::ffi::OsStr::from_bytes(raw));
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        s.root.join(path)
    };
    if path.canonicalize().map_err(|e| e.to_string())? != s.git_dir {
        return Err("Checkout link changed; removal refused".into());
    }
    let bytes = read_link(&s.git_dir.join("gitdir"))?;
    let raw = bytes.strip_suffix(b"\n").unwrap_or(&bytes);
    if Path::new(std::ffi::OsStr::from_bytes(raw)) != s.root.join(".git") {
        return Err("Worktree registration path changed; removal refused".into());
    }
    Ok(())
}
/// NUL-delimited registration inspection preserves opaque Unix path bytes.
fn registration(s: &CheckoutIdentity, cancel: &AtomicBool) -> Result<(bool, Vec<String>), String> {
    let bytes = git(s, false, &["worktree", "list", "--porcelain", "-z"], cancel)?;
    let mut found = false;
    let mut target = false;
    let mut blockers = Vec::new();
    for field in bytes.split(|b| *b == 0) {
        if let Some(path) = field.strip_prefix(b"worktree ") {
            target = path == s.root.as_os_str().as_bytes();
            if target {
                if found {
                    return Err("Ambiguous worktree registration; removal refused".into());
                }
                found = true;
            }
        } else if target && (field == b"locked" || field.starts_with(b"locked ")) {
            blockers.push(
                "Worktree locked; explicitly unlock it with Git before previewing removal".into(),
            );
        } else if target && (field == b"prunable" || field.starts_with(b"prunable ")) {
            blockers.push(
                "Worktree prunable or missing; inspect/repair Git registration before removal"
                    .into(),
            );
        } else if target && field == b"bare" {
            blockers.push("Bare repository is not an eligible linked checkout".into());
        }
    }
    Ok((found, blockers))
}
fn display_path(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .chars()
        .flat_map(|c| {
            if c.is_control() {
                c.escape_default().collect::<Vec<_>>()
            } else {
                vec![c]
            }
        })
        .collect()
}
fn operation_blockers(s: &CheckoutIdentity, cancel: &AtomicBool) -> Result<Vec<String>, String> {
    let mut blockers = Vec::new();
    match fs::symlink_metadata(s.common_dir.join("objects/info/alternates")) {
        Ok(_) => blockers.push("External alternate objects are not protected from their owner's Git GC by this recovery ref; make objects local before removal".into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {},
        Err(e) => return Err(format!("Alternate object coverage cannot be inspected: {e}")),
    }
    if std::env::var_os("GIT_ALTERNATE_OBJECT_DIRECTORIES").is_some_and(|v| !v.is_empty()) {
        blockers.push("External alternate object environment makes local recovery coverage uncertain; removal unavailable".into());
    }
    if text(s, true, &["rev-parse", "--is-shallow-repository"], cancel)? != "false" {
        blockers.push("Shallow history has incomplete local commit coverage; obtain complete history before removal".into());
    }
    if !config(s, "extensions.partialClone", cancel)?.is_empty() {
        blockers.push("Partial clone has uncertain local saved-object coverage; make objects local before removal".into());
    }
    for remote in text(s, true, &["remote"], cancel)?.lines().take(8) {
        if text(
            s,
            true,
            &[
                "config",
                "--bool",
                "--default",
                "false",
                "--get",
                &format!("remote.{remote}.promisor"),
            ],
            cancel,
        )? != "false"
        {
            blockers.push("Partial/promisor object coverage cannot guarantee local code recovery; make objects local before removal".into());
        }
    }
    for name in [
        "index.lock",
        "HEAD.lock",
        "MERGE_HEAD",
        "CHERRY_PICK_HEAD",
        "REVERT_HEAD",
        "rebase-merge",
        "rebase-apply",
        "BISECT_LOG",
        "sequencer",
    ] {
        match fs::symlink_metadata(s.git_dir.join(name)) {
            Ok(_) => blockers.push(format!(
                "Git operation/lock {name} is present; finish or abort it before removal"
            )),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("Git operation {name} cannot be checked: {e}")),
        }
    }
    for name in ["packed-refs.lock", "config.lock", "shallow.lock"] {
        match fs::symlink_metadata(s.common_dir.join(name)) {
            Ok(_) => blockers.push(format!(
                "Repository lock {name} is present; finish the Git operation before removal"
            )),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("Repository lock {name} cannot be checked: {e}")),
        }
    }
    if text(
        s,
        true,
        &[
            "config",
            "--bool",
            "--default",
            "false",
            "--get",
            "core.sparseCheckout",
        ],
        cancel,
    )? != "false"
    {
        blockers.push(
            "Sparse index/checkout coverage is unsupported; restore a full checkout before removal"
                .into(),
        );
    }
    for entry in git(s, true, &["ls-files", "-v", "-z"], cancel)?
        .split(|b| *b == 0)
        .filter(|b| !b.is_empty())
    {
        if entry[0] == b'S' || entry[0].is_ascii_lowercase() {
            blockers.push(format!("Special index flag can hide unsaved files: {} — clear assume-unchanged/skip-worktree before removal",display_path(entry.get(2..).unwrap_or(entry))));
        }
    }
    for entry in git(s, true, &["ls-files", "--stage", "-z"], cancel)?
        .split(|b| *b == 0)
        .filter(|b| !b.is_empty())
    {
        if entry.starts_with(b"160000 ") {
            blockers.push(format!(
                "Submodule index entry has unsupported nested-file coverage: {}",
                display_path(entry.split(|b| *b == b'\t').nth(1).unwrap_or(entry))
            ));
        }
    }
    Ok(blockers)
}
fn files(s: &CheckoutIdentity, cancel: &AtomicBool) -> Result<Vec<String>, String> {
    let mut blockers = operation_blockers(s, cancel)?;
    let status = git(
        s,
        true,
        &[
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
            "--ignore-submodules=none",
        ],
        cancel,
    )?;
    for entry in status.split(|b| *b == 0).filter(|b| !b.is_empty()) {
        blockers.push(format!(
            "Unsaved tracked/untracked file: {} — save/commit or move it before removal",
            display_path(entry.get(3..).unwrap_or(entry))
        ));
    }
    // Ordinary `git worktree remove` can silently delete ignored files; status alone
    // does not protect them. Inspect every ignored path independently on both sides
    // of confirmation, without adding it to the index or making a backup claim.
    let ignored = git(
        s,
        true,
        &[
            "ls-files",
            "--others",
            "--ignored",
            "--exclude-standard",
            "-z",
        ],
        cancel,
    )?;
    for entry in ignored.split(|b| *b == 0).filter(|b| !b.is_empty()) {
        blockers.push(format!(
            "Ignored file outside saved commits: {} — move or explicitly delete it before removal",
            display_path(entry)
        ));
    }
    Ok(blockers)
}
fn fingerprint(s: &CheckoutIdentity, bytes: &[u8], cancel: &AtomicBool) -> Result<String, String> {
    use std::io::Write;
    let mut file = tempfile::NamedTempFile::new().map_err(|e| e.to_string())?;
    file.write_all(bytes).map_err(|e| e.to_string())?;
    let output = crate::git::run_git_command(
        command(s, false)
            .arg("hash-object")
            .arg("--")
            .arg(file.path()),
        cancel,
    )?;
    String::from_utf8(output)
        .map(|s| s.trim().into())
        .map_err(|_| "Invalid evidence fingerprint".into())
}
fn config(s: &CheckoutIdentity, key: &str, cancel: &AtomicBool) -> Result<String, String> {
    text(s, true, &["config", "--default", "", "--get", key], cancel)
}
fn remote(
    s: &CheckoutIdentity,
    head: &str,
    branch: Option<&str>,
    cancel: &AtomicBool,
) -> Result<(bool, String, Vec<String>), String> {
    let started = std::time::Instant::now();
    let mut blockers = Vec::new();
    let mut evidence = String::new();
    let names = text(s, true, &["remote"], cancel)?;
    let names: Vec<_> = names.lines().collect();
    if names.len() > 8
        || names
            .iter()
            .any(|name| name.is_empty() || name.chars().any(char::is_control))
    {
        return Ok((
            true,
            String::new(),
            vec!["Remote evidence exceeds safe inspection bounds; removal unavailable".into()],
        ));
    }
    let (upstream_remote, upstream_ref) =
        if let Some(branch) = branch.and_then(|b| b.strip_prefix("refs/heads/")) {
            (
                config(s, &format!("branch.{branch}.remote"), cancel)?,
                config(s, &format!("branch.{branch}.merge"), cancel)?,
            )
        } else {
            (String::new(), String::new())
        };
    if !upstream_ref.is_empty()
        && (!upstream_ref.starts_with("refs/heads/")
            || git(s, true, &["check-ref-format", &upstream_ref], cancel).is_err())
    {
        blockers.push("Configured upstream branch identity is invalid/ambiguous; repair Git configuration before removal".into());
        return Ok((true, String::new(), blockers));
    }
    if upstream_remote.is_empty() != upstream_ref.is_empty()
        || (!upstream_remote.is_empty()
            && upstream_remote != "."
            && !names.contains(&upstream_remote.as_str()))
    {
        blockers.push("Configured upstream remote or branch is missing/uncertain; repair upstream configuration and reopen preview".into());
    }
    evidence.push_str(&format!("upstream={upstream_remote}:{upstream_ref}\n"));
    if upstream_remote == "." {
        match text(s,true,&["rev-parse","--verify",&format!("{upstream_ref}^{{commit}}")],cancel) {
            Ok(oid)=>evidence.push_str(&format!("local-upstream={oid}\n")),
            Err(_)=>blockers.push("Configured local upstream branch is missing; repair upstream configuration before removal (a local upstream is not a remote backup)".into()),
        }
    }
    let mut backed_up = false;
    let mut upstream_found = upstream_remote.is_empty() || upstream_remote == ".";
    let mut inspected = 0usize;
    for name in names {
        let urls = match git(
            s,
            true,
            &["remote", "get-url", "--push", "--all", name],
            cancel,
        ) {
            Ok(urls) => urls,
            Err(_) => {
                blockers.push(format!(
                    "Remote {name} has no inspectable push destination; remote backup uncertain"
                ));
                continue;
            }
        };
        evidence.push_str(&format!(
            "remote={name}:{}\n",
            fingerprint(s, &urls, cancel)?
        ));
        let urls = String::from_utf8(urls)
            .map_err(|_| "Remote URL encoding uncertain; removal refused")?;
        let urls: Vec<_> = urls.lines().collect();
        if urls.is_empty() {
            blockers.push(format!(
                "Remote {name} has no push destination; backup evidence uncertain"
            ));
        }
        for url in urls {
            inspected += 1;
            if inspected > 8 || started.elapsed() > std::time::Duration::from_secs(12) {
                blockers.push(
                    "Remote push checks exceeded bounded inspection; removal unavailable".into(),
                );
                return Ok((!backed_up, evidence, blockers));
            }
            if url.is_empty() || url.chars().any(char::is_control) {
                blockers.push(format!(
                    "Remote {name} has an unsafe push destination; inspect Git configuration"
                ));
                continue;
            }
            let mut c = command(s, true);
            c.args([
                "-c",
                "credential.interactive=false",
                "ls-remote",
                "--refs",
                "--",
                url,
            ])
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_ASKPASS", "/usr/bin/false")
            .env("SSH_ASKPASS", "/usr/bin/false")
            .env("GCM_INTERACTIVE", "Never")
            .env("GCM_GUI_PROMPT", "0");
            let advertisement = match crate::git::run_git_command(&mut c, cancel) {
                Ok(bytes) => bytes,
                Err(_) => {
                    blockers.push(format!("Remote push evidence unavailable for {name}; check connectivity/authentication and reopen preview"));
                    continue;
                }
            };
            evidence.push_str(&format!(
                "advertised={}\n",
                fingerprint(s, &advertisement, cancel)?
            ));
            let advertisement = String::from_utf8(advertisement)
                .map_err(|_| "Remote advertisement encoding uncertain")?;
            let mut seen = std::collections::HashSet::new();
            for line in advertisement.lines() {
                let Some((oid, reference)) = line.split_once('\t') else {
                    return Err("Remote advertisement malformed; removal refused".into());
                };
                if !seen.insert(reference) {
                    return Err("Duplicate remote reference evidence; removal refused".into());
                }
                if name == upstream_remote && reference == upstream_ref {
                    upstream_found = true;
                }
                if oid == head {
                    backed_up = true;
                } else if name == upstream_remote && reference == upstream_ref {
                    if oid.len() != head.len() || !oid.bytes().all(|b| b.is_ascii_hexdigit()) {
                        return Err("Invalid upstream commit evidence".into());
                    }
                    if git(
                        s,
                        true,
                        &["cat-file", "-e", &format!("{oid}^{{commit}}")],
                        cancel,
                    )
                    .is_err()
                    {
                        blockers.push(format!("Live upstream commit for {name} is unavailable locally; ancestry/push evidence uncertain, fetch with Git then reopen preview"));
                    } else if text(
                        s,
                        true,
                        &["rev-list", "--count", head, "--not", oid],
                        cancel,
                    )? == "0"
                    {
                        backed_up = true;
                    }
                }
            }
        }
    }
    if !upstream_found {
        blockers.push("Configured upstream branch is absent from live push destinations; remote backup is uncertain".into());
    }
    Ok((!backed_up, evidence, blockers))
}
fn revision(s: &CheckoutIdentity, cancel: &AtomicBool) -> Result<(String, Option<String>), String> {
    let head = text(s, true, &["rev-parse", "--verify", "HEAD^{commit}"], cancel)?;
    let branch = text(
        s,
        true,
        &["rev-parse", "--symbolic-full-name", "HEAD"],
        cancel,
    )?;
    let branch = if branch == "HEAD" {
        None
    } else {
        if !branch.starts_with("refs/heads/") {
            return Err("Unsafe symbolic HEAD; removal refused".into());
        }
        text(s, true, &["check-ref-format", &branch], cancel)?;
        Some(branch)
    };
    Ok((head, branch))
}
fn reference(snapshot: &Snapshot, cancel: &AtomicBool) -> Result<Option<String>, String> {
    let bytes = git(
        &snapshot.checkout,
        false,
        &[
            "for-each-ref",
            "--format=%(refname)%00%(objectname)%00%(symref)%00",
            &snapshot.reference,
        ],
        cancel,
    )?;
    for line in bytes.split(|b| *b == b'\n').filter(|line| !line.is_empty()) {
        let parts: Vec<_> = line.split(|b| *b == 0).collect();
        if parts.first().copied() == Some(snapshot.reference.as_bytes()) {
            if parts.len() != 4 || !parts[2].is_empty() || !parts[3].is_empty() {
                return Err("Symbolic or malformed recovery reference; removal refused".into());
            }
            return String::from_utf8(parts[1].to_vec())
                .map(Some)
                .map_err(|_| "Invalid recovery reference object".into());
        }
    }
    Ok(None)
}
fn validate_namespace(snapshot: &Snapshot) -> Result<(), String> {
    let pieces: Vec<_> = snapshot.reference.split('/').collect();
    if pieces.len() != 5
        || pieces[..3] != ["refs", "rat", "removal"]
        || uuid::Uuid::parse_str(pieces[3]).is_err()
        || uuid::Uuid::parse_str(pieces[4]).is_err()
    {
        return Err("Invalid removal recovery identity".into());
    }
    Ok(())
}
pub fn preview(
    workspace: &crate::registry::Workspace,
    request: &str,
    cancel: &AtomicBool,
) -> Result<Snapshot, String> {
    uuid::Uuid::parse_str(request).map_err(|_| "Removal request must be a UUID")?;
    uuid::Uuid::parse_str(&workspace.id)
        .map_err(|_| "Removal workspace identity must be a UUID")?;
    let checkout = workspace
        .checkout
        .clone()
        .ok_or("Canonical checkout identity unavailable")?;
    if crate::git::resolve_checkout(&checkout.root, cancel)? != checkout {
        return Err("Checkout identity changed; removal refused".into());
    }
    link_files(&checkout)?;
    let (found, mut blockers) = registration(&checkout, cancel)?;
    blockers.extend(files(&checkout, cancel)?);
    if !found {
        return Err("Linked checkout registration missing; removal refused".into());
    }
    let (head, branch) = revision(&checkout, cancel)?;
    let (local_only, remote_evidence, remote_blockers) =
        remote(&checkout, &head, branch.as_deref(), cancel)?;
    blockers.extend(remote_blockers);
    let root_anchor = anchor(&checkout.root, true)?;
    let git_anchor = anchor(&checkout.git_dir, true)?;
    let common_anchor = anchor(&checkout.common_dir, true)?;
    let dotgit_anchor = anchor(&checkout.root.join(".git"), false)?;
    let registration_anchor = anchor(&checkout.git_dir.join("gitdir"), false)?;
    Ok(Snapshot { checkout, head, branch, reference:format!("refs/rat/removal/{}/{}",workspace.id,request),local_only,warning:if local_only {"Current HEAD has no confirmed remote backup; saved code is retained locally only. Folder removal does not push"}else{"Live remote evidence covers current HEAD; local recovery reference and saved code remain retained"}.into(),blockers,root_anchor,git_anchor,common_anchor,dotgit_anchor,registration_anchor,remote_evidence })
}
fn anchors(snapshot: &Snapshot) -> Result<(), String> {
    let s = &snapshot.checkout;
    if anchor(&s.root, true)? != snapshot.root_anchor
        || anchor(&s.git_dir, true)? != snapshot.git_anchor
        || anchor(&s.common_dir, true)? != snapshot.common_anchor
        || anchor(&s.root.join(".git"), false)? != snapshot.dotgit_anchor
        || anchor(&s.git_dir.join("gitdir"), false)? != snapshot.registration_anchor
    {
        return Err("Checkout path replaced or identity changed; removal refused".into());
    }
    link_files(s)
}
fn quick(snapshot: &Snapshot, cancel: &AtomicBool) -> Result<(), String> {
    anchors(snapshot)?;
    let s = &snapshot.checkout;
    if crate::git::resolve_checkout(&s.root, cancel)? != *s {
        return Err("Checkout identity changed; removal refused".into());
    }
    let (found, blockers) = registration(s, cancel)?;
    if !found || !blockers.is_empty() {
        return Err(format!(
            "Worktree registration changed; removal refused: {}",
            blockers.join("; ")
        ));
    }
    let blockers = files(s, cancel)?;
    if !blockers.is_empty() {
        return Err(format!("Removal blocked: {}", blockers.join("; ")));
    }
    if revision(s, cancel)? != (snapshot.head.clone(), snapshot.branch.clone()) {
        return Err("HEAD or branch changed since preview; reopen removal preview".into());
    }
    anchors(snapshot)
}
fn same(snapshot: &Snapshot, cancel: &AtomicBool) -> Result<(), String> {
    validate_namespace(snapshot)?;
    anchors(snapshot)?;
    let (local_only, evidence, blockers) = remote(
        &snapshot.checkout,
        &snapshot.head,
        snapshot.branch.as_deref(),
        cancel,
    )?;
    if local_only != snapshot.local_only || evidence != snapshot.remote_evidence {
        return Err(
            "Remote/upstream evidence changed since preview; reopen removal preview".into(),
        );
    }
    if !blockers.is_empty() {
        return Err(format!("Removal blocked: {}", blockers.join("; ")));
    }
    // Network evidence may have taken seconds. Reinspect local blockers and frozen
    // identity only after it finishes, immediately before the mutation boundary.
    quick(snapshot, cancel)
}
fn final_check(snapshot: &Snapshot, cancel: &AtomicBool) -> Result<(), String> {
    if reference(snapshot, cancel)?.as_deref() != Some(&snapshot.head) {
        return Err("Exact current HEAD recovery reference unavailable; removal refused".into());
    }
    quick(snapshot, cancel)?;
    // The late ignored inventory is independent of Git's ordinary refusal (which
    // protects tracked/untracked state but can erase ignored contents).
    let ignored = git(
        &snapshot.checkout,
        true,
        &[
            "ls-files",
            "--others",
            "--ignored",
            "--exclude-standard",
            "-z",
        ],
        cancel,
    )?;
    if !ignored.is_empty() {
        return Err(format!(
            "Ignored files appeared after confirmation; removal blocked: {}",
            display_path(&ignored)
        ));
    }
    anchors(snapshot)
}

pub fn protect(snapshot: &Snapshot, cancel: &AtomicBool) -> Result<(), String> {
    let _lease = crate::git::RepositoryMutation::acquire(&snapshot.checkout.common_dir, cancel)?;
    same(snapshot, cancel)?;
    if !snapshot.blockers.is_empty() {
        return Err(format!("Removal blocked: {}", snapshot.blockers.join("; ")));
    }
    match reference(snapshot, cancel)? {
        Some(oid) if oid == snapshot.head => {}
        Some(_) => {
            return Err(
                "Recovery reference identifies different saved work; removal refused".into(),
            );
        }
        None => {
            git(
                &snapshot.checkout,
                false,
                &["update-ref", &snapshot.reference, &snapshot.head, ""],
                cancel,
            )?;
        }
    }
    if reference(snapshot, cancel)?.as_deref() != Some(&snapshot.head) {
        return Err("Recovery reference verification failed".into());
    }
    same(snapshot, cancel)?;
    final_check(snapshot, cancel)
}
pub fn remove(snapshot: &Snapshot, cancel: &AtomicBool) -> Result<(), String> {
    let _lease = crate::git::RepositoryMutation::acquire(&snapshot.checkout.common_dir, cancel)?;
    same(snapshot, cancel)?;
    if !snapshot.blockers.is_empty() {
        return Err(format!("Removal blocked: {}", snapshot.blockers.join("; ")));
    }
    final_check(snapshot, cancel)?;
    // Git owns ordinary removal and its refusal; never force or recursively delete a recorded path.
    crate::git::run_git_command(
        command(&snapshot.checkout, false)
            .args(["worktree", "remove", "--"])
            .arg(&snapshot.checkout.root),
        cancel,
    )
    .map_err(|e| {
        format!("Git removal refused or result uncertain; inspect/reconcile exact request: {e}")
    })?;
    if !reconcile(snapshot, cancel)? {
        return Err("Git returned without confirming folder removal; inspect exact request".into());
    }
    Ok(())
}
/// Read-only outcome recovery. Startup and lost replies never replay deletion.
pub fn reconcile(snapshot: &Snapshot, cancel: &AtomicBool) -> Result<bool, String> {
    validate_namespace(snapshot)?;
    if anchor(&snapshot.checkout.common_dir, true)? != snapshot.common_anchor {
        return Err("Repository identity changed; removal outcome uncertain".into());
    }
    let (registered, _) = registration(&snapshot.checkout, cancel)?;
    match fs::symlink_metadata(&snapshot.checkout.root) {
        Ok(_) if registered => {
            anchors(snapshot)?;
            Ok(false)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound && !registered => {
            match fs::symlink_metadata(&snapshot.checkout.git_dir) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Ok(_) => {
                    return Err(
                        "Worktree registration directory remains; removal outcome uncertain".into(),
                    );
                }
                Err(e) => {
                    return Err(format!(
                        "Worktree registration cannot be inspected; removal outcome uncertain: {e}"
                    ));
                }
            }
            if reference(snapshot, cancel)?.as_deref() != Some(&snapshot.head) {
                return Err(
                    "Removal recovery reference unavailable; inspect retained Git state".into(),
                );
            }
            Ok(true)
        }
        _ => Err("Folder or registration replaced, missing or changed; no deletion retried".into()),
    }
}

#[cfg(test)]
#[path = "../tests/support/removal_git.rs"]
mod tests;
