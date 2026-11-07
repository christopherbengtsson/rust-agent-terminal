//! Workspace archive checkpoints. Git execution stays in an owned bounded worker;
//! durable phase receipts are written by the sole persistence writer.
use crate::git::CheckoutIdentity;
use serde::{Deserialize, Serialize};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum BranchPolicy {
    Keep,
    Delete(String),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Record {
    #[serde(default)]
    pub delete_target: Option<String>,
    #[serde(default)]
    pub owned_branch: Option<String>,
    #[serde(default)]
    pub protected_base: Option<String>,
    #[serde(default)]
    pub branch_phase: BranchPhase,
    #[serde(default)]
    pub branch_message: String,
    pub request: String,
    pub workspace: String,
    pub checkout: CheckoutIdentity,
    pub phase: Phase,
    pub before: Option<String>,
    pub branch: Option<String>,
    pub tree: Option<String>,
    pub index_tree: Option<String>,
    pub index_hash: Option<String>,
    pub saved: Option<String>,
    pub reference: String,
    pub excluded: Vec<Vec<u8>>,
    pub message: String,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum BranchPhase {
    #[default]
    None,
    CleanupPending,
    Cleaned,
    Retained,
    RestorePending,
    Restored,
    RestoreFailed,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    Waiting,
    Prepared,
    Saved,
    Archived,
    Failed,
    Cancelled,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct View {
    #[serde(default)]
    pub status: String,
    pub records: Vec<Record>,
    pub pending: bool,
    pub error: Option<String>,
}
impl Record {
    pub fn new(workspace: &crate::registry::Workspace, request: &str) -> Result<Self, String> {
        uuid::Uuid::parse_str(request).map_err(|_| "Archive request must be a UUID")?;
        uuid::Uuid::parse_str(&workspace.id)
            .map_err(|_| "Archive workspace identity unavailable")?;
        Ok(Self {
            delete_target: workspace
                .creation
                .as_ref()
                .filter(|f| f.created_branch)
                .map(|f| format!("refs/heads/{}", f.original_branch)),
            owned_branch: workspace
                .creation
                .as_ref()
                .filter(|f| f.created_branch)
                .map(|f| format!("refs/heads/{}", f.original_branch)),
            protected_base: workspace
                .creation
                .as_ref()
                .map(|f| f.original_base_ref.clone()),
            branch_phase: BranchPhase::None,
            branch_message: String::new(),
            request: request.into(),
            workspace: workspace.id.clone(),
            checkout: workspace
                .checkout
                .clone()
                .ok_or("Canonical checkout unavailable")?,
            phase: Phase::Waiting,
            before: None,
            branch: None,
            tree: None,
            index_tree: None,
            index_hash: None,
            saved: None,
            reference: format!("refs/rat/archive/{}/{}", workspace.id, request),
            excluded: Vec::new(),
            message: "Stopping owned coding/setup execution and settling capture before save"
                .into(),
        })
    }
    pub fn blocks(&self) -> bool {
        !matches!(
            self.phase,
            Phase::Archived | Phase::Failed | Phase::Cancelled
        ) || matches!(
            self.branch_phase,
            BranchPhase::CleanupPending | BranchPhase::RestorePending
        )
    }
}
pub struct Job {
    cancel: Arc<AtomicBool>,
    thread: thread::JoinHandle<()>,
    pub result: mpsc::Receiver<Result<Record, String>>,
}
impl Job {
    pub fn start(record: Record) -> std::io::Result<Self> {
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let (tx, result) = mpsc::sync_channel(1);
        let thread = thread::Builder::new()
            .name("archive-checkpoint".into())
            .spawn(move || {
                let outcome = match record.phase {
                    Phase::Waiting => crate::archive_git::prepare(record, &worker_cancel),
                    Phase::Prepared | Phase::Saved => {
                        crate::archive_git::save(record, &worker_cancel)
                    }
                    Phase::Archived => {
                        let mut r = record;
                        if r.branch_phase == BranchPhase::RestorePending {
                            match crate::archive_restore::restore(&r, &worker_cancel) {
                                Ok(branch) => {
                                    r.branch_phase = BranchPhase::Restored;
                                    r.branch_message =
                                        format!("Restored branch {branch}; coding remains stopped");
                                }
                                Err(error) => {
                                    r.branch_phase = BranchPhase::RestoreFailed;
                                    r.branch_message = format!(
                                        "Return unavailable: {error}; coding remains stopped"
                                    );
                                }
                            }
                        } else {
                            match crate::archive_branch::cleanup(&r, &worker_cancel) {
                                Ok(message) => {
                                    r.branch_phase = BranchPhase::Cleaned;
                                    r.branch_message = message;
                                }
                                Err(error) => {
                                    r.branch_phase = BranchPhase::Retained;
                                    r.branch_message =
                                        format!("Branch retained; cleanup incomplete: {error}");
                                }
                            }
                        }
                        Ok(r)
                    }
                    _ => Err("Archive phase cannot execute Git".into()),
                };
                let _ = tx.send(outcome);
            })?;
        Ok(Self {
            cancel,
            thread,
            result,
        })
    }
    pub fn finished(&self) -> bool {
        self.thread.is_finished()
    }
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Release)
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancel()
    }
}

pub struct Coordinator {
    pub view: View,
    loaded: bool,
    current: Option<Record>,
    saving: Option<(uuid::Uuid, Record)>,
    job: Option<Job>,
    armed: bool,
    cancelled: bool,
}
impl Coordinator {
    pub fn new(storage: &crate::persistence::Worker) -> Self {
        let error = storage
            .commands
            .try_send(crate::persistence::Command::ArchiveLoad)
            .err()
            .map(|e| e.to_string());
        Self {
            view: View {
                error,
                ..View::default()
            },
            loaded: false,
            current: None,
            saving: None,
            job: None,
            armed: false,
            cancelled: false,
        }
    }
    pub fn ready(&self) -> bool {
        self.loaded
    }
    pub fn busy(&self) -> bool {
        self.job.is_some() || self.saving.is_some() || self.armed
    }
    pub fn blocks(&self, checkout: &CheckoutIdentity) -> bool {
        self.current.as_ref().is_some_and(|r| {
            (self.busy() || r.blocks())
                && r.checkout.common_dir == checkout.common_dir
                && r.checkout.git_dir == checkout.git_dir
        }) || self.view.records.iter().any(|r| {
            r.blocks()
                && r.checkout.common_dir == checkout.common_dir
                && r.checkout.git_dir == checkout.git_dir
        })
    }
    pub fn begin(
        &mut self,
        workspace: &crate::registry::Workspace,
        request: &str,
        storage: &crate::persistence::Worker,
    ) {
        self.begin_policy(workspace, request, None, storage);
    }
    pub fn begin_policy(
        &mut self,
        workspace: &crate::registry::Workspace,
        request: &str,
        policy: Option<Option<String>>,
        storage: &crate::persistence::Worker,
    ) {
        if !self.loaded || self.busy() {
            self.view.error =
                Some("Archive receipt loading or operation pending; wait or cancel".into());
            return;
        }
        if self
            .view
            .records
            .iter()
            .any(|r| r.workspace == workspace.id && r.request != request && r.blocks())
        {
            self.view.error=Some("An earlier archive request is unresolved; retry/cancel its receipt before creating another".into());
            return;
        }
        let existing = self.view.records.iter().find(|r| r.request == request);
        if existing.is_some_and(|r| r.workspace != workspace.id) {
            self.view.error = Some("Archive request already belongs to another workspace".into());
            return;
        }
        let record = existing
            .cloned()
            .map(Ok)
            .unwrap_or_else(|| Record::new(workspace, request));
        match record {
            Ok(mut r) => {
                self.cancelled = false;
                if existing.is_none()
                    && let Some(target) = policy
                {
                    r.delete_target = target;
                }
                if matches!(r.phase, Phase::Archived) {
                    self.armed = r.branch_phase == BranchPhase::CleanupPending;
                    self.current = Some(r.clone());
                    self.view.error = None;
                    if self.armed {
                        self.persist(r, storage);
                    }
                    return;
                }
                // Failed command replies retain the prepared snapshot so same-request
                // recovery can inspect an already-created commit, never blindly retry it.
                if matches!(r.phase, Phase::Failed | Phase::Cancelled) {
                    r.phase = if r.tree.is_some() {
                        Phase::Prepared
                    } else {
                        Phase::Waiting
                    }
                }
                self.armed = true;
                self.current = Some(r.clone());
                self.view.error = None;
                self.persist(r, storage);
            }
            Err(e) => self.view.error = Some(e),
        }
    }
    pub fn restore(
        &mut self,
        workspace: &str,
        request: &str,
        storage: &crate::persistence::Worker,
    ) {
        if !self.loaded || self.busy() {
            self.view.error = Some("Archive/return operation pending".into());
            return;
        }
        let Some(mut record) = self
            .view
            .records
            .iter()
            .rev()
            .find(|r| r.workspace == workspace && r.phase == Phase::Archived)
            .cloned()
        else {
            self.view.error = Some("Saved archive recovery unavailable".into());
            return;
        };
        if record.request != request {
            self.view.error =
                Some("Archive revision changed; reopen its exact receipt before Return".into());
            return;
        }
        if record.branch_phase == BranchPhase::Restored {
            self.view.error = None;
            return;
        }
        self.cancelled = false;
        record.branch_phase = BranchPhase::RestorePending;
        self.armed = true;
        self.current = Some(record.clone());
        self.view.error = None;
        self.persist(record, storage);
    }
    fn persist(&mut self, mut record: Record, storage: &crate::persistence::Worker) {
        if record.phase == Phase::Saved {
            record.phase = Phase::Archived;
            record.branch_phase = if record.delete_target.is_some() {
                BranchPhase::CleanupPending
            } else {
                BranchPhase::None
            };
            record.branch_message = if record.delete_target.is_some() {
                "Branch cleanup pending durable archive confirmation"
            } else {
                "Local branch retained by archive policy"
            }
            .into();
        }
        let id = uuid::Uuid::new_v4();
        match storage
            .commands
            .try_send(crate::persistence::Command::ArchiveSave(
                id,
                Box::new(record.clone()),
            )) {
            Ok(()) => self.saving = Some((id, record)),
            Err(e) => {
                self.armed = false;
                self.view.error = Some(format!(
                    "Archive phase not durable: {e}; retry same request"
                ));
                self.current = Some(record)
            }
        }
    }
    pub fn complete(
        &mut self,
        event: &crate::persistence::Completion,
    ) -> Option<crate::registry::Registry> {
        match event {
            crate::persistence::Completion::Archives(result) => {
                self.loaded = true;
                match result {
                    Ok(records) => self.view.records = records.clone(),
                    Err(e) => {
                        self.loaded = false;
                        self.view.error = Some(e.clone())
                    }
                }
            }
            crate::persistence::Completion::ArchiveSaved(id, result)
                if self
                    .saving
                    .as_ref()
                    .is_some_and(|(pending, _)| pending == id) =>
            {
                let (_, mut record) = self.saving.take().unwrap();
                match result {
                    Ok(registry) => {
                        if record.phase == Phase::Saved {
                            record.phase = Phase::Archived
                        }
                        self.view.records.retain(|r| r.request != record.request);
                        self.view.records.push(record.clone());
                        self.current = Some(record.clone());
                        if record.phase == Phase::Archived {
                            self.armed = self.armed
                                && matches!(
                                    record.branch_phase,
                                    BranchPhase::CleanupPending | BranchPhase::RestorePending
                                );
                        }
                        return Some(registry.clone());
                    }
                    Err(e) => {
                        self.current = Some(record);
                        self.armed = false;
                        self.view.error = Some(format!(
                            "Archive metadata not saved: {e}; last durable visibility retained, retry same request"
                        ))
                    }
                }
            }
            _ => {}
        }
        None
    }
    pub fn advance(&mut self, settled: bool, storage: &crate::persistence::Worker) {
        if self.job.as_ref().is_some_and(Job::finished) {
            let job = self.job.take().unwrap();
            match job.result.try_recv() {
                Ok(Ok(mut r)) => {
                    if self.cancelled && r.branch_phase == BranchPhase::Restored {
                        r.branch_phase = BranchPhase::RestoreFailed;
                        r.branch_message = "Return cancelled; any created branch/checkout retained; workspace remains archived and coding stopped".into();
                    }
                    if !self.armed && r.phase != Phase::Archived {
                        r.phase = Phase::Cancelled;
                        r.message = format!("Archive cancelled; {}", r.message);
                    }
                    self.current = Some(r.clone());
                    self.persist(r, storage)
                }
                result => {
                    let mut r = self.current.clone().unwrap();
                    let error = match result {
                        Ok(Err(e)) => e,
                        _ => "Worker ended without a receipt; retry exact request to reconcile"
                            .into(),
                    };
                    if r.phase == Phase::Archived {
                        r.branch_phase = if r.branch_phase == BranchPhase::RestorePending {
                            BranchPhase::RestoreFailed
                        } else {
                            BranchPhase::Retained
                        };
                        r.branch_message =
                            format!("Saved archive retained; branch outcome uncertain: {error}");
                    } else {
                        r.phase = if self.armed {
                            Phase::Failed
                        } else {
                            Phase::Cancelled
                        };
                        r.message = error.clone();
                    }
                    self.view.error = Some(error);
                    self.armed = false;
                    self.persist(r, storage)
                }
            }
        }
        if self.armed
            && settled
            && self.job.is_none()
            && self.saving.is_none()
            && let Some(r) = self.current.clone()
        {
            match Job::start(r) {
                Ok(job) => self.job = Some(job),
                Err(e) => {
                    self.armed = false;
                    self.view.error = Some(e.to_string())
                }
            }
        }
        if !self.armed
            && self.job.is_none()
            && self.saving.is_none()
            && let Some(mut r) = self.current.clone()
            && r.blocks()
            && (r.phase != Phase::Archived || self.cancelled)
        {
            if r.phase == Phase::Archived {
                r.branch_phase = if r.branch_phase == BranchPhase::RestorePending {
                    BranchPhase::RestoreFailed
                } else {
                    BranchPhase::Retained
                };
                r.branch_message = "Branch operation cancelled; saved archive retained".into();
            } else {
                r.phase = Phase::Cancelled;
                r.message="Archive cancelled; execution remains stopped. Retry same request reconciles any partial save".into();
            }
            self.current = Some(r.clone());
            self.persist(r, storage);
        }
        self.view.pending = self.busy();
        self.view.status=if self.saving.as_ref().is_some_and(|(_,r)|r.phase==Phase::Archived){"Checkpoint protected locally; waiting for durable archive metadata"}
            else if self.saving.is_some(){"Waiting for durable archive phase receipt"}
            else if self.job.is_some() && self.current.as_ref().is_some_and(|r|r.branch_phase==BranchPhase::RestorePending){"Restoring local branch; coding remains stopped"}
            else if self.job.is_some() && self.current.as_ref().is_some_and(|r|r.phase==Phase::Archived){"Archive saved; cleaning local branch (failure retains saved archive)"}
            else if self.job.is_some(){"Saving local checkpoint; Git identity, hooks and signing apply"}
            else if self.armed&&!settled{"Waiting for confirmed Stop, setup cleanup and durable history; uncertainty blocks saving"}
            else if !self.loaded{"Loading archive recovery receipts"}else{"Archive operation settled"}.into();
    }
    pub fn target(&self) -> Option<&CheckoutIdentity> {
        self.current
            .as_ref()
            .filter(|_| self.armed)
            .map(|r| &r.checkout)
    }
    pub fn cancel_request(&mut self, workspace: &str, request: &str) {
        if self
            .current
            .as_ref()
            .is_some_and(|r| r.workspace == workspace && r.request == request)
        {
            self.cancel();
            return;
        }
        if self.busy() {
            self.view.error =
                Some("Another archive operation is pending; cancel its exact receipt".into());
            return;
        }
        if let Some(record) = self
            .view
            .records
            .iter()
            .find(|r| r.workspace == workspace && r.request == request)
            .cloned()
        {
            self.current = Some(record);
            self.cancel();
        }
    }
    pub fn cancel(&mut self) {
        if self.saving.as_ref().is_some_and(|(_, r)| {
            r.branch_phase == BranchPhase::Restored || r.branch_phase == BranchPhase::Cleaned
        }) {
            self.view.error=Some("Git branch operation already completed; durable result is pending and cannot be cancelled. Coding remains stopped".into());
            return;
        }
        // A failed result write may leave current Git success in memory while
        // the last durable record is still Prepared. Cancel that durable intent.
        if !self.busy()
            && let Some(current) = &self.current
            && let Some(previous) = self
                .view
                .records
                .iter()
                .find(|r| r.request == current.request && r.blocks())
                .cloned()
        {
            self.current = Some(previous);
        }
        if self.current.is_none()
            || !self.busy() && self.current.as_ref().is_some_and(|r| !r.blocks())
        {
            return;
        }
        self.cancelled = true;
        self.armed = false;
        if let Some(job) = &self.job {
            job.cancel()
        }
        self.view.error=Some("Archive cancelled; Stop remains in effect. A partial checkpoint/ref may exist locally; retry same request reconciles it".into());
    }
}
