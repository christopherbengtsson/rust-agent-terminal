//! Project setup is owned local coding-workspace execution, never a conversation.
//! Saved receipts grant no execution or launch permission on restart.
use crate::setup_execution::Job;
use crate::{
    creation::{LaunchHandoff, Record as Creation},
    git::CheckoutIdentity,
    persistence::{Command, Completion, Worker},
    recovery::Execution,
};
use serde::{Deserialize, Serialize};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
};
const MAX_RECORDS: usize = 32;
const OUTPUT: usize = 8192;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Policy {
    Disabled,
    Command(String),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    Prepared,
    Authorized,
    Running,
    Succeeded,
    Failed,
    Cancelled,
    Uncertain,
    Skipped,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Record {
    pub attempt: String,
    pub creation: Creation,
    pub command: String,
    pub phase: Phase,
    pub execution: Execution,
    #[serde(default)]
    pub cleanup_confirmed: bool,
    pub output: String,
    pub discarded: u64,
    pub message: String,
}
impl Record {
    pub fn reserved(&self) -> bool {
        matches!(
            self.phase,
            Phase::Prepared | Phase::Authorized | Phase::Running | Phase::Uncertain | Phase::Failed
        )
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct View {
    pub records: Vec<Record>,
    pub pending: bool,
    pub armed_launches: Vec<String>,
    pub error: Option<String>,
}
#[derive(Default)]
pub(crate) struct Tail {
    bytes: Vec<u8>,
    discarded: u64,
}
impl Tail {
    pub(crate) fn append(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
        if self.bytes.len() > OUTPUT {
            let n = self.bytes.len() - OUTPUT;
            self.bytes.drain(..n);
            self.discarded += n as u64;
        }
    }
    pub(crate) fn apply(&self, record: &mut Record) {
        record.output = String::from_utf8_lossy(&self.bytes)
            .chars()
            .map(|c| {
                if c.is_control() && c != '\n' && c != '\t' {
                    '�'
                } else {
                    c
                }
            })
            .collect();
        record.discarded = self.discarded;
    }
}
pub struct Coordinator {
    pub view: View,
    pub launch: Option<LaunchHandoff>,
    job: Option<Job>,
    job_attempt: Option<String>,
    pending: Option<(uuid::Uuid, Record, bool)>,
    unsaved: Option<Record>,
    armed: std::collections::BTreeSet<String>,
    loaded: bool,
    cancelled: std::collections::BTreeSet<String>,
}
impl Coordinator {
    pub fn new(storage: &Worker) -> Self {
        let _ = storage.commands.try_send(Command::SetupLoad);
        Self {
            view: View::default(),
            launch: None,
            job: None,
            job_attempt: None,
            pending: None,
            unsaved: None,
            armed: Default::default(),
            loaded: false,
            cancelled: Default::default(),
        }
    }
    pub fn busy(&self) -> bool {
        self.job.is_some() || self.pending.is_some() || self.unsaved.is_some()
    }
    pub fn stopped(&self) -> bool {
        self.loaded
            && !self.busy()
            && !self.view.records.iter().any(|r| {
                !r.cleanup_confirmed
                    && matches!(
                        r.phase,
                        Phase::Prepared | Phase::Authorized | Phase::Running | Phase::Uncertain
                    )
                    && self
                        .view
                        .records
                        .iter()
                        .rev()
                        .find(|latest| latest.creation.intent.request == r.creation.intent.request)
                        .is_some_and(|latest| latest.attempt == r.attempt)
            })
    }
    pub fn owns_original_launch(&self, task: &str, launch: &str) -> bool {
        task == launch
            && self
                .view
                .records
                .iter()
                .any(|r| r.creation.intent.request == task)
    }
    pub fn ready(&self) -> bool {
        self.loaded
    }
    pub fn capacity(&self) -> bool {
        self.view.records.len() < MAX_RECORDS
    }
    pub fn admission_busy(&self) -> bool {
        !self.loaded || self.busy()
    }

    pub fn reserved(&self, checkout: &CheckoutIdentity) -> bool {
        self.view
            .records
            .iter()
            .filter(|r| {
                !self
                    .view
                    .records
                    .iter()
                    .rev()
                    .find(|other| other.creation.intent.request == r.creation.intent.request)
                    .is_some_and(|latest| latest.attempt != r.attempt)
                    && (r.reserved()
                        || self.busy() && self.current_attempt() == Some(r.attempt.as_str()))
            })
            .any(|r| {
                r.creation.checkout.as_ref().is_some_and(|c| {
                    c.common_dir == checkout.common_dir && c.git_dir == checkout.git_dir
                })
            })
    }
    fn current_attempt(&self) -> Option<&str> {
        self.pending
            .as_ref()
            .map(|(_, r, _)| r.attempt.as_str())
            .or_else(|| self.unsaved.as_ref().map(|r| r.attempt.as_str()))
            .or(self.job_attempt.as_deref())
    }
    fn publish(&mut self, record: Record) {
        if let Some(old) = self
            .view
            .records
            .iter_mut()
            .find(|r| r.attempt == record.attempt)
        {
            *old = record;
        } else {
            self.view.records.push(record);
        }
    }
    fn save(&mut self, record: Record, storage: &Worker) {
        let id = uuid::Uuid::new_v4();
        self.publish(record.clone());
        self.unsaved = Some(record.clone());
        match storage
            .commands
            .try_send(Command::SetupSave(id, Box::new(record.clone())))
        {
            Ok(()) => {
                self.pending = Some((id, record, true));
                self.unsaved = None;
            }
            Err(e) => {
                self.view.error = Some(format!("Setup outcome not saved: {e}; retry storage"))
            }
        }
    }
    pub fn accept(&mut self, handoff: LaunchHandoff, command: Option<String>, storage: &Worker) {
        let Some(command) = command else {
            self.launch = Some(handoff);
            return;
        };
        if self.admission_busy() || self.view.records.len() >= MAX_RECORDS {
            self.view.error=Some("Setup capacity unavailable; workspace retained. Explicit recovery required before Start".into());
            return;
        }
        if handoff.authorized_start {
            self.armed.insert(handoff.record.intent.request.clone());
        }
        match Execution::prepared() {
            Ok(execution) => self.save(
                Record {
                    attempt: uuid::Uuid::new_v4().to_string(),
                    creation: handoff.record,
                    command,
                    phase: Phase::Prepared,
                    execution,
                    cleanup_confirmed: false,
                    output: String::new(),
                    discarded: 0,
                    message: "Setup prepared; waiting for durable permission".into(),
                },
                storage,
            ),
            Err(e) => {
                self.armed.remove(&handoff.record.intent.request);
                self.view.error = Some(format!("Setup not prepared: {e}; workspace retained"));
            }
        }
    }
    pub fn cancel(&mut self, creation: Option<&str>) {
        let current = self.current_attempt().map(str::to_owned);
        let matches = |r: &Record| creation.is_none_or(|id| r.creation.intent.request == id);
        let stop_job = current
            .as_deref()
            .and_then(|id| self.view.records.iter().find(|r| r.attempt == id))
            .is_some_and(matches);
        for r in self.view.records.iter().filter(|r| matches(r)) {
            self.cancelled.insert(r.attempt.clone());
            self.armed.remove(&r.creation.intent.request);
        }
        if creation.is_none() {
            self.armed.clear();
        }
        if self
            .launch
            .as_ref()
            .is_some_and(|l| creation.is_none_or(|id| l.record.intent.request == id))
        {
            self.launch = None;
        }
        if stop_job && let Some(job) = &self.job {
            job.cancel.store(true, Ordering::Release);
        }
        self.view.armed_launches = self.armed.iter().cloned().collect();
        if self.job.is_none()
            && self
                .view
                .records
                .iter()
                .any(|r| matches(r) && !r.cleanup_confirmed && r.phase == Phase::Uncertain)
        {
            self.view.error=Some("Setup Stop unconfirmed: restored process authority is unavailable. Reconcile survivors; saved PIDs are never signalled. Detach remains available".into());
        }
    }
    pub fn cancel_checkout(&mut self, checkout: &CheckoutIdentity) {
        let requests: Vec<_> = self
            .view
            .records
            .iter()
            .filter(|r| {
                r.creation.checkout.as_ref().is_some_and(|c| {
                    c.common_dir == checkout.common_dir && c.git_dir == checkout.git_dir
                })
            })
            .map(|r| r.creation.intent.request.clone())
            .collect();
        for request in requests {
            self.cancel(Some(&request));
        }
    }
    pub fn retry_storage(&mut self, storage: &Worker) {
        if !self.loaded {
            if let Err(e) = storage.commands.try_send(Command::SetupLoad) {
                self.view.error = Some(format!("Setup receipts not loaded: {e}"));
            }
            return;
        }
        if self.pending.is_none()
            && let Some(record) = self.unsaved.take()
        {
            self.save(record, storage);
        }
    }
    pub fn recover(&mut self, attempt: &str, retry: bool, informed: bool, storage: &Worker) {
        if self.busy() {
            self.view.error = Some(
                "Setup still owns execution or unsaved outcome; Stop or retry storage first".into(),
            );
            return;
        }
        let Some(record) = self
            .view
            .records
            .iter()
            .find(|r| r.attempt == attempt)
            .cloned()
        else {
            self.view.error = Some("Setup attempt unavailable".into());
            return;
        };
        if self
            .view
            .records
            .iter()
            .rev()
            .find(|r| r.creation.intent.request == record.creation.intent.request)
            .is_some_and(|r| r.attempt != record.attempt)
        {
            self.view.error = Some("Setup attempt superseded; recover the latest attempt".into());
            return;
        }
        if !matches!(
            record.phase,
            Phase::Failed
                | Phase::Cancelled
                | Phase::Uncertain
                | Phase::Prepared
                | Phase::Authorized
                | Phase::Running
        ) {
            self.view.error = Some("Setup already settled; use explicit Start".into());
            return;
        }
        // Reconciliation is read-only and runs off the routing loop.
        if !record.cleanup_confirmed
            && matches!(
                record.phase,
                Phase::Prepared | Phase::Authorized | Phase::Running | Phase::Uncertain
            )
        {
            if !informed {
                self.view.error=Some("Setup outcome uncertain. Informed retry may repeat external effects; reconcile survivors before retry/skip".into());
                return;
            }
            let mut next = record.clone();
            next.phase = Phase::Uncertain;
            self.armed.remove(&record.creation.intent.request);
            let (tx, events) = mpsc::sync_channel(1);
            let (ack, _) = mpsc::sync_channel(1);
            let cancel = Arc::new(AtomicBool::new(false));
            let tail = Arc::new(Mutex::new(Tail::default()));
            let worker = thread::spawn(move || {
                match record.execution.reconcile() {
                    Ok(()) => {
                        next.cleanup_confirmed = true;
                        next.phase = Phase::Failed;
                        next.message="No survivors after read-only reconciliation. Command outcome remains unknown; choose informed Retry or Continue without setup".into();
                    }
                    Err(e) => next.message = e,
                }
                let _ = tx.send(next);
            });
            self.job_attempt = Some(attempt.to_owned());
            self.job = Some(Job {
                cancel,
                events,
                acknowledgement: ack,
                tail,
                _thread: worker,
            });
            return;
        }
        if (record.phase == Phase::Uncertain || record.message.contains("outcome remains unknown"))
            && !informed
        {
            self.view.error = Some("Retry may repeat external effects; use informed retry".into());
            return;
        }
        self.view.error = None;

        let mut next = record;
        if retry {
            if self.view.records.len() >= MAX_RECORDS {
                self.view.error =
                    Some("Setup receipt capacity reached; retained outcomes preserved".into());
                return;
            }
            match Execution::prepared() {
                Ok(e) => next.execution = e,
                Err(e) => {
                    self.view.error = Some(e.to_string());
                    return;
                }
            }
            next.attempt = uuid::Uuid::new_v4().to_string();
            next.phase = Phase::Prepared;
            next.cleanup_confirmed = false;
            next.output.clear();
            next.discarded = 0;
            next.message = "Explicit setup retry; command may repeat external effects".into();
        } else {
            next.phase = Phase::Skipped;
            next.message = "Explicit Continue without setup; no claim of command success".into();
        }
        self.save(next, storage);
    }
    pub fn complete(&mut self, event: &Completion, storage: &Worker) {
        match event {
            Completion::Setups(result) => {
                self.loaded = true;
                match result {
                    Ok(records) => {
                        for mut r in records.clone() {
                            if matches!(
                                r.phase,
                                Phase::Prepared
                                    | Phase::Authorized
                                    | Phase::Running
                                    | Phase::Uncertain
                            ) {
                                r.phase = Phase::Uncertain;
                                r.message="Restored setup outcome uncertain; no automatic retry or native launch. Reconcile survivors before informed Retry/Continue".into();
                            }
                            self.publish(r);
                        }
                    }
                    Err(e) => {
                        self.loaded = false;
                        self.view.error = Some(e.clone());
                    }
                }
            }
            Completion::SetupSaved(id, result)
                if self
                    .pending
                    .as_ref()
                    .is_some_and(|(pending, _, _)| pending == id) =>
            {
                let (_, record, _) = self.pending.take().unwrap();
                if let Err(e) = result {
                    self.unsaved = Some(record);
                    self.view.error = Some(format!("Setup outcome not saved: {e}; retry storage"));
                    return;
                }
                self.publish(record.clone());
                self.view.error = None;
                if record.phase == Phase::Prepared && self.job.is_none() {
                    // Stop while a prepare save was outstanding must never spawn a child.
                    if self.cancelled.contains(&record.attempt) {
                        let mut r = record;
                        r.phase = Phase::Cancelled;
                        r.message = "Setup cancelled before spawn; pending launch disarmed".into();
                        self.save(r, storage);
                        return;
                    }
                    match Job::start(record.clone()) {
                        Ok(job) => {
                            self.job_attempt = Some(record.attempt.clone());
                            self.job = Some(job);
                        }
                        Err(e) => {
                            let mut r = record;
                            r.phase = Phase::Failed;
                            r.message = e.to_string();
                            self.save(r, storage);
                        }
                    }
                } else if matches!(record.phase, Phase::Authorized | Phase::Running) {
                    if let Some(job) = &self.job {
                        let _ = job.acknowledgement.try_send(());
                    }
                } else if matches!(record.phase, Phase::Succeeded | Phase::Skipped)
                    && self.armed.contains(&record.creation.intent.request)
                {
                    self.armed.remove(&record.creation.intent.request);
                    self.launch = Some(LaunchHandoff {
                        record: record.creation,
                        authorized_start: true,
                    });
                }
            }
            _ => {}
        }
    }
    pub fn advance(&mut self, storage: &Worker) {
        if self.pending.is_none() && self.unsaved.is_none() {
            match self.job.as_ref().map(|j| j.events.try_recv()) {
                Some(Ok(record)) => {
                    if !matches!(record.phase, Phase::Authorized | Phase::Running) {
                        self.job = None;
                        self.job_attempt = None;
                    }
                    self.save(record, storage);
                }
                Some(Err(mpsc::TryRecvError::Disconnected)) => {
                    self.job = None;
                    if let Some(mut r) = self
                        .view
                        .records
                        .iter()
                        .find(|r| Some(&r.attempt) == self.job_attempt.as_ref())
                        .cloned()
                    {
                        r.phase = Phase::Uncertain;
                        r.cleanup_confirmed = false;
                        r.message="Setup worker ended without an outcome; reconcile survivors before explicit informed recovery".into();
                        self.armed.remove(&r.creation.intent.request);
                        self.save(r, storage);
                    }
                    self.job_attempt = None;
                }
                _ => {}
            }
        }
        self.view.pending = self.busy();
        self.view.armed_launches = self.armed.iter().cloned().collect();
        if let Some(job) = &self.job
            && let Some(record) = self
                .view
                .records
                .iter_mut()
                .find(|r| Some(&r.attempt) == self.job_attempt.as_ref())
        {
            job.tail.lock().unwrap().apply(record);
        }
    }
}
pub fn validate_command(command: &Option<String>) -> Result<(), String> {
    if command
        .as_ref()
        .is_some_and(|c| c.trim().is_empty() || c.len() > 4096 || c.contains('\0'))
    {
        Err("Setup shell command must contain 1–4096 bytes without NUL; empty configuration disables setup".into())
    } else {
        Ok(())
    }
}
