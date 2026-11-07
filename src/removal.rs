//! Explicit folder removal. Durable receipts precede Git mutation; restart never replays it.
use crate::{
    git::CheckoutIdentity,
    persistence::{Command, Completion, Worker},
    removal_git::Snapshot,
};
use serde::{Deserialize, Serialize};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    Preview,
    Intent,
    Protected,
    Removing,
    Removed,
    Failed,
    Uncertain,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Record {
    pub request: String,
    pub workspace: String,
    pub snapshot: Snapshot,
    pub phase: Phase,
    pub message: String,
}
impl Record {
    pub fn blocks(&self) -> bool {
        matches!(
            self.phase,
            Phase::Intent | Phase::Protected | Phase::Removing | Phase::Uncertain
        )
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct View {
    pub ready: bool,
    pub records: Vec<Record>,
    pub pending: bool,
    pub error: Option<String>,
}
pub struct Confirmation {
    pub workspace: String,
    pub request: String,
    pub head: String,
    pub reference: String,
    pub local_ack: bool,
}
#[derive(Clone, Copy)]
enum Step {
    Preview,
    Protect,
    Remove,
    Reconcile,
}
struct Job {
    cancel: Arc<AtomicBool>,
    result: mpsc::Receiver<Result<Record, String>>,
    thread: thread::JoinHandle<()>,
}
impl Job {
    fn start(
        step: Step,
        record: Option<Record>,
        workspace: Option<crate::registry::Workspace>,
        request: String,
        blockers: Vec<String>,
    ) -> std::io::Result<Self> {
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let (sender, result) = mpsc::sync_channel(1);
        let thread = thread::Builder::new().name("workspace-removal".into()).spawn(move || {
            let outcome = (|| {
                if matches!(step, Step::Preview) {
                    let workspace = workspace.unwrap();
                    let mut snapshot = crate::removal_git::preview(&workspace, &request, &flag)?;
                    snapshot.blockers.extend(blockers);
                    return Ok(Record { request, workspace: workspace.id, snapshot, phase: Phase::Preview, message: "Preview only; folder retained. Confirm the exact HEAD and recovery reference.".into() });
                }
                let mut r = record.unwrap();
                match step {
                    Step::Protect => {
                        crate::removal_git::protect(&r.snapshot, &flag)?;
                        r.phase = Phase::Protected;
                        r.message = "Exact HEAD protected locally; folder retained until durable removal intent".into();
                    }
                    Step::Remove => {
                        crate::removal_git::remove(&r.snapshot, &flag)?;
                        r.phase = Phase::Removed;
                        r.message = "Workspace folder removed. Saved commits, recovery refs, branches and chats retained. In-place native resume unavailable; readable history remains available.".into();
                    }
                    Step::Reconcile => {
                        if crate::removal_git::reconcile(&r.snapshot, &flag)? {
                            r.phase = Phase::Removed;
                            r.message = "Reconciled removed folder; saved code and history retained. No deletion replayed.".into();
                        } else {
                            r.phase = Phase::Failed;
                            r.message = "Reconciled retained folder; no deletion replayed. Open a new preview to request removal.".into();
                        }
                    }
                    Step::Preview => unreachable!(),
                }
                Ok(r)
            })();
            let _ = sender.send(outcome);
        })?;
        Ok(Self {
            cancel,
            result,
            thread,
        })
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
    }
}

pub struct Coordinator {
    pub view: View,
    loaded: bool,
    current: Option<Record>,
    saving: Option<(uuid::Uuid, Record)>,
    job: Option<Job>,
    next: Option<Step>,
    cancelled: bool,
}
impl Coordinator {
    pub fn new(storage: &Worker) -> Self {
        let error = storage
            .commands
            .try_send(Command::RemovalLoad)
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
            next: None,
            cancelled: false,
        }
    }
    pub fn ready(&self) -> bool {
        self.loaded
    }
    pub fn busy(&self) -> bool {
        self.job.is_some() || self.saving.is_some() || self.next.is_some()
    }
    pub fn blocks(&self, checkout: &CheckoutIdentity) -> bool {
        self.current
            .iter()
            .chain(self.view.records.iter())
            .any(|r| {
                r.blocks()
                    && r.snapshot.checkout.common_dir == checkout.common_dir
                    && r.snapshot.checkout.git_dir == checkout.git_dir
            })
    }
    pub fn target(&self) -> Option<&CheckoutIdentity> {
        self.current
            .as_ref()
            .filter(|r| r.blocks())
            .map(|r| &r.snapshot.checkout)
    }
    pub fn preview(
        &mut self,
        workspace: &crate::registry::Workspace,
        request: &str,
        blockers: Vec<String>,
    ) {
        if !self.loaded || self.busy() {
            self.view.error = Some("Folder removal recovery loading or worker pending".into());
            return;
        }
        if uuid::Uuid::parse_str(request).is_err() {
            self.view.error = Some("Removal request must be a UUID".into());
            return;
        }
        if let Some(old) = self.view.records.iter().find(|r| r.request == request) {
            self.view.error = (old.workspace != workspace.id)
                .then(|| "Removal request belongs to another workspace".into());
            return;
        }
        if workspace.checkout.as_ref().is_some_and(|c| self.blocks(c)) {
            self.view.error = Some(
                "Earlier removal requires read-only reconciliation before a new preview".into(),
            );
            return;
        }
        self.cancelled = false;
        self.current = None;
        self.view.error = None;
        match Job::start(
            Step::Preview,
            None,
            Some(workspace.clone()),
            request.into(),
            blockers,
        ) {
            Ok(job) => self.job = Some(job),
            Err(e) => self.view.error = Some(e.to_string()),
        }
    }
    pub fn confirm(&mut self, confirmation: Confirmation, blockers: Vec<String>, storage: &Worker) {
        let Confirmation {
            workspace,
            request,
            head,
            reference,
            local_ack,
        } = confirmation;
        if !self.loaded || self.busy() {
            self.view.error = Some("Removal receipt/worker pending; wait".into());
            return;
        }
        let Some(mut r) = self
            .view
            .records
            .iter()
            .find(|r| r.request == request && r.workspace == workspace)
            .cloned()
        else {
            self.view.error = Some("Exact removal preview unavailable".into());
            return;
        };
        if r.phase != Phase::Preview {
            self.view.error =
                Some("Recorded operation cannot be replayed; use read-only reconciliation".into());
            return;
        }
        if r.snapshot.head != head || r.snapshot.reference != reference {
            self.view.error = Some("Preview revision changed; reopen before confirmation".into());
            return;
        }
        if !r.snapshot.blockers.is_empty() || !blockers.is_empty() {
            self.view.error = Some(format!(
                "Removal blocked: {}",
                r.snapshot
                    .blockers
                    .iter()
                    .chain(blockers.iter())
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("; ")
            ));
            return;
        }
        if r.snapshot.local_only && !local_ack {
            self.view.error=Some("Saved code is local only, not pushed. Explicit local-only acknowledgement required".into());
            return;
        }
        self.cancelled = false;
        r.phase = Phase::Intent;
        r.message = "Confirmed exact preview; saving durable intent before protecting HEAD".into();
        self.current = Some(r.clone());
        self.view.error = None;
        self.persist(r, Some(Step::Protect), storage);
    }
    pub fn reconcile(&mut self, workspace: &str, request: &str) {
        if !self.loaded || self.busy() {
            self.view.error = Some("Removal receipt/worker pending; wait".into());
            return;
        }
        let Some(r) = self
            .view
            .records
            .iter()
            .find(|r| r.workspace == workspace && r.request == request && r.phase != Phase::Preview)
            .cloned()
        else {
            self.view.error = Some("No removal operation to reconcile".into());
            return;
        };
        self.cancelled = false;
        self.current = Some(r.clone());
        self.view.error = None;
        match Job::start(Step::Reconcile, Some(r), None, request.into(), vec![]) {
            Ok(j) => self.job = Some(j),
            Err(e) => self.view.error = Some(e.to_string()),
        }
    }
    fn publish(&mut self, record: Record) {
        self.view.records.retain(|r| r.request != record.request);
        self.view.records.push(record);
    }
    fn persist(&mut self, record: Record, next: Option<Step>, storage: &Worker) {
        let id = uuid::Uuid::new_v4();
        self.next = next;
        self.current = Some(record.clone());
        match storage
            .commands
            .try_send(Command::RemovalSave(id, Box::new(record.clone())))
        {
            Ok(()) => self.saving = Some((id, record)),
            Err(e) => self.storage_failed(e.to_string()),
        }
    }
    fn storage_failed(&mut self, error: String) {
        self.next = None;
        self.view.error = Some(format!(
            "Removal receipt not durable: {error}. No further deletion; retry storage, then reconcile exact request."
        ));
        if let Some(mut r) = self.current.clone() {
            if r.phase != Phase::Preview {
                r.phase = Phase::Uncertain;
                r.message = self.view.error.clone().unwrap();
            }
            self.current = Some(r.clone());
            self.publish(r);
        }
    }
    pub fn retry_storage(&mut self, storage: &Worker) {
        if !self.loaded {
            let _ = storage.commands.try_send(Command::RemovalLoad);
        } else if !self.busy()
            && self.view.error.is_some()
            && let Some(r) = self.current.clone()
        {
            self.persist(r, None, storage);
        }
    }
    pub fn complete(&mut self, event: &Completion) {
        match event {
            Completion::Removals(result) => match result {
                Ok(records) => {
                    self.loaded = true;
                    self.view.ready = true;
                    self.view.records = records.clone();
                }
                Err(e) => self.view.error = Some(e.clone()),
            },
            Completion::RemovalSaved(id, result)
                if self
                    .saving
                    .as_ref()
                    .is_some_and(|(pending, _)| pending == id) =>
            {
                let (_, r) = self.saving.take().unwrap();
                match result {
                    Ok(()) => {
                        self.publish(r.clone());
                        self.current = Some(r);
                    }
                    Err(e) => self.storage_failed(e.clone()),
                }
            }
            _ => {}
        }
    }
    pub fn advance(&mut self, eligible: bool, storage: &Worker) {
        if self.job.as_ref().is_some_and(|j| j.thread.is_finished()) {
            let job = self.job.take().unwrap();
            match job.result.try_recv() {
                Ok(Ok(mut r)) => {
                    if self.cancelled && r.phase != Phase::Removed {
                        r.phase = Phase::Uncertain;
                        r.message="Removal cancelled; reconcile retained code/folder before a new request".into();
                    }
                    let next = if !self.cancelled && r.phase == Phase::Protected {
                        Some(Step::Remove)
                    } else {
                        None
                    };
                    if next.is_some() {
                        r.phase = Phase::Removing;
                        r.message="Exact HEAD protected; saving durable removal intent before Git removal".into();
                    }
                    self.persist(r, next, storage);
                }
                result => {
                    let error = match result {
                        Ok(Err(e)) => e,
                        _ => {
                            "Worker ended without receipt; read-only reconciliation required".into()
                        }
                    };
                    self.view.error = Some(error.clone());
                    self.next = None;
                    if let Some(mut r) = self.current.clone() {
                        r.phase = Phase::Uncertain;
                        r.message = error;
                        self.persist(r, None, storage);
                    }
                }
            }
        }
        if self.saving.is_none()
            && self.job.is_none()
            && let Some(step) = self.next.take()
        {
            if self.cancelled || !eligible {
                if let Some(mut r) = self.current.clone() {
                    r.phase = Phase::Uncertain;
                    r.message="Removal stopped: execution, capture or archive blockers changed. Reconcile; folder retained unless Git had already completed.".into();
                    self.persist(r, None, storage);
                }
            } else {
                let r = self.current.clone().unwrap();
                match Job::start(step, Some(r.clone()), None, r.request, vec![]) {
                    Ok(job) => self.job = Some(job),
                    Err(e) => self.storage_failed(e.to_string()),
                }
            }
        }
        self.view.pending = self.busy();
    }
    pub fn cancel(&mut self) {
        self.cancelled = true;
        if let Some(job) = &self.job {
            job.cancel.store(true, Ordering::Release);
        }
    }
}
