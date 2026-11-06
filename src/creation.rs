//! Independent workspace creation. Git is executed only by the bounded worker;
//! a committed exact base precedes mutation. Restoring records never runs work.
use crate::{events::NativeAgent, git::CheckoutIdentity};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    thread,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Intent {
    pub request: String,
    pub project: String,
    pub base: String,
    pub branch: String,
    pub path: Vec<u8>,
    pub agent: NativeAgent,
    pub start: bool,
    pub prompt: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    Requested,
    LocalChoice,
    Ready,
    Creating,
    Created,
    Cancelled,
    Failed,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Record {
    pub intent: Intent,
    pub repository: Vec<u8>,
    pub common_dir: Vec<u8>,
    pub phase: Phase,
    #[serde(default)]
    pub launch_cancelled: bool,
    pub base_ref: Option<String>,
    pub commit: Option<String>,
    pub message: String,
    pub checkout: Option<CheckoutIdentity>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Facts {
    pub request: String,
    pub original_base_ref: String,
    pub original_base_commit: String,
    pub original_branch: String,
    pub created_branch: bool,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Preview {
    pub project: String,
    pub bases: Vec<String>,
    pub default_base: Option<String>,
    pub error: Option<String>,
    pub managed_root: Vec<u8>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct View {
    pub launches: Vec<LaunchReceipt>,
    pub retry_request: Option<String>,
    pub active_request: Option<String>,
    pub pending: bool,
    pub preview: Option<Preview>,
    pub records: Vec<Record>,
    pub error: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LaunchReceipt {
    pub request: String,
    pub session: Option<String>,
    pub lifecycle: crate::events::Lifecycle,
    pub result: Option<crate::events::LaunchResult>,
    pub pending: bool,
    pub error: Option<String>,
}
impl Intent {
    pub fn validate(&self) -> Result<(), String> {
        let request =
            uuid::Uuid::parse_str(&self.request).map_err(|_| "Invalid creation request UUID")?;
        if request.to_string() != self.request {
            return Err("Creation request must be a canonical UUID".into());
        }
        if self.base.is_empty()
            || self.base.len() > 1024
            || self.base.starts_with('-')
            || self.branch.is_empty()
            || self.branch.len() > 240
            || self.branch.starts_with('-')
            || self.branch.contains("@{")
            || self.path.is_empty()
            || self.path.len() > 32768
            || self.path.contains(&0)
            || self.prompt.as_ref().is_some_and(|p| p.len() > 4084)
        {
            return Err(
                "Choose a base, valid branch and absolute folder; prompt limit is 4084 bytes"
                    .into(),
            );
        }
        if path(&self.path).components().any(|c| {
            matches!(
                c,
                std::path::Component::ParentDir | std::path::Component::CurDir
            )
        }) {
            return Err("Workspace folder must not contain . or .. components".into());
        }
        if !path(&self.path).is_absolute() {
            return Err("Workspace folder must be absolute".into());
        }
        Ok(())
    }
}
pub fn path(bytes: &[u8]) -> PathBuf {
    use std::os::unix::ffi::OsStringExt;
    PathBuf::from(std::ffi::OsString::from_vec(bytes.to_vec()))
}
fn text(bytes: Vec<u8>) -> Result<String, String> {
    let mut bytes = bytes;
    if bytes.last() == Some(&b'\n') {
        bytes.pop();
    }
    String::from_utf8(bytes)
        .map_err(|_| "Git reference is not UTF-8; choose a supported explicit base".into())
}
fn git(repo: &Path, args: &[&str], cancel: &AtomicBool) -> Result<String, String> {
    if args.first() == Some(&"fetch") {
        text(crate::git::run_git_command_deadline(
            std::process::Command::new("git")
                .arg("-C")
                .arg(repo)
                .args(args),
            cancel,
            std::time::Duration::from_secs(30),
        )?)
    } else {
        text(crate::git::run_git(repo, args, cancel)?)
    }
}

pub enum Work {
    Preview {
        project: String,
        repository: PathBuf,
        managed_root: PathBuf,
    },
    Prepare(Record),
    Create(Record),
}
pub enum ResultEvent {
    Preview(Preview),
    Record(Box<Record>),
}
pub struct Job {
    cancel: Arc<AtomicBool>,
    _worker: thread::JoinHandle<()>,
    pub result: Receiver<ResultEvent>,
}
impl Job {
    pub fn start(work: Work) -> std::io::Result<Self> {
        let cancel = Arc::new(AtomicBool::new(false));
        let cancellation = cancel.clone();
        let (sender, result) = mpsc::sync_channel(1);
        let worker = thread::Builder::new()
            .name("workspace-creation".into())
            .spawn(move || {
                let event = match work {
                    Work::Preview {
                        project,
                        repository,
                        managed_root,
                    } => {
                        let mut result = preview(project, &repository, &cancellation);
                        let managed_root = if managed_root.is_absolute() {
                            managed_root
                        } else {
                            match std::env::current_dir() {
                                Ok(cwd) => cwd.join(managed_root),
                                Err(error) => {
                                    result.error =
                                        Some(format!("Managed folder unavailable: {error}"));
                                    PathBuf::new()
                                }
                            }
                        };
                        result.managed_root = crate::session::path_bytes(&managed_root);
                        ResultEvent::Preview(result)
                    }
                    Work::Prepare(mut record) => {
                        prepare(&mut record, &cancellation);
                        ResultEvent::Record(Box::new(record))
                    }
                    Work::Create(mut record) => {
                        create(&mut record, &cancellation);
                        ResultEvent::Record(Box::new(record))
                    }
                };
                let _ = sender.try_send(event);
            })?;
        Ok(Self {
            cancel,
            _worker: worker,
            result,
        })
    }
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Release);
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancel();
    }
}
fn preview(project: String, repo: &Path, cancel: &AtomicBool) -> Preview {
    let mut result = Preview {
        project,
        ..Preview::default()
    };
    match git(
        repo,
        &[
            "for-each-ref",
            "--format=%(refname:short)",
            "refs/heads/",
            "refs/remotes/",
        ],
        cancel,
    ) {
        Ok(refs) => {
            result.bases = refs
                .lines()
                .filter(|r| !r.ends_with("/HEAD"))
                .map(str::to_owned)
                .collect()
        }
        Err(error) => {
            result.error = Some(error);
            return result;
        }
    }
    // Remote HEAD is evidence; the caller must choose when remotes disagree.
    let defaults = git(
        repo,
        &["for-each-ref", "--format=%(symref)", "refs/remotes/"],
        cancel,
    )
    .unwrap_or_default()
    .lines()
    .filter_map(|r| r.strip_prefix("refs/remotes/"))
    .map(str::to_owned)
    .collect::<Vec<_>>();
    if defaults.len() == 1 {
        result.default_base = Some(defaults[0].clone());
    }
    result
}
fn prepare(record: &mut Record, cancel: &AtomicBool) {
    let result = (|| -> Result<(), String> {
        let repo = path(&record.repository);
        let identity = crate::git::resolve_checkout(&repo, cancel)?;
        if crate::session::path_bytes(&identity.common_dir) != record.common_dir {
            return Err("Project repository identity changed".into());
        }
        let _mutation = crate::git::RepositoryMutation::acquire(&identity.common_dir, cancel)?;
        let destination = path(&record.intent.path);
        let mut ancestor = destination.as_path();
        while !ancestor.exists() {
            ancestor = ancestor.parent().ok_or("No available folder ancestor")?;
        }
        let canonical = ancestor.canonicalize().map_err(|e| e.to_string())?;
        if canonical.starts_with(&identity.root) || canonical.starts_with(&identity.common_dir) {
            return Err(
                "Choose a workspace folder outside the original checkout and Git directory".into(),
            );
        }
        if destination.exists() {
            return Err("Workspace folder already exists; choose another folder".into());
        }
        git(
            &repo,
            &["check-ref-format", "--branch", &record.intent.branch],
            cancel,
        )?;
        if git(
            &repo,
            &[
                "show-ref",
                "--verify",
                &format!("refs/heads/{}", record.intent.branch),
            ],
            cancel,
        )
        .is_ok()
        {
            return Err("Workspace branch already exists; choose another branch".into());
        }
        let remotes = git(&repo, &["remote"], cancel)?;
        let base = &record.intent.base;
        let remote_base = remotes.lines().find_map(|remote| {
            base.strip_prefix(&format!("{remote}/"))
                .map(|branch| (remote.to_owned(), branch.to_owned()))
        });
        let local_ref = if base.starts_with("refs/") {
            base.clone()
        } else {
            format!("refs/heads/{base}")
        };
        let remote_base = if remote_base.is_some() || remotes.is_empty() {
            remote_base
        } else {
            let upstream = git(
                &repo,
                &["for-each-ref", "--format=%(upstream)", &local_ref],
                cancel,
            )?;
            let upstream=upstream.strip_prefix("refs/remotes/").ok_or("Choose a remote base (for example origin/main); the local branch has no configured upstream")?;
            let (remote, branch) = upstream
                .split_once('/')
                .ok_or("Upstream is ambiguous; choose a remote base")?;
            Some((remote.to_owned(), branch.to_owned()))
        };
        let base_ref = remote_base
            .as_ref()
            .map(|(remote, branch)| format!("refs/remotes/{remote}/{branch}"))
            .unwrap_or(local_ref);
        if let Some((remote, branch)) = remote_base
            && let Err(error) = git(
                &repo,
                &[
                    "fetch",
                    "--no-tags",
                    &remote,
                    &format!("+refs/heads/{branch}:{base_ref}"),
                ],
                cancel,
            )
        {
            record.commit = git(
                &repo,
                &["rev-parse", "--verify", &format!("{base_ref}^{{commit}}")],
                cancel,
            )
            .ok();
            record.base_ref = Some(base_ref);
            record.phase = Phase::LocalChoice;
            record.message = format!(
                "Latest code could not be obtained: {error}. Explicit local fallback uses {}",
                record
                    .commit
                    .as_deref()
                    .unwrap_or("unavailable; choose another base")
            );
            return Ok(());
        }
        record.commit = Some(git(
            &repo,
            &["rev-parse", "--verify", &format!("{base_ref}^{{commit}}")],
            cancel,
        )?);
        record.base_ref = Some(base_ref);
        record.phase = Phase::Ready;
        record.message = format!(
            "Base pinned at {}",
            record.commit.as_deref().unwrap_or_default()
        );
        Ok(())
    })();
    if let Err(error) = result {
        record.phase = Phase::Failed;
        record.message = error;
    }
}

fn create(record: &mut Record, cancel: &AtomicBool) {
    let mut reconciled = false;
    let result = (|| -> Result<CheckoutIdentity, String> {
        record.intent.validate()?;
        let repo = path(&record.repository);
        let folder = path(&record.intent.path);
        let identity = crate::git::resolve_checkout(&repo, cancel)?;
        if crate::session::path_bytes(&identity.common_dir) != record.common_dir {
            return Err("Project repository identity changed".into());
        }
        let _mutation = crate::git::RepositoryMutation::acquire(&identity.common_dir, cancel)?;
        let commit = record
            .commit
            .as_deref()
            .ok_or("No committed creation base")?;
        if !matches!(commit.len(), 40 | 64) || !commit.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("Saved base commit is invalid; creation refused".into());
        }
        let branch = &record.intent.branch;
        let branch_ref = format!("refs/heads/{branch}");
        let claim_ref = format!("refs/rat/creation/{}", record.intent.request);
        let claimed = git(&repo, &["rev-parse", "--verify", &claim_ref], cancel).ok();
        let branch_head = git(&repo, &["rev-parse", "--verify", &branch_ref], cancel).ok();
        if claimed.as_deref().is_some_and(|head| head != commit) {
            return Err("Creation claim changed; retained without mutation".into());
        }
        if branch_head.is_some()
            && (claimed.as_deref() != Some(commit) || branch_head.as_deref() != Some(commit))
        {
            return Err(
                "Branch exists without this operation's matching claim; retained without mutation"
                    .into(),
            );
        }
        if claimed.is_some()
            && (git(&repo, &["symbolic-ref", "--quiet", &branch_ref], cancel).is_ok()
                || git(&repo, &["symbolic-ref", "--quiet", &claim_ref], cancel).is_ok())
        {
            return Err("Claimed branch is now symbolic; retained without mutation".into());
        }
        git(&repo, &["check-ref-format", "--branch", branch], cancel)?;
        if folder.exists() {
            // Reconcile a lost completion only for the exact durable target and pinned Git facts.
            let checkout = crate::git::resolve_checkout(&folder, cancel)?;
            let actual = git(&folder, &["symbolic-ref", "--short", "HEAD"], cancel)?;
            let head = git(&folder, &["rev-parse", "HEAD"], cancel)?;
            if checkout.common_dir == identity.common_dir
                && actual == *branch
                && head == commit
                && record.phase == Phase::Creating
                && claimed.as_deref() == Some(commit)
                && folder.canonicalize().ok().as_ref() == Some(&checkout.root)
            {
                reconciled = true;
                return Ok(checkout);
            }
            return Err("Folder exists; retained without modification. Choose a new request/folder or reconcile the original creation".into());
        }
        if claimed.is_none() || branch_head.is_none() {
            let claim = if claimed.is_none() {
                format!("create {claim_ref} {commit}\n")
            } else {
                format!("verify {claim_ref} {commit}\n")
            };
            let transaction =
                format!("start\ncreate {branch_ref} {commit}\n{claim}prepare\ncommit\n");
            crate::git::run_git_command_input(
                std::process::Command::new("git")
                    .arg("-C")
                    .arg(&repo)
                    .args(["update-ref", "--no-deref", "--stdin"]),
                cancel,
                transaction.as_bytes(),
            )?;
        }
        if cancel.load(Ordering::Acquire) {
            return Err("Creation cancelled before Git mutation".into());
        }
        if let Some(parent) = folder.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut command = std::process::Command::new("git");
        command
            .arg("-C")
            .arg(&repo)
            .args(["worktree", "add", "--"])
            .arg(&folder)
            .arg(branch);
        let parent = folder
            .parent()
            .ok_or("Workspace parent unavailable")?
            .canonicalize()
            .map_err(|e| e.to_string())?;
        if parent.starts_with(&identity.root) || parent.starts_with(&identity.common_dir) {
            return Err(
                "Workspace folder now resolves inside the original checkout; creation refused"
                    .into(),
            );
        }
        crate::git::run_git_command_deadline(
            &mut command,
            cancel,
            std::time::Duration::from_secs(30),
        )?;
        let checkout = crate::git::resolve_checkout(&folder, cancel)?;
        if git(&folder, &["rev-parse", "HEAD"], cancel)? != commit
            || folder.canonicalize().ok().as_ref() != Some(&checkout.root)
        {
            return Err(
                "Created checkout or branch changed during mutation; retained without launch"
                    .into(),
            );
        }
        Ok(checkout)
    })();
    match result {
        Ok(checkout) => {
            record.checkout = Some(checkout);
            record.phase = Phase::Created;
            record.message=if reconciled {"Existing operation checkout reconciled; working files retained as found. Start remains explicit."}else{"Workspace created. Start remains an explicit action if launch fails or this operation is recovered."}.into();
        }
        Err(error) => {
            record.phase = Phase::Failed;
            record.message = format!(
                "{error}. Any created branch/folder is retained; recover this same request before choosing another."
            );
        }
    }
}

/// Supervisor-side coordinator. It advances only matching writer receipts and
/// never starts work merely because a saved record was loaded.
pub struct Coordinator {
    pub view: View,
    loaded: bool,
    retry_record: Option<Record>,
    job: Option<Job>,
    saving: Option<(uuid::Uuid, Record)>,
    cancelled: bool,
    launch_authorized: bool,
    pub launch: Option<Record>,
}
impl Coordinator {
    pub fn new(storage: &crate::persistence::Worker) -> Self {
        let error = storage
            .commands
            .try_send(crate::persistence::Command::CreationLoad)
            .err()
            .map(|e| e.to_string());
        Self {
            view: View {
                pending: true,
                error,
                ..View::default()
            },
            loaded: false,
            retry_record: None,
            job: None,
            saving: None,
            cancelled: false,
            launch_authorized: false,
            launch: None,
        }
    }
    pub fn busy(&self) -> bool {
        self.job.is_some() || self.saving.is_some() || !self.loaded
    }
    pub fn cancel(&mut self) {
        self.cancelled = true;
        self.launch_authorized = false;
        self.launch = None;
        if let Some(job) = &self.job {
            job.cancel();
        }
    }
    fn save(&mut self, record: Record, storage: &crate::persistence::Worker) {
        let id = uuid::Uuid::new_v4();
        match storage
            .commands
            .try_send(crate::persistence::Command::CreationSave(
                id,
                Box::new(record.clone()),
            )) {
            Ok(()) => self.saving = Some((id, record)),
            Err(error) => {
                self.view.retry_request = Some(record.intent.request.clone());
                self.retry_record = Some(record);
                self.view.error = Some(format!(
                    "Creation phase not saved: {error}. Retry same request; no further step authorized"
                ))
            }
        }
    }
    fn work(&mut self, work: Work) {
        match Job::start(work) {
            Ok(job) => self.job = Some(job),
            Err(error) => self.view.error = Some(error.to_string()),
        }
    }
    pub fn handle(
        &mut self,
        request: crate::events::Request,
        registry: &crate::registry::Registry,
        storage: &crate::persistence::Worker,
        directory: &Path,
    ) {
        use crate::events::Request;
        if let Request::CancelCreation { request, .. } = request {
            if let Some(record) = self
                .retry_record
                .as_mut()
                .filter(|r| r.intent.request == request)
            {
                record.launch_cancelled = true;
                if record.checkout.is_none() {
                    record.phase = Phase::Cancelled;
                }
                record.message="Cancelled intent retained; G records this same cancellation after storage repair".into();
                if self
                    .view
                    .active_request
                    .as_ref()
                    .is_none_or(|current| *current == request)
                {
                    self.launch_authorized = false;
                    self.cancelled = true;
                }
                return;
            }
            if self.view.active_request.as_ref() == Some(&request) {
                self.cancel();
            } else if !self.busy()
                && let Some(mut record) = self
                    .view
                    .records
                    .iter()
                    .find(|r| r.intent.request == request)
                    .cloned()
            {
                self.cancelled = true;
                self.launch_authorized = false;
                record.launch_cancelled = true;
                if record.checkout.is_none() {
                    record.phase = Phase::Cancelled;
                }
                record.message="Creation launch intent cancelled; completed Git work retained. Native stop has its own observed outcome.".into();
                self.view.active_request = Some(request);
                self.save(record, storage);
            }
            return;
        }
        self.view.error = None;
        if self.busy() {
            self.view.error = Some("Workspace creation pending; wait or cancel".into());
            return;
        }
        self.cancelled = false;
        match request {
            Request::PreviewCreation { project } => {
                if let Some(checkout) = registry
                    .workspaces
                    .iter()
                    .find(|w| w.project.as_ref() == Some(&project))
                    .and_then(|w| w.checkout.as_ref())
                {
                    self.work(Work::Preview {
                        managed_root: directory.join("workspaces").join(&project),
                        project,
                        repository: checkout.root.clone(),
                    });
                } else {
                    self.view.error =
                        Some("Select a registered project with an available checkout".into());
                }
            }
            Request::CreateWorkspace(intent) => {
                if let Some(previous) = self
                    .view
                    .records
                    .iter()
                    .find(|r| r.intent.request == intent.request)
                {
                    if serde_json::to_string(&previous.intent).ok()
                        != serde_json::to_string(&intent).ok()
                    {
                        self.view.error=Some("Creation request already belongs to different choices; original outcome retained".into());
                    }
                    return;
                }
                self.launch_authorized = intent.start;
                self.view.active_request = Some(intent.request.clone());
                if let Err(error) = intent.validate() {
                    self.view.error = Some(error);
                    return;
                }
                let Some(project) = registry.projects.iter().find(|p| p.id == intent.project)
                else {
                    self.view.error = Some("Project unavailable".into());
                    return;
                };
                let Some(checkout) = registry
                    .workspaces
                    .iter()
                    .filter(|w| w.project.as_ref() == Some(&project.id))
                    .find_map(|w| w.checkout.as_ref())
                else {
                    self.view.error = Some("Project has no known checkout".into());
                    return;
                };
                // The default folder is beneath durable supervisor-owned storage, outside source.
                let record = Record {
                    intent,
                    repository: crate::session::path_bytes(&checkout.root),
                    common_dir: project.common_dir.clone(),
                    phase: Phase::Requested,
                    launch_cancelled: false,
                    base_ref: None,
                    commit: None,
                    message: "Creation intent saved before fetching".into(),
                    checkout: None,
                };
                self.save(record, storage);
            }
            Request::RecoverCreation { request, local } => {
                self.launch_authorized = false;
                self.view.active_request = Some(request.clone());
                if self
                    .retry_record
                    .as_ref()
                    .is_some_and(|record| record.intent.request == request)
                {
                    let mut record = self.retry_record.take().unwrap();
                    if !local && record.intent.start {
                        record.launch_cancelled = true;
                    }
                    self.view.retry_request = None;
                    self.save(record, storage);
                    return;
                }
                self.launch_authorized = false;
                let Some(mut record) = self
                    .view
                    .records
                    .iter()
                    .find(|r| r.intent.request == request)
                    .cloned()
                else {
                    self.view.error =
                        Some("Creation outcome unknown; do not repeat with a new identity".into());
                    return;
                };
                if local && record.phase != Phase::LocalChoice {
                    self.view.error=Some("Local fallback is available only after a failed fetch; query or recover with G".into());
                    return;
                }
                let local_choice = record.phase == Phase::LocalChoice && local;
                self.launch_authorized =
                    local_choice && record.intent.start && !record.launch_cancelled;
                if record.intent.start && !local_choice {
                    record.launch_cancelled = true;
                }
                match record.phase {
                    Phase::Created => {}
                    Phase::LocalChoice if local => {
                        if record.commit.is_none() {
                            self.view.error=Some("No local commit available; select a different base in a new creation".into());
                            return;
                        }
                        record.phase = Phase::Ready;
                        record.message =
                            "Explicit local fallback; using the displayed pinned commit".into();
                        self.save(record, storage);
                    }
                    Phase::LocalChoice => self.view.error = Some(
                        "Fetch failed. Choose explicit local fallback to use the displayed commit"
                            .into(),
                    ),
                    Phase::Requested => self.work(Work::Prepare(record)),
                    _ if record.commit.is_some() => {
                        record.phase = Phase::Creating;
                        self.save(record, storage);
                    }
                    _ => self.work(Work::Prepare(record)),
                }
            }
            _ => {}
        }
        self.view.pending = self.busy();
    }
    pub fn complete(
        &mut self,
        event: &crate::persistence::Completion,
        storage: &crate::persistence::Worker,
    ) -> Option<crate::registry::Registry> {
        match event {
            crate::persistence::Completion::Creations(result) => {
                self.loaded = result.is_ok();
                match result {
                    Ok(records) => self.view.records = records.clone(),
                    Err(error) => {
                        self.view.error = Some(format!(
                            "Creation receipts unavailable: {error}. Repair storage and reopen before creating"
                        ))
                    }
                };
            }
            crate::persistence::Completion::CreationSaved(id, result) => {
                if self
                    .saving
                    .as_ref()
                    .is_none_or(|(pending, _)| pending != id)
                {
                    return None;
                }
                let (_, mut record) = self.saving.take().unwrap();
                match result {
                    Err(error) => {
                        if self.cancelled {
                            record.launch_cancelled = true;
                            if record.checkout.is_none() {
                                record.phase = Phase::Cancelled;
                            }
                        }
                        self.view.retry_request = Some(record.intent.request.clone());
                        self.retry_record = Some(record);
                        self.view.error = Some(format!(
                            "Creation phase not durable: {error}. Recover same request before retrying"
                        ))
                    }
                    Ok(registry) => {
                        if let Some(old) = self
                            .view
                            .records
                            .iter_mut()
                            .find(|r| r.intent.request == record.intent.request)
                        {
                            *old = record.clone();
                        } else {
                            self.view.records.push(record.clone());
                        }
                        if self.cancelled {
                            if record.phase == Phase::Created && !record.launch_cancelled {
                                record.launch_cancelled = true;
                                record.message="Workspace created; requested launch cancelled. Retained for explicit Start.".into();
                                self.save(record, storage);
                            } else if !matches!(
                                record.phase,
                                Phase::Cancelled | Phase::Created | Phase::Failed
                            ) {
                                record.phase = Phase::Cancelled;
                                record.launch_cancelled = true;
                                record.message="Creation cancelled; completed Git work, if any, retained. Explicit recovery required".into();
                                self.save(record, storage);
                            }
                        } else {
                            match record.phase {
                                Phase::Requested => self.work(Work::Prepare(record)),
                                Phase::Ready => {
                                    record.phase = Phase::Creating;
                                    record.message =
                                        "Exact base committed; creating branch and checkout".into();
                                    self.save(record, storage);
                                }
                                Phase::Creating => self.work(Work::Create(record)),
                                Phase::Created => {
                                    record.intent.start &=
                                        self.launch_authorized && !record.launch_cancelled;
                                    self.launch = Some(record);
                                }
                                _ => {}
                            }
                        }
                        return Some(registry.clone());
                    }
                }
            }
            _ => {}
        }
        self.view.pending = self.busy();
        None
    }
    pub fn retry_storage(&mut self, storage: &crate::persistence::Worker) {
        if !self.loaded
            && self.job.is_none()
            && self.saving.is_none()
            && let Err(error) = storage
                .commands
                .try_send(crate::persistence::Command::CreationLoad)
        {
            self.view.error = Some(format!("Creation receipts not loaded: {error}"));
        }
    }
    pub fn advance(&mut self, storage: &crate::persistence::Worker) {
        let event = self.job.as_ref().map(|job| job.result.try_recv());
        if matches!(event, Some(Err(mpsc::TryRecvError::Disconnected))) {
            self.job = None;
            self.view.error = Some(
                "Creation worker ended without a result; recover the same request explicitly"
                    .into(),
            );
        }
        if let Some(Ok(event)) = event {
            self.job = None;
            match event {
                ResultEvent::Preview(preview) => self.view.preview = Some(preview),
                ResultEvent::Record(mut record) => {
                    if self.cancelled {
                        record.launch_cancelled = true;
                        record
                            .message
                            .push_str(" Requested coding launch cancelled; workspace retained.");
                    }
                    if self.cancelled && record.checkout.is_none() {
                        record.phase = Phase::Cancelled;
                        record.message.push_str(
                            " Cancelled; no launch authorized. Reconcile same request explicitly.",
                        );
                    }
                    self.save(*record, storage);
                }
            }
        }

        self.view.pending = self.busy();
        if !self.view.pending {
            self.view.active_request = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn command(repo: &Path, args: &[&str]) -> String {
        let result = std::process::Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(args)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        String::from_utf8(result.stdout).unwrap().trim().into()
    }
    fn repository() -> tempfile::TempDir {
        let repo = tempfile::tempdir().unwrap();
        command(repo.path(), &["init", "-b", "main"]);
        command(repo.path(), &["config", "user.email", "test@example.com"]);
        command(repo.path(), &["config", "user.name", "Test"]);
        std::fs::write(repo.path().join("tracked"), "original\n").unwrap();
        command(repo.path(), &["add", "tracked"]);
        command(repo.path(), &["commit", "-m", "initial"]);
        repo
    }
    fn record(repo: &Path, folder: &Path) -> Record {
        Record {
            intent: Intent {
                request: uuid::Uuid::new_v4().to_string(),
                project: "project".into(),
                base: "main".into(),
                branch: "workspace/new".into(),
                path: crate::session::path_bytes(folder),
                agent: NativeAgent::Claude,
                start: false,
                prompt: Some("Fix the parser".into()),
            },
            repository: crate::session::path_bytes(repo),
            common_dir: crate::session::path_bytes(&repo.join(".git").canonicalize().unwrap()),
            phase: Phase::Requested,
            launch_cancelled: false,
            base_ref: None,
            commit: None,
            message: String::new(),
            checkout: None,
        }
    }
    fn worker(work: Work) -> ResultEvent {
        Job::start(work)
            .unwrap()
            .result
            .recv_timeout(std::time::Duration::from_secs(15))
            .unwrap()
    }
    fn result(work: Work) -> Record {
        let ResultEvent::Record(record) = worker(work) else {
            panic!("expected record")
        };
        *record
    }
    #[test]
    fn independent_checkout_pins_base_preserves_index_and_recovers_same_result() {
        let repo = repository();
        let folders = tempfile::tempdir().unwrap();
        let folder = folders.path().join("new");
        std::fs::write(repo.path().join("tracked"), "staged\n").unwrap();
        command(repo.path(), &["add", "tracked"]);
        std::fs::write(repo.path().join("tracked"), "unstaged\n").unwrap();
        std::fs::write(repo.path().join("untracked"), "private\n").unwrap();
        let before = command(repo.path(), &["status", "--porcelain"]);
        let mut prepared = result(Work::Prepare(record(repo.path(), &folder)));
        assert_eq!(prepared.phase, Phase::Ready);
        let pinned = prepared.commit.clone().unwrap();
        // The branch can move after preparation; creation must use its saved commit.
        command(repo.path(), &["commit", "-m", "later"]);
        let index = std::fs::read(repo.path().join(".git/index")).unwrap();
        let status = command(repo.path(), &["status", "--porcelain"]);
        prepared.phase = Phase::Creating;
        let created = result(Work::Create(prepared.clone()));
        assert_eq!(created.phase, Phase::Created, "{}", created.message);
        assert_eq!(command(&folder, &["rev-parse", "HEAD"]), pinned);
        assert_eq!(
            command(
                repo.path(),
                &[
                    "rev-parse",
                    "--verify",
                    &format!("refs/rat/creation/{}", created.intent.request)
                ]
            ),
            pinned
        );
        assert_eq!(
            std::fs::read_to_string(folder.join("tracked")).unwrap(),
            "original\n"
        );
        let again = result(Work::Create(prepared));
        assert_eq!(again.phase, Phase::Created, "{}", again.message);
        assert_eq!(
            command(repo.path(), &["worktree", "list", "--porcelain"])
                .matches("worktree ")
                .count(),
            2
        );
        assert!(before.contains("MM tracked"));
        assert_eq!(
            std::fs::read(repo.path().join(".git/index")).unwrap(),
            index
        );
        assert_eq!(command(repo.path(), &["status", "--porcelain"]), status);
        assert_eq!(
            std::fs::read_to_string(repo.path().join("tracked")).unwrap(),
            "unstaged\n"
        );
        assert_eq!(
            std::fs::read_to_string(repo.path().join("untracked")).unwrap(),
            "private\n"
        );
    }
    #[test]
    fn default_ambiguity_and_collisions_require_explicit_choices_without_overwriting() {
        let repo = repository();
        let temp = tempfile::tempdir().unwrap();
        command(repo.path(), &["branch", "other"]);
        let ResultEvent::Preview(preview) = worker(Work::Preview {
            project: "project".into(),
            repository: repo.path().into(),
            managed_root: temp.path().into(),
        }) else {
            panic!("preview expected")
        };
        assert!(preview.default_base.is_none());
        assert_eq!(preview.bases, vec!["main", "other"]);
        let folder = temp.path().join("existing");
        std::fs::create_dir(&folder).unwrap();
        std::fs::write(folder.join("keep"), "untouched").unwrap();
        let result = result(Work::Prepare(record(repo.path(), &folder)));
        assert_eq!(result.phase, Phase::Failed);
        assert_eq!(
            std::fs::read_to_string(folder.join("keep")).unwrap(),
            "untouched"
        );
        let folder = temp.path().join("absent");
        let mut intent = record(repo.path(), &folder);
        intent.intent.branch = "other".into();
        let outcome = super::tests::result(Work::Prepare(intent));
        assert_eq!(outcome.phase, Phase::Failed);
        assert!(!folder.exists());
        assert_eq!(
            command(repo.path(), &["branch", "--list"]).lines().count(),
            2
        );
    }
    #[test]
    fn remote_creation_fetches_a_new_commit_without_moving_the_original_checkout() {
        let repo = repository();
        let temp = tempfile::tempdir().unwrap();
        let remote = temp.path().join("remote.git");
        std::fs::create_dir(&remote).unwrap();
        command(&remote, &["init", "--bare", "-b", "main"]);
        command(
            repo.path(),
            &["remote", "add", "origin", remote.to_str().unwrap()],
        );
        command(repo.path(), &["push", "-u", "origin", "main"]);
        command(
            repo.path(),
            &[
                "symbolic-ref",
                "refs/remotes/origin/HEAD",
                "refs/remotes/origin/main",
            ],
        );
        let original = command(repo.path(), &["rev-parse", "HEAD"]);
        let index = std::fs::read(repo.path().join(".git/index")).unwrap();
        let updater = temp.path().join("updater");
        command(
            temp.path(),
            &["clone", remote.to_str().unwrap(), updater.to_str().unwrap()],
        );
        command(
            &updater,
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.com",
                "commit",
                "--allow-empty",
                "-m",
                "remote advance",
            ],
        );
        command(&updater, &["push", "origin", "main"]);
        let expected = command(&updater, &["rev-parse", "HEAD"]);
        assert_ne!(expected, original);
        let ResultEvent::Preview(preview) = worker(Work::Preview {
            project: "project".into(),
            repository: repo.path().into(),
            managed_root: temp.path().join("managed"),
        }) else {
            panic!("preview expected")
        };
        assert_eq!(preview.default_base.as_deref(), Some("origin/main"));
        let folder = temp.path().join("created");
        let mut requested = record(repo.path(), &folder);
        requested.intent.base = "origin/main".into();
        let mut pinned = result(Work::Prepare(requested));
        assert_eq!(pinned.phase, Phase::Ready, "{}", pinned.message);
        assert_eq!(pinned.commit.as_deref(), Some(expected.as_str()));
        assert_eq!(pinned.base_ref.as_deref(), Some("refs/remotes/origin/main"));
        pinned.phase = Phase::Creating;
        let created = result(Work::Create(pinned));
        assert_eq!(created.phase, Phase::Created, "{}", created.message);
        assert_eq!(command(&folder, &["rev-parse", "HEAD"]), expected);
        assert_eq!(command(repo.path(), &["rev-parse", "HEAD"]), original);
        assert_eq!(
            std::fs::read(repo.path().join(".git/index")).unwrap(),
            index
        );
    }
    #[test]
    fn same_request_finishes_its_claimed_branch_after_checkout_creation_was_interrupted() {
        let repo = repository();
        let temp = tempfile::tempdir().unwrap();
        let folder = temp.path().join("workspace");
        let mut pending = result(Work::Prepare(record(repo.path(), &folder)));
        assert_eq!(pending.phase, Phase::Ready);
        pending.phase = Phase::Creating;
        let commit = pending.commit.as_deref().unwrap();
        // A committed creation claim is Git's own atomic branch/operation pair;
        // this reproduces restart after that pair committed but before checkout.
        command(
            repo.path(),
            &[
                "update-ref",
                &format!("refs/heads/{}", pending.intent.branch),
                commit,
            ],
        );
        command(
            repo.path(),
            &[
                "update-ref",
                &format!("refs/rat/creation/{}", pending.intent.request),
                commit,
            ],
        );
        let created = result(Work::Create(pending.clone()));
        assert_eq!(created.phase, Phase::Created, "{}", created.message);
        assert_eq!(command(&folder, &["rev-parse", "HEAD"]), commit);
        let repeated = result(Work::Create(pending));
        assert_eq!(repeated.phase, Phase::Created);
        assert_eq!(
            command(repo.path(), &["worktree", "list", "--porcelain"])
                .matches("worktree ")
                .count(),
            2
        );
    }
    #[test]
    fn recovering_a_failed_intent_write_disarms_the_previously_requested_native_launch() {
        for cancel_before_recovery in [false, true] {
            let repo = repository();
            let state = tempfile::tempdir().unwrap();
            let folder = state.path().join("workspace");
            let (storage, _) = crate::persistence::Worker::start(state.path()).unwrap();
            let mut coordinator = Coordinator::new(&storage);
            let drive = |coordinator: &mut Coordinator| {
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
                loop {
                    while let Ok(event) = storage.events.try_recv() {
                        coordinator.complete(&event, &storage);
                    }
                    coordinator.advance(&storage);
                    if !coordinator.busy() {
                        break;
                    }
                    assert!(
                        std::time::Instant::now() < deadline,
                        "{:?}",
                        coordinator.view
                    );
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
            };
            drive(&mut coordinator);
            let root = repo.path().canonicalize().unwrap();
            let common = root.join(".git");
            let operation = uuid::Uuid::new_v4();
            storage
                .commands
                .send(crate::persistence::Command::Register(
                    operation,
                    CheckoutIdentity {
                        root,
                        git_dir: common.clone(),
                        common_dir: common,
                    },
                ))
                .unwrap();
            let registry = loop {
                let event = storage
                    .events
                    .recv_timeout(std::time::Duration::from_secs(3))
                    .unwrap();
                if let crate::persistence::Completion::Registered(id, registry) = event
                    && id == operation
                {
                    break registry.unwrap();
                }
            };
            let mut intent = record(repo.path(), &folder).intent;
            intent.project = registry.projects[0].id.clone();
            intent.start = true;
            std::fs::write(state.path().join("proof-storage-registry"), "error").unwrap();
            coordinator.handle(
                crate::events::Request::CreateWorkspace(intent.clone()),
                &registry,
                &storage,
                state.path(),
            );
            drive(&mut coordinator);
            assert_eq!(
                coordinator.view.retry_request.as_deref(),
                Some(intent.request.as_str())
            );
            assert!(!folder.exists());
            if cancel_before_recovery {
                coordinator.handle(
                    crate::events::Request::CancelCreation {
                        request: intent.request.clone(),
                        native_session: None,
                    },
                    &registry,
                    &storage,
                    state.path(),
                );
            }
            std::fs::remove_file(state.path().join("proof-storage-registry")).unwrap();
            coordinator.handle(
                crate::events::Request::RecoverCreation {
                    request: intent.request,
                    local: false,
                },
                &registry,
                &storage,
                state.path(),
            );
            drive(&mut coordinator);
            if cancel_before_recovery {
                assert!(coordinator.launch.is_none());
                assert!(!folder.exists());
                assert_eq!(
                    coordinator.view.records.last().unwrap().phase,
                    Phase::Cancelled
                );
            } else {
                let completed = coordinator
                    .launch
                    .take()
                    .expect("created workspace handoff");
                assert_eq!(completed.phase, Phase::Created, "{}", completed.message);
                assert!(
                    !completed.intent.start,
                    "reconciliation must never rearm an earlier launch"
                );
                assert!(folder.join(".git").exists());
            }
        }
    }
}

#[cfg(test)]
mod fetch_tests {
    use super::*;
    fn git(repo: &Path, args: &[&str]) -> String {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap().trim().into()
    }
    #[test]
    fn failed_remote_fetch_offers_exact_local_commit_and_never_creates_a_folder() {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path().join("repo");
        std::fs::create_dir(&repo).unwrap();
        git(&repo, &["init", "-b", "main"]);
        git(
            &repo,
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=t@example.com",
                "commit",
                "--allow-empty",
                "-m",
                "base",
            ],
        );
        let commit = git(&repo, &["rev-parse", "HEAD"]);
        git(
            &repo,
            &["remote", "add", "origin", "/nonexistent/rat-remote"],
        );
        git(&repo, &["update-ref", "refs/remotes/origin/main", &commit]);
        let folder = temp.path().join("workspace");
        let record = Record {
            intent: Intent {
                request: uuid::Uuid::new_v4().to_string(),
                project: "project".into(),
                base: "origin/main".into(),
                branch: "workspace/one".into(),
                path: crate::session::path_bytes(&folder),
                agent: NativeAgent::Codex,
                start: true,
                prompt: None,
            },
            repository: crate::session::path_bytes(&repo),
            common_dir: crate::session::path_bytes(&repo.join(".git").canonicalize().unwrap()),
            phase: Phase::Requested,
            launch_cancelled: false,
            base_ref: None,
            commit: None,
            message: String::new(),
            checkout: None,
        };
        let job = Job::start(Work::Prepare(record)).unwrap();
        let ResultEvent::Record(record) = job
            .result
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap()
        else {
            panic!("record expected")
        };
        assert_eq!(record.phase, Phase::LocalChoice, "{}", record.message);
        assert_eq!(record.commit.as_deref(), Some(commit.as_str()));
        assert!(!folder.exists());
        assert!(record.message.contains("Latest code could not be obtained"));
    }
}
