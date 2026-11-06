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
    io::{self, Read, Write},
    os::unix::{io::AsRawFd, net::UnixListener, process::CommandExt},
    process::{Command as Process, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread,
    time::{Duration, Instant},
};
const MAX_RECORDS: usize = 256;
const OUTPUT: usize = 8192;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    Prepared,
    Authorized,
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
    pub output: String,
    pub discarded: u64,
    pub message: String,
}
impl Record {
    pub fn reserved(&self) -> bool {
        matches!(
            self.phase,
            Phase::Prepared | Phase::Authorized | Phase::Uncertain
        )
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct View {
    pub records: Vec<Record>,
    pub pending: bool,
    pub authorized_start: Option<String>,
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
    pending: Option<(uuid::Uuid, Record, bool)>,
    unsaved: Option<Record>,
    start: Option<String>,
    loaded: bool,
    cancelled: bool,
}
impl Coordinator {
    pub fn new(storage: &Worker) -> Self {
        let _ = storage.commands.try_send(Command::SetupLoad);
        Self {
            view: View::default(),
            launch: None,
            job: None,
            pending: None,
            unsaved: None,
            start: None,
            loaded: false,
            cancelled: false,
        }
    }
    pub fn busy(&self) -> bool {
        self.job.is_some() || self.pending.is_some() || self.unsaved.is_some()
    }
    pub fn admission_busy(&self) -> bool {
        !self.loaded || self.busy()
    }
    pub fn reservations(&self) -> usize {
        self.view.records.iter().filter(|r| r.reserved()).count()
    }
    pub fn reserved(&self, checkout: &CheckoutIdentity) -> bool {
        self.view
            .records
            .iter()
            .filter(|r| {
                r.reserved() || self.busy() && self.current_attempt() == Some(r.attempt.as_str())
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
            .or_else(|| {
                self.view
                    .records
                    .last()
                    .filter(|_| self.job.is_some())
                    .map(|r| r.attempt.as_str())
            })
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
            self.start = None;
            return;
        }
        self.cancelled = false;
        self.start = handoff
            .authorized_start
            .then(|| handoff.record.intent.request.clone());
        match Execution::prepared() {
            Ok(execution) => self.save(
                Record {
                    attempt: uuid::Uuid::new_v4().to_string(),
                    creation: handoff.record,
                    command,
                    phase: Phase::Prepared,
                    execution,
                    output: String::new(),
                    discarded: 0,
                    message: "Setup prepared; waiting for durable permission".into(),
                },
                storage,
            ),
            Err(e) => {
                self.start = None;
                self.view.error = Some(format!("Setup not prepared: {e}; workspace retained"));
            }
        }
    }
    pub fn cancel(&mut self, creation: Option<&str>) {
        let applies = creation.is_none()
            || creation == self.start.as_deref()
            || self
                .view
                .records
                .last()
                .is_some_and(|r| Some(r.creation.intent.request.as_str()) == creation);
        if applies {
            self.cancelled = true;
            self.start = None;
            self.launch = None;
            if let Some(job) = &self.job {
                job.cancel.store(true, Ordering::Release);
            }
        }
    }
    pub fn cancel_checkout(&mut self, checkout: &CheckoutIdentity) {
        if self.reserved(checkout) {
            self.cancel(None);
        }
    }
    pub fn retry_storage(&mut self, storage: &Worker) {
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
        if !matches!(
            record.phase,
            Phase::Failed
                | Phase::Cancelled
                | Phase::Uncertain
                | Phase::Prepared
                | Phase::Authorized
        ) {
            self.view.error = Some("Setup already settled; use explicit Start".into());
            return;
        }
        // Reconciliation is read-only and runs off the routing loop.
        if record.reserved() {
            if !informed {
                self.view.error=Some("Setup outcome uncertain. Informed retry may repeat external effects; reconcile survivors before retry/skip".into());
                return;
            }
            let mut next = record.clone();
            next.phase = Phase::Uncertain;
            self.start = None;
            let (tx, events) = mpsc::sync_channel(1);
            let (ack, _) = mpsc::sync_channel(1);
            let cancel = Arc::new(AtomicBool::new(false));
            let tail = Arc::new(Mutex::new(Tail::default()));
            let worker = thread::spawn(move || {
                match record.execution.reconcile() {
                    Ok(()) => {
                        next.execution.released = false;
                        next.phase = Phase::Failed;
                        next.message="No survivors after read-only reconciliation. Command outcome remains unknown; choose informed Retry or Continue without setup".into();
                    }
                    Err(e) => next.message = e,
                }
                let _ = tx.send(next);
            });
            self.job = Some(Job {
                cancel,
                events,
                acknowledgement: ack,
                tail,
                _thread: worker,
            });
            return;
        }
        if record.message.contains("outcome remains unknown") && !informed {
            self.view.error = Some("Retry may repeat external effects; use informed retry".into());
            return;
        }
        self.view.error = None;
        self.cancelled = false;
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
                            if r.reserved() {
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
                    if self.cancelled {
                        let mut r = record;
                        r.phase = Phase::Cancelled;
                        r.message = "Setup cancelled before spawn; pending launch disarmed".into();
                        self.save(r, storage);
                        return;
                    }
                    match Job::start(record.clone()) {
                        Ok(job) => self.job = Some(job),
                        Err(e) => {
                            let mut r = record;
                            r.phase = Phase::Failed;
                            r.message = e.to_string();
                            self.save(r, storage);
                        }
                    }
                } else if record.phase == Phase::Authorized {
                    if let Some(job) = &self.job {
                        let _ = job.acknowledgement.try_send(());
                    }
                } else if matches!(record.phase, Phase::Succeeded | Phase::Skipped)
                    && self.start.as_deref() == Some(&record.creation.intent.request)
                {
                    self.start = None;
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
        if self.pending.is_none()
            && self.unsaved.is_none()
            && let Some(record) = self.job.as_ref().and_then(|j| j.events.try_recv().ok())
        {
            if record.phase != Phase::Authorized {
                self.job = None;
            }
            self.save(record, storage);
        }
        self.view.pending = self.busy();
        self.view.authorized_start = self.start.clone();
        if let Some(job) = &self.job
            && let Some(record) = self.view.records.last_mut()
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
