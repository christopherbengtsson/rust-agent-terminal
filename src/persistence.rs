//! The supervisor's sole SQLite/file writer. No disk operation runs on the UI thread.
pub mod search;
use crate::session::{Lifecycle, Observation};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    os::{
        fd::AsRawFd,
        unix::{
            ffi::{OsStrExt, OsStringExt},
            fs::OpenOptionsExt,
        },
    },
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread,
    time::{Duration, Instant},
};
const CHUNK: u64 = 65536;
pub const PAGE: u64 = 2048;
const FLUSH: Duration = Duration::from_millis(250);
type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct HistoryPage {
    pub identity: String,
    #[serde(default)]
    pub session: String,
    #[serde(default)]
    pub committed: u64,
    pub checkout: String,
    pub lifecycle: Lifecycle,
    #[serde(default)]
    pub process_exit: Option<crate::events::ProcessExit>,
    /// Escaped raw bytes; retained separately from the readable projection.
    pub text: String,
    #[serde(default)]
    pub readable: Option<crate::history::ReadablePage>,
    #[serde(default)]
    pub projection_preparing: bool,
    pub offset: u64,
    pub next: Option<u64>,
}
/// Finite per-commit evidence. Sequence gaps identify overwritten observations;
/// polling the last completion alone cannot establish an interval distribution.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DurableCommit {
    pub sequence: u64,
    pub prefix: u64,
    pub completed_micros: u64,
    pub sync_commit_micros: u64,
    pub oldest_accepted_micros: u64,
}
#[derive(Clone, Debug, Default)]
pub struct Progress {
    pub capacity: crate::cleanup::Capacity,
    pub commits: std::collections::VecDeque<DurableCommit>,
    pub accepted: u64,
    pub written: u64,
    pub operation: Option<(&'static str, Instant)>,
    pub producer_wait: Option<Instant>,
    pub pending_bytes: usize,
    pub pending_high_water: usize,
    pub backpressure_count: u64,
    pub backpressure_micros: u64,
    pub durable: u64,
    pub error: Option<String>,
    pub last_flush_micros: u64,
    pub flush_count: u64,
    pub completed_millis: u64,
    pub accepted_to_durable_millis: u64,
}
#[derive(Clone)]
pub struct Capture {
    session: String,
    sender: SyncSender<(String, Vec<u8>, Instant)>,
    progress: Arc<Mutex<Progress>>,
}
impl Capture {
    pub fn accept(&self, size: usize) {
        let mut p = self.progress.lock().unwrap();
        p.accepted += size as u64;
        p.pending_bytes += size;
        p.pending_high_water = p.pending_high_water.max(p.pending_bytes);
    }
    /// Return ownership to the sole PTY reader when Stop needs terminal drain.
    /// Retrying a retained block transfers ownership without accepting it twice.
    pub fn send(
        &self,
        mut block: (String, Vec<u8>, Instant),
        stop: Option<&AtomicBool>,
    ) -> std::result::Result<Option<(String, Vec<u8>, Instant)>, ()> {
        if block.0 != self.session || block.1.len() > 4096 {
            return Err(());
        }
        let since = Instant::now();
        let mut waiting = false;
        loop {
            match self.sender.try_send(block) {
                Ok(()) => break,
                Err(mpsc::TrySendError::Disconnected(_)) => return Err(()),
                Err(mpsc::TrySendError::Full(retained)) => {
                    block = retained;
                    if !waiting {
                        let mut p = self.progress.lock().unwrap();
                        p.backpressure_count += 1;
                        p.producer_wait = Some(since);
                        waiting = true;
                    }
                    if stop.is_some_and(|stop| stop.load(Ordering::Acquire)) {
                        let mut p = self.progress.lock().unwrap();
                        p.backpressure_micros += since.elapsed().as_micros() as u64;
                        p.producer_wait = None;
                        return Ok(Some(block));
                    }
                    thread::sleep(Duration::from_millis(5));
                }
            }
        }
        if waiting {
            let mut p = self.progress.lock().unwrap();
            p.backpressure_micros += since.elapsed().as_micros() as u64;
            p.producer_wait = None;
        }
        Ok(None)
    }
}
pub fn metadata(state: &Observation) -> Box<Observation> {
    let mut saved = state.clone();
    saved.screen = String::new();
    saved.formatted = String::new();
    saved.decoded = None;
    saved.history = None;
    saved.browse = None;
    saved.discovery = None;
    saved.creation = Default::default();
    saved.creation_protocol = 0;
    saved.registry = Default::default();
    saved.sessions.clear();
    saved.control_target.clear();
    saved.registry_pending = false;
    saved.adoption_pending = false;
    saved.cleanup = None;
    saved.cleanup_message = None;
    saved.durable_commits.clear();
    saved.status = crate::native_status::Status::default();
    Box::new(saved)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Purpose {
    Metadata,
    Prepare,
    Authorize,
    LaunchResult,
    FinalFlush,
    Seal,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Operation {
    pub id: uuid::Uuid,
    pub session: Option<String>,
    pub request: String,
    pub purpose: Purpose,
}
impl Operation {
    pub fn new(state: &Observation, purpose: Purpose) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            session: state
                .session_identity
                .as_ref()
                .map(|identity| identity.session.clone()),
            request: if matches!(purpose, Purpose::FinalFlush | Purpose::Seal) {
                state.session_identity.as_ref().map_or_else(
                    || state.launch_request.clone(),
                    |identity| identity.request.clone(),
                )
            } else {
                state.launch_request.clone()
            },
            purpose,
        }
    }
    pub fn matches(&self, state: &Observation) -> bool {
        self.session
            == state
                .session_identity
                .as_ref()
                .map(|identity| identity.session.clone())
            && if self.purpose == Purpose::LaunchResult {
                state
                    .launch_result
                    .as_ref()
                    .is_some_and(|result| result.request == self.request)
            } else if matches!(self.purpose, Purpose::FinalFlush | Purpose::Seal) {
                self.request
                    == state
                        .session_identity
                        .as_ref()
                        .map_or(state.launch_request.as_str(), |identity| {
                            identity.request.as_str()
                        })
            } else {
                self.request == state.launch_request
            }
    }
}
/// Only accepted commands create obligations. Rejected, duplicate or altered
/// completions cannot retire another command, even for the same launch request.
#[derive(Default)]
pub struct Obligations {
    pending: std::collections::HashMap<uuid::Uuid, Operation>,
}
impl Obligations {
    pub fn send(
        &mut self,
        sender: &SyncSender<Command>,
        mut command: Command,
    ) -> io::Result<Operation> {
        let operation = match &mut command {
            Command::Prepare(snapshot) => {
                snapshot.operation.purpose = Purpose::Prepare;
                snapshot.operation.clone()
            }
            Command::Authorize(snapshot) => {
                snapshot.operation.purpose = Purpose::Authorize;
                snapshot.operation.clone()
            }
            Command::Save(snapshot) => snapshot.operation.clone(),
            Command::Flush(operation) | Command::Seal(operation) => operation.clone(),
            _ => unreachable!("only durable session commands create obligations"),
        };
        sender
            .try_send(command)
            .map_err(|error| io::Error::other(error.to_string()))?;
        self.pending.insert(operation.id, operation.clone());
        Ok(operation)
    }
    pub fn complete(&mut self, operation: &Operation) -> bool {
        if self.pending.get(&operation.id) != Some(operation) {
            return false;
        }
        self.pending.remove(&operation.id);
        true
    }
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
    pub fn metadata_pending(&self) -> bool {
        self.pending
            .values()
            .any(|operation| !matches!(operation.purpose, Purpose::FinalFlush | Purpose::Seal))
    }
}
pub struct Snapshot {
    pub workspace_name: Option<String>,
    pub operation: Operation,
    pub state: Box<Observation>,
    pub checkout: Option<Vec<u8>>,
    pub task: String,
}
impl Snapshot {
    pub fn new(state: &Observation, checkout: Option<&Path>, task: &str) -> Self {
        Self {
            workspace_name: None,
            operation: Operation::new(state, Purpose::Metadata),
            state: metadata(state),
            checkout: checkout.map(crate::session::path_bytes),
            task: task.to_owned(),
        }
    }
    pub fn result(state: &Observation, checkout: Option<&Path>, task: &str) -> Self {
        let mut snapshot = Self::new(state, checkout, task);
        snapshot.operation.purpose = Purpose::LaunchResult;
        if let Some(result) = &state.launch_result {
            snapshot.operation.request.clone_from(&result.request);
        }
        snapshot
    }
}
#[derive(Clone)]
pub struct ResumeIdentity {
    pub agent: crate::events::NativeAgent,
    pub native: String,
}
pub enum Command {
    ArchiveLoad,
    ArchiveSave(uuid::Uuid, Box<crate::archive::Record>),
    SetupLoad,
    SetupSave(uuid::Uuid, Box<crate::setup::Record>),
    ConfigureSetup(uuid::Uuid, String, Option<String>),
    ObservedBranches(Vec<(Vec<u8>, String)>),
    Browse(search::Query),
    CreationLoad,
    CreationSave(uuid::Uuid, Box<crate::creation::Record>),
    SelectWorkspace(uuid::Uuid, String),
    Capture(CaptureStream),
    Rename(uuid::Uuid, String, String),
    ComparisonBase(uuid::Uuid, String, crate::working_diff::Base),
    Register(uuid::Uuid, crate::git::CheckoutIdentity),
    CancelResume {
        target: String,
        request: String,
    },
    ResumeIdentity {
        target: String,
        request: String,
        workspace: String,
        session: String,
        task: String,
    },
    Prepare(Snapshot),
    Authorize(Snapshot),
    Save(Snapshot),
    // Legacy storage test seam; runtime replies use HistoryTarget.
    #[allow(dead_code)]
    History(usize, u64),
    HistoryTarget {
        target: String,
        request: uuid::Uuid,
        index: usize,
        session: Option<String>,
        offset: u64,
    },
    #[allow(dead_code)]
    HistorySession(String, u64),
    Cleanup(usize),
    Delete(String),
    Seal(Operation),
    Retry,
    Flush(Operation),
}
#[derive(Clone)]
pub enum Completion {
    Archives(std::result::Result<Vec<crate::archive::Record>, String>),
    ArchiveSaved(
        uuid::Uuid,
        std::result::Result<crate::registry::Registry, String>,
    ),
    ResumeIdentity(String, String, std::result::Result<ResumeIdentity, String>),
    ResumeCancelled(String, String, std::result::Result<(), String>),
    Setups(std::result::Result<Vec<crate::setup::Record>, String>),
    SetupSaved(uuid::Uuid, std::result::Result<(), String>),
    Browse(search::Response),
    Creations(std::result::Result<Vec<crate::creation::Record>, String>),
    CreationSaved(
        uuid::Uuid,
        std::result::Result<crate::registry::Registry, String>,
    ),
    Selected(
        uuid::Uuid,
        std::result::Result<crate::registry::Registry, String>,
    ),
    Registered(
        uuid::Uuid,
        std::result::Result<crate::registry::Registry, String>,
    ),
    Registry(crate::registry::Registry),
    Retried,
    Sealed,
    Cleanup(crate::cleanup::Page),
    Deleted(String),
    HistoryHidden(String),
    Committed(Operation),
    Failed(Operation, String),
    History(HistoryPage),
    HistoryTarget(String, uuid::Uuid, std::result::Result<HistoryPage, String>),
    Error(&'static str, String),
}
struct SessionRestore {
    sessions: Vec<RestoredSession>,
    creation_launches: Vec<crate::creation::LaunchReceipt>,
}
pub struct RestoredSession {
    pub state: Observation,
    pub checkout: Option<PathBuf>,
    pub task: String,
}
pub struct Restored {
    pub sessions: Vec<RestoredSession>,
    pub state: Observation,
    pub checkout: Option<Vec<u8>>,
    pub task: String,
    pub launches: Vec<crate::session::LaunchResult>,
}
// Admission bounds live streams, not retained history rows. Each stream owns
// 64 queued <=4096-byte blocks, one worker block and one blocked producer block:
// <=270336 accepted bytes before the existing reader's separate finite Stop tail.
// Successful seal releases its queue, handle and commit ring; historic rows allocate none.
const MAX_CAPTURE_STREAMS: usize = 32;
pub struct CaptureStream {
    session: String,
    receiver: Receiver<(String, Vec<u8>, Instant)>,
    progress: Arc<Mutex<Progress>>,
    pending: Option<(String, Vec<u8>, Instant)>,
    retry_at: Option<Instant>,
    write_error: Option<String>,
    sync_error: Option<String>,
}
pub struct Worker {
    pub commands: SyncSender<Command>,
    pub events: Receiver<Completion>,
    pub progress: Arc<Mutex<Progress>>,
    captures: Arc<Mutex<std::collections::HashMap<String, Capture>>>,
}
impl Worker {
    pub fn capture_for(&self, session: &str) -> io::Result<Capture> {
        let mut captures = self.captures.lock().unwrap();
        if let Some(capture) = captures.get(session) {
            return Ok(capture.clone());
        }
        if captures.len() >= MAX_CAPTURE_STREAMS {
            return Err(io::Error::other(
                "32 live capture streams already reserved; stop and settle history before starting another",
            ));
        }
        let (sender, receiver) = mpsc::sync_channel(64);
        let progress = Arc::new(Mutex::new(Progress::default()));
        let capture = Capture {
            session: session.into(),
            sender,
            progress: progress.clone(),
        };
        self.commands
            .try_send(Command::Capture(CaptureStream {
                session: session.into(),
                receiver,
                progress,
                pending: None,
                retry_at: None,
                write_error: None,
                sync_error: None,
            }))
            .map_err(|e| io::Error::other(e.to_string()))?;
        captures.insert(session.into(), capture.clone());
        Ok(capture)
    }
    pub fn progress_for(&self, session: Option<&str>) -> Progress {
        let global = self.progress.lock().unwrap().clone();
        let mut progress = session
            .and_then(|id| {
                self.captures
                    .lock()
                    .unwrap()
                    .get(id)
                    .map(|capture| capture.progress.lock().unwrap().clone())
            })
            .unwrap_or_default();
        progress.capacity = global.capacity;
        progress.operation = global.operation;
        progress
    }

    pub fn start(directory: &Path) -> Result<(Self, Restored)> {
        let directory = directory.to_owned();
        let (ready_tx, ready) = mpsc::sync_channel(1);
        let (commands, control) = mpsc::sync_channel(16);
        let captures = Arc::new(Mutex::new(
            std::collections::HashMap::<String, Capture>::new(),
        ));
        let endpoints = captures.clone();
        let (tx, events) = mpsc::sync_channel(16);
        let progress = Arc::new(Mutex::new(Progress::default()));
        let status = progress.clone();
        thread::Builder::new().name("history-storage".into()).spawn(move || {
            let mut store = match Store::open(directory) {
                Ok(store) => store,
                Err(error) => { let _ = ready_tx.send(Err(error.to_string())); return; }
            };
            let restored = store.restore().map_err(|e| e.to_string());
            if ready_tx.send(restored).is_err() { return; }
            let mut streams: Vec<CaptureStream> = Vec::new();
            let mut finals: Vec<(Operation, u64)> = Vec::new();
            let mut cursor = 0usize;
            #[cfg(debug_assertions)]
            let mut last_operation: Option<Operation> = None;
            let mut since = Instant::now();
            let mut capacity_since = Instant::now() - Duration::from_secs(1);
            loop {
                // Reserve control capacity independently from capture traffic.
                let ready_final = finals.iter().position(|(operation, through)| {
                    streams.iter().find(|stream| Some(&stream.session) == operation.session.as_ref())
                        .is_none_or(|stream| { let p = stream.progress.lock().unwrap(); p.written >= *through || p.error.is_some() })
                }).map(|index| finals.remove(index).0);
                let command = ready_final.map(Command::Flush).or_else(|| control.try_recv().ok());
                if let Some(command) = command {
                    if let Command::Capture(stream) = command { streams.push(stream); continue; }
                    if let Command::Flush(operation) = &command
                        && let Some(stream) = streams.iter().find(|stream| Some(&stream.session) == operation.session.as_ref()) {
                        let p = stream.progress.lock().unwrap();
                        if p.written < p.accepted && p.error.is_none() {
                            if finals.iter().any(|(pending, _)| pending.session == operation.session) {
                                let _ = tx.send(Completion::Failed(operation.clone(), "Final capture operation already pending for session".into()));
                            } else {
                                finals.push((operation.clone(), p.accepted));
                            }
                            continue;
                        }
                    }
                    let kind = match &command { Command::CancelResume { .. } | Command::ResumeIdentity { .. } => "resume", Command::ArchiveLoad | Command::ArchiveSave(..) => "registry", Command::Browse(..) => "browse", Command::ObservedBranches(..) => "registry", Command::SetupLoad | Command::SetupSave(..) | Command::ConfigureSetup(..) | Command::ComparisonBase(..) | Command::CreationLoad | Command::CreationSave(..) => "registry", Command::SelectWorkspace(..) => "selection", Command::Capture(..) => unreachable!(), Command::Rename(..) | Command::Register(..) => "registry", Command::Save(..) | Command::Prepare(..) | Command::Authorize(..) => "metadata", Command::History(..) | Command::HistorySession(..) | Command::HistoryTarget { .. } => "history", Command::Cleanup(..) | Command::Delete(..) | Command::Seal(..) => "cleanup", Command::Retry => "retry", Command::Flush(..) => "flush" };
                    status.lock().unwrap().operation = Some((kind, Instant::now()));
                    let operation = match &command { Command::Prepare(snapshot) | Command::Authorize(snapshot) | Command::Save(snapshot) => Some(snapshot.operation.clone()), Command::Flush(operation) | Command::Seal(operation) => Some(operation.clone()), _ => None };
                    let result = match command {
                        Command::SetupLoad => Ok(Completion::Setups(store.setups().map_err(|e|e.to_string()))),
                        Command::ArchiveLoad => Ok(Completion::Archives(store.archives().map_err(|e|e.to_string()))),
                        Command::ArchiveSave(id,record) => Ok(Completion::ArchiveSaved(id,store.save_archive(&record).map_err(|e|e.to_string()))),
                        Command::SetupSave(id, record) => Ok(Completion::SetupSaved(id, store.save_setup(&record).map_err(|e|e.to_string()))),
                        Command::ConfigureSetup(id, project, command) => Ok(Completion::Registered(id, store.configure_setup(&project, &command).map_err(|e|e.to_string()))),
                        Command::CancelResume { target, request } => {
                            let result=store.db.execute("INSERT INTO resume_selections(request,workspace,session,cancelled) VALUES(?1,'','',1) ON CONFLICT(request) DO UPDATE SET cancelled=1",[&request]).map(|_|()).map_err(|error|error.to_string());
                            Ok(Completion::ResumeCancelled(target,request,result))
                        }
                        Command::ResumeIdentity { target, request, workspace, session, task } => { let result = store.resume_identity(&request, &workspace, &session, &task).map_err(|e| e.to_string()); Ok(Completion::ResumeIdentity(target, request, result)) },
                        Command::Browse(query) => Ok(Completion::Browse(store.start_browse(query))),
                        Command::ObservedBranches(facts) => (|| {
                            if facts.len()>32 {return Err("Branch fact batch exceeds 32".into());}
                            let tx=store.db.transaction()?;
                            for (path,branch) in facts {tx.execute("INSERT INTO checkout_branch_facts VALUES(?1,?2) ON CONFLICT(path) DO UPDATE SET branch=excluded.branch",params![path,branch])?;}
                            tx.commit()?;Ok(Completion::Sealed)
                        })(),
                        Command::SelectWorkspace(request, task) => Ok(Completion::Selected(request, store.select_workspace(&task).map_err(|error| error.to_string()))),
                        Command::Capture(..) => unreachable!(),
                        Command::CreationLoad => Ok(Completion::Creations(store.creations().map_err(|e|e.to_string()))),
                        Command::CreationSave(id, record) => Ok(Completion::CreationSaved(id,store.save_creation(&record).map_err(|e|e.to_string()))),
                        Command::ComparisonBase(id, workspace, base) => Ok(Completion::Registered(id, store.comparison_base(&workspace, &base).map_err(|error| error.to_string()))),
                        Command::Rename(id, workspace, name) => Ok(Completion::Registered(id, store.rename(&workspace, &name).map_err(|error| error.to_string()))),
                        Command::Register(id, checkout) => Ok(Completion::Registered(id, store.register(checkout).map_err(|error| error.to_string()))),
                        Command::Cleanup(index) => store.start_cleanup(index).map(|_| Completion::Sealed),
                        Command::Delete(id) => store.start_delete(&id).map(|_| Completion::HistoryHidden(id)),
                        Command::Seal(operation) => operation.session.as_deref().ok_or_else(|| io::Error::other("Missing session for seal").into()).and_then(|id| {
                            if streams.iter().find(|stream| stream.session == id).is_some_and(|stream| stream.progress.lock().unwrap().pending_bytes != 0) { return Err("History still owns pending capture".into()); }
                            store.seal(id)?;
                            store.transcripts.remove(id);
                            streams.retain(|stream| stream.session != id);
                            endpoints.lock().unwrap().remove(id);
                            Ok(())
                        }).map(|_| Completion::Committed(operation)),
                        Command::Retry => { store.index_error=None; since = Instant::now() - FLUSH; Ok(Completion::Retried) },
                        Command::Prepare(Snapshot { operation, state, checkout, task, workspace_name }) => store.fault("prepare").and_then(|_| store.save_named(&state, checkout, task, workspace_name)).map(|_| Completion::Committed(operation)),
                        Command::Authorize(Snapshot { operation, state, checkout, task, workspace_name }) => store.fault("authorize").and_then(|_| store.save_named(&state, checkout, task, workspace_name)).map(|_| Completion::Committed(operation)),
                        Command::Save(Snapshot { operation, state, checkout, task, workspace_name }) => store.fault("save").and_then(|_| store.save_named(&state, checkout, task, workspace_name)).map(|_| Completion::Committed(operation)),
                        Command::HistoryTarget { target, request, index, session, offset } => {
                            let page = store.start_readable(target.clone(), request, index, session, offset);
                            Ok(Completion::HistoryTarget(target, request, page.map_err(|error| error.to_string())))
                        },
                        Command::History(index, offset) => store.history(index, offset).map(Completion::History),
                        Command::HistorySession(id, offset) => store.history_session(&id, offset).map(Completion::History),
                        Command::Flush(operation) => {
                            (|| {
                                if let Some(id) = operation.session.as_deref() {
                                    if streams.iter().find(|stream| stream.session == id).is_some_and(|stream| stream.progress.lock().unwrap().pending_bytes != 0) { return Err("Final capture still pending; repair storage and retry".into()); }
                                    store.flush_session(id)?;
                                    if let Some(stream) = streams.iter_mut().find(|stream| stream.session == id) {
                                        stream.sync_error = None;
                                        stream.progress.lock().unwrap().error.clone_from(&stream.write_error);
                                    }
                                }
                                Ok(Completion::Committed(operation))
                            })()
                        }

                    };
                    status.lock().unwrap().operation = None;
                    for stream in &streams { store.publish(&stream.session, &mut stream.progress.lock().unwrap()); }
                    let event = result.unwrap_or_else(|e: Box<dyn std::error::Error + Send + Sync>| match operation { Some(operation) => Completion::Failed(operation, e.to_string()), None => Completion::Error(kind, e.to_string()) });
                    #[cfg(debug_assertions)]
                    if let Completion::Committed(operation) = &event {
                        let marker = store.directory.join("proof-storage-completion");
                        let expected = match operation.purpose { Purpose::Prepare => "prepare", Purpose::Authorize => "authorize", Purpose::LaunchResult => "result", Purpose::Metadata => "metadata", Purpose::FinalFlush => "flush", Purpose::Seal => "seal" };
                        if std::fs::read_to_string(&marker).is_ok_and(|value| value.trim() == expected) {
                            // Exercise the real completion channel while holding the exact
                            // commit receipt. No second storage writer or product command.
                            for field in 0..4 {
                                let mut altered = operation.clone();
                                match field { 0 => altered.id = uuid::Uuid::new_v4(), 1 => altered.session = Some("another-session".into()), 2 => altered.request = "another-request".into(), _ => altered.purpose = if altered.purpose == Purpose::Metadata { Purpose::LaunchResult } else { Purpose::Metadata } }
                                let _ = tx.send(Completion::Committed(altered.clone()));
                                let _ = tx.send(Completion::Failed(altered, "stale completion".into()));
                            }
                            let _ = std::fs::write(marker.with_extension("ready"), expected);
                            while marker.exists() { thread::sleep(Duration::from_millis(10)); }
                        }
                    }
                    #[cfg(debug_assertions)]
                    if let Completion::Committed(operation) = &event {
                        if store.directory.join("proof-storage-replay").exists()
                            && let Some(previous) = last_operation.take() {
                            let _ = tx.send(Completion::Committed(previous.clone()));
                            let _ = tx.send(Completion::Failed(previous, "duplicate stale failure".into()));
                        }
                        last_operation = Some(operation.clone());
                    }
                    if matches!(event, Completion::Committed(_)) {
                        let selected: Option<String> = store.db.query_row("SELECT task FROM app WHERE id=1", [], |row| row.get(0)).optional().unwrap_or(None);
                        if let Ok(registry) = crate::registry::snapshot(&store.db, selected.as_deref())
                            && tx.send(Completion::Registry(registry)).is_err() { break; }
                    }
                    if tx.send(event).is_err() { break; }
                }
                if let Some(event) = store.readable_step()
                    && tx.send(event).is_err() { break; }
                if let Some(event)=store.browse_step() && tx.send(event).is_err() {break;}
                if capacity_since.elapsed() >= Duration::from_secs(1) {
                    status.lock().unwrap().capacity = crate::cleanup::Capacity::sample(&store.directory);
                    capacity_since = Instant::now();
                }
                if store.cleanup_job.is_some() {
                    status.lock().unwrap().operation = Some(("cleanup", Instant::now()));
                    match store.cleanup_step() {
                        Ok(Some(event)) => {
                            if matches!(event, Completion::Deleted(_)) { status.lock().unwrap().capacity = crate::cleanup::Capacity::sample(&store.directory); }
                            for stream in &streams { store.publish(&stream.session, &mut stream.progress.lock().unwrap()); }
                            if tx.send(event).is_err() { break; }
                        },
                        Ok(None) => {},
                        Err(error) => { store.cleanup_job = None; let _ = tx.send(Completion::Error("cleanup", format!("History cleanup incomplete: {error}; g reopen then explicitly retry selected history"))); }
                    }
                }
                status.lock().unwrap().operation = None;
                // One append attempt per stream per pass, rotating the first stream.
                // Failed streams retain their block and cannot monopolize healthy streams.
                let count = streams.len();
                let mut worked = false;
                for turn in 0..count {
                    let index = (cursor + turn) % count;
                    let stream = &mut streams[index];
                    if stream.pending.is_none() { stream.pending = stream.receiver.try_recv().ok(); }
                    if let Some((id, bytes, accepted)) = &mut stream.pending
                        && stream.retry_at.is_none_or(|until| Instant::now() >= until) {
                        worked = true;
                        status.lock().unwrap().operation = Some(("write", Instant::now()));
                        match store.append(id, bytes, *accepted) {
                            Ok(count) => {
                                let mut p = stream.progress.lock().unwrap();
                                p.pending_bytes -= count;
                                stream.write_error = None;
                                p.error.clone_from(&stream.sync_error);
                                store.publish(&stream.session, &mut p);
                                stream.retry_at = None;
                                bytes.drain(..count);
                                if bytes.is_empty() { stream.pending = None; }
                            }
                            Err(error) => {
                                stream.write_error = Some(format!("History capture blocked: {error}; output retained; repair storage then press y to retry or s to stop"));
                                stream.progress.lock().unwrap().error.clone_from(&stream.write_error);
                                stream.retry_at = Some(Instant::now() + Duration::from_millis(100));
                            },
                        }
                    }
                    store.publish(&stream.session, &mut stream.progress.lock().unwrap());
                }
                if count > 0 { cursor = (cursor + 1) % count; }
                status.lock().unwrap().operation = None;
                if since.elapsed() >= FLUSH {
                    status.lock().unwrap().operation = Some(("sync", Instant::now()));
                    for stream in &mut streams {
                        stream.sync_error = store.flush_session(&stream.session).err().map(|error| format!("History sync failed: {error}; quit requires successful flush"));
                        let mut p = stream.progress.lock().unwrap();
                        p.error = stream.write_error.clone().or_else(|| stream.sync_error.clone());
                        store.publish(&stream.session, &mut p);
                    }
                    status.lock().unwrap().operation = None;
                    since = Instant::now();
                }
                if let Err(error)=store.index_step() {store.index_error=Some(format!("History index: {error}; committed raw history remains available"));}
                if !worked { thread::sleep(Duration::from_millis(5)); }
                if Arc::strong_count(&endpoints) == 1 { break; }

            }
        })?;
        let restored = ready.recv()?.map_err(io::Error::other)?;
        Ok((
            Self {
                commands,
                captures,
                events,
                progress,
            },
            restored,
        ))
    }
}
enum CleanupJob {
    List(crate::cleanup::Page, std::fs::ReadDir),
    Delete(String, std::fs::ReadDir),
}
struct HistoryJob {
    target: String,
    request: uuid::Uuid,
    session: String,
    page: HistoryPage,
    position: u64,
    end: u64,
    projection: crate::history::Projection,
    readable: crate::history::ReadablePage,
}
struct Store {
    search_job: Option<search::Job>,
    index_cursor: i64,
    index_error: Option<String>,
    history_jobs: std::collections::VecDeque<HistoryJob>,
    history_checkpoints: std::collections::VecDeque<(String, u64, crate::history::Projection)>,
    cleanup_job: Option<CleanupJob>,
    directory_handle: File,
    directory: PathBuf,
    db: Connection,
    transcripts: std::collections::HashMap<String, Transcript>,
    origin: Instant,
}
#[derive(Default)]
struct Transcript {
    active: Option<(String, File, u64)>,
    total: u64,
    active_dirty: bool,
    oldest: Option<Instant>,
    sync_micros: u64,
    completions: u64,
    completed_millis: u64,
    accepted_to_durable_millis: u64,
    commits: std::collections::VecDeque<DurableCommit>,
}
impl Store {
    fn publish(&self, id: &str, progress: &mut Progress) {
        let Some(transcript) = self.transcripts.get(id) else {
            return;
        };
        progress.commits.clone_from(&transcript.commits);
        progress.written = transcript
            .active
            .as_ref()
            .map_or(transcript.total, |(_, _, offset)| *offset);
        progress.durable = transcript.total;
        progress.last_flush_micros = transcript.sync_micros;
        progress.flush_count = transcript.completions;
        progress.completed_millis = transcript.completed_millis;
        progress.accepted_to_durable_millis = transcript.accepted_to_durable_millis;
    }
    fn open(directory: PathBuf) -> Result<Self> {
        // Anchor cleanup to the validated directory inode, never a mutable saved path.
        let directory_handle = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
            .open(&directory)?;
        let path = directory.join("history.sqlite3");
        // Create privately before SQLite opens it; the parent is already validated.
        OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&path)?;
        let mut db = Connection::open(&path)?;
        db.execute_batch(
            "PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL; PRAGMA fullfsync=ON;",
        )?;
        let version: u32 = db.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version > 10 {
            return Err("History schema is newer than this executable".into());
        }
        if version == 0 {
            let tx = db.transaction()?;
            tx.execute_batch("CREATE TABLE IF NOT EXISTS app (id INTEGER PRIMARY KEY CHECK(id=1), state TEXT NOT NULL, checkout BLOB, task TEXT NOT NULL);
                CREATE TABLE IF NOT EXISTS sessions (id TEXT PRIMARY KEY, identity TEXT NOT NULL, state TEXT NOT NULL, durable INTEGER NOT NULL DEFAULT 0);
                PRAGMA user_version=1;")?;
            tx.commit()?;
            File::open(&directory)?.sync_all()?;
        }
        if version < 2 {
            let tx = db.transaction()?;
            tx.execute_batch("CREATE TABLE IF NOT EXISTS requests (id TEXT PRIMARY KEY, result TEXT NOT NULL); PRAGMA user_version=2;")?;
            tx.commit()?;
        }
        if version < 3 {
            let tx = db.transaction()?;
            tx.execute_batch(
                "ALTER TABLE sessions ADD COLUMN created INTEGER;
                ALTER TABLE sessions ADD COLUMN sealed INTEGER NOT NULL DEFAULT 0;
                ALTER TABLE sessions ADD COLUMN history_state TEXT NOT NULL DEFAULT 'retained';
                PRAGMA user_version=3;",
            )?;
            tx.commit()?;
        }
        if version < 4 {
            let tx = db.transaction()?;
            crate::registry::create(&tx)?;
            tx.execute_batch(
                "CREATE TABLE workspace_names(workspace TEXT PRIMARY KEY, source TEXT NOT NULL);",
            )?;
            let historical = tx
                .prepare("SELECT id,identity,state FROM sessions ORDER BY rowid")?
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            for (id, label, json) in historical {
                let mut state: Observation = serde_json::from_str(&json)?;
                if state.session_identity.is_none() && state.identity.is_none() {
                    state.identity = Some(label);
                }
                let state = decode_record(&serde_json::to_string(&state)?)?;
                let identity = state.session_identity.as_ref().ok_or(
                    "Legacy session has no attributable task identity; registry migration refused",
                )?;
                if identity.session != id {
                    return Err("Legacy session UUID conflicts with its durable row; registry migration refused".into());
                }
                crate::registry::record(&tx, &state, None, &identity.task)?;
            }
            let current: Option<(String, Option<Vec<u8>>, String)> = tx
                .query_row(
                    "SELECT state,checkout,task FROM app WHERE id=1",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .optional()?;
            if let Some((json, path, task)) = current {
                crate::registry::record(&tx, &decode_record(&json)?, path.as_deref(), &task)?;
            }
            tx.pragma_update(None, "user_version", 4)?;
            tx.commit()?;
        }
        if version < 5 {
            let tx = db.transaction()?;
            tx.execute_batch("CREATE TABLE IF NOT EXISTS workspace_names(workspace TEXT PRIMARY KEY, source TEXT NOT NULL); INSERT INTO workspace_names SELECT id,'retained' FROM workspaces WHERE true ON CONFLICT(workspace) DO UPDATE SET source='retained'; PRAGMA user_version=5;")?;
            tx.commit()?;
        }
        if version < 6 {
            let tx = db.transaction()?;
            tx.execute_batch("CREATE TABLE workspace_creations(id TEXT PRIMARY KEY, record TEXT NOT NULL); PRAGMA user_version=6;")?;
            tx.commit()?;
        }
        db.execute_batch("CREATE TABLE IF NOT EXISTS workspace_comparison_bases(workspace TEXT PRIMARY KEY, reference BLOB NOT NULL, base_commit TEXT NOT NULL);")?;
        if version < 7 {
            let tx = db.transaction()?;
            search::schema(&tx)?;
            tx.pragma_update(None, "user_version", 7)?;
            tx.commit()?;
        }
        if version < 8 {
            let tx = db.transaction()?;
            tx.execute_batch("CREATE TABLE IF NOT EXISTS project_setup(project TEXT PRIMARY KEY, command TEXT NOT NULL); CREATE TABLE IF NOT EXISTS workspace_setup(attempt TEXT PRIMARY KEY, record TEXT NOT NULL); CREATE TABLE IF NOT EXISTS workspace_setup_policies(request TEXT PRIMARY KEY, command TEXT); PRAGMA user_version=8;")?;
            tx.commit()?;
        }
        if version < 9 {
            let tx = db.transaction()?;
            tx.execute_batch("CREATE TABLE resume_selections(request TEXT PRIMARY KEY, workspace TEXT NOT NULL, session TEXT NOT NULL, cancelled INTEGER NOT NULL DEFAULT 0); PRAGMA user_version=9;")?;
            tx.commit()?;
        }
        db.execute_batch("CREATE TABLE IF NOT EXISTS workspace_setup_policies(request TEXT PRIMARY KEY, command TEXT);")?;
        if version < 10 {
            let tx = db.transaction()?;
            tx.execute_batch("CREATE TABLE IF NOT EXISTS workspace_archives(request TEXT PRIMARY KEY,workspace TEXT NOT NULL,record TEXT NOT NULL);")?;
            tx.pragma_update(None, "user_version", 10)?;
            tx.commit()?;
        }
        let mut store = Self {
            search_job: None,
            index_cursor: 0,
            index_error: None,
            history_jobs: Default::default(),
            history_checkpoints: Default::default(),
            cleanup_job: None,
            directory_handle,
            directory,
            db,
            transcripts: Default::default(),
            origin: Instant::now(),
        };
        // Only a previously committed explicit intent authorizes restart work.
        store.resume_delete()?;
        Ok(store)
    }
    fn seal(&mut self, id: &str) -> Result<()> {
        let json: String =
            self.db
                .query_row("SELECT state FROM sessions WHERE id=?1", [id], |r| r.get(0))?;
        let state: Observation = decode_record(&json)?;
        if state.execution.is_some()
            || !matches!(
                state.lifecycle,
                Lifecycle::Stopped | Lifecycle::Exited | Lifecycle::Failed
            )
        {
            return Err("History still has active or uncertain execution".into());
        }
        if let Some(transcript) = self.transcripts.get_mut(id) {
            if transcript.oldest.is_some() {
                return Err("History final flush still pending".into());
            }
            transcript.active = None;
        }
        self.db
            .execute("UPDATE sessions SET sealed=1 WHERE id=?1", [id])?;
        Ok(())
    }
    fn start_cleanup(&mut self, index: usize) -> Result<()> {
        if self.cleanup_job.is_some() {
            return Err("Cleanup busy; retry shortly".into());
        }
        let mut statement = self.db.prepare("SELECT id,identity,state,created,sealed,history_state FROM sessions WHERE history_state!='deleted' ORDER BY sealed DESC, created IS NULL, created ASC, rowid ASC LIMIT ?1 OFFSET ?2")?;
        let mut rows = statement.query(params![
            crate::cleanup::LIST_SIZE + 1,
            index.saturating_mul(crate::cleanup::LIST_SIZE)
        ])?;
        let mut page = crate::cleanup::Page {
            index,
            ..Default::default()
        };
        while let Some(row) = rows.next()? {
            if page.entries.len() == crate::cleanup::LIST_SIZE {
                page.next = true;
                break;
            }
            let state: Observation = decode_record(&row.get::<_, String>(2)?)?;
            let sealed: bool = row.get(4)?;
            let history_state: String = row.get(5)?;
            let eligible = sealed
                && state.execution.is_none()
                && matches!(
                    state.lifecycle,
                    Lifecycle::Stopped | Lifecycle::Exited | Lifecycle::Failed
                );
            page.entries.push(crate::cleanup::Entry {
                id: row.get(0)?,
                identity: row.get(1)?,
                checkout: state.adopted.unwrap_or_default(),
                created: row.get(3)?,
                bytes: Some(0),
                eligible,
                status: if history_state == "deleting" {
                    "incomplete deletion; explicitly retry".into()
                } else if eligible {
                    "stopped; final history committed".into()
                } else {
                    "unavailable: capture/execution not sealed".into()
                },
            });
        }
        self.cleanup_job = Some(CleanupJob::List(page, std::fs::read_dir(&self.directory)?));
        Ok(())
    }
    fn start_delete(&mut self, id: &str) -> Result<()> {
        if uuid::Uuid::parse_str(id)?.to_string() != id {
            return Err("Invalid managed session UUID".into());
        }
        if self.cleanup_job.is_some() {
            return Err("Cleanup busy; retry shortly".into());
        }
        let (json, sealed, history): (String, bool, String) = self.db.query_row(
            "SELECT state,sealed,history_state FROM sessions WHERE id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        let state: Observation = decode_record(&json)?;
        if history == "deleted" {
            return Err("History already deleted".into());
        }
        if !sealed
            || state.execution.is_some()
            || !matches!(
                state.lifecycle,
                Lifecycle::Stopped | Lifecycle::Exited | Lifecycle::Failed
            )
        {
            return Err("History deletion refused: active, uncertain or unflushed session".into());
        }
        // The accepted EOF/final-flush seal closed any retained file handle.
        if self
            .transcripts
            .get(id)
            .is_some_and(|entry| entry.active.is_some() || entry.oldest.is_some())
        {
            return Err("History capture still owns selected transcript".into());
        }
        self.fault("delete-intent")?;
        let tx = self.db.transaction()?;
        tx.execute(
            "UPDATE sessions SET history_state='deleting' WHERE id=?1",
            [id],
        )?;
        tx.commit()?;
        self.search_job = None;
        self.history_checkpoints
            .retain(|(session, _, _)| session != id);
        self.cleanup_job = Some(CleanupJob::Delete(
            id.into(),
            std::fs::read_dir(&self.directory)?,
        ));
        Ok(())
    }
    fn resume_delete(&mut self) -> Result<()> {
        let id: Option<String> = self
            .db
            .query_row(
                "SELECT id FROM sessions WHERE history_state='deleting' ORDER BY id LIMIT 1",
                [],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(id) = id {
            self.start_delete(&id)?;
        }
        Ok(())
    }
    fn matching_chunk(name: &std::ffi::OsStr, id: &str) -> bool {
        let Some(name) = name.to_str() else {
            return false;
        };
        let Some(ordinal) = name
            .strip_prefix(id)
            .and_then(|name| name.strip_prefix('-'))
            .and_then(|name| name.strip_suffix(".transcript"))
        else {
            return false;
        };
        // Canonical format only; never use a saved arbitrary path or checkout.
        ordinal.len() >= 8
            && ordinal.bytes().all(|c| c.is_ascii_digit())
            && ordinal
                .parse::<u64>()
                .is_ok_and(|number| format!("{number:08}") == ordinal)
    }
    fn chunk_metadata(&self, name: &std::ffi::OsStr) -> Result<(u64, bool)> {
        let name = std::ffi::CString::new(name.as_bytes())?;
        let mut meta = std::mem::MaybeUninit::<libc::stat>::uninit();
        // SAFETY: anchored directory fd, single relative name, initialized output on success.
        if unsafe {
            libc::fstatat(
                self.directory_handle.as_raw_fd(),
                name.as_ptr(),
                meta.as_mut_ptr(),
                libc::AT_SYMLINK_NOFOLLOW,
            )
        } != 0
        {
            return Err(io::Error::last_os_error().into());
        }
        // SAFETY: fstatat succeeded.
        let meta = unsafe { meta.assume_init() };
        Ok((
            (meta.st_blocks.max(0) as u64).saturating_mul(512),
            meta.st_mode & libc::S_IFMT == libc::S_IFREG && meta.st_nlink == 1,
        ))
    }
    fn unlink_chunk(&self, name: &std::ffi::OsStr) -> Result<()> {
        let name = std::ffi::CString::new(name.as_bytes())?;
        // SAFETY: name is one validated relative filename, anchored to our private directory.
        if unsafe { libc::unlinkat(self.directory_handle.as_raw_fd(), name.as_ptr(), 0) } != 0 {
            return Err(io::Error::last_os_error().into());
        }
        Ok(())
    }
    fn cleanup_step(&mut self) -> Result<Option<Completion>> {
        let Some(mut job) = self.cleanup_job.take() else {
            return Ok(None);
        };
        for _ in 0..64 {
            let next = match &mut job {
                CleanupJob::List(_, scan) | CleanupJob::Delete(_, scan) => scan.next(),
            };
            let Some(entry) = next else {
                return match job {
                    CleanupJob::List(page, _) => Ok(Some(Completion::Cleanup(page))),
                    CleanupJob::Delete(id, _) => {
                        self.fault("delete-directory-sync")?;
                        self.directory_handle.sync_all()?;
                        self.fault("delete-complete")?;
                        let tx = self.db.transaction()?;
                        tx.execute(
                            "UPDATE sessions SET history_state='deleted',durable=0 WHERE id=?1",
                            [&id],
                        )?;
                        tx.commit()?;
                        let current: String =
                            self.db
                                .query_row("SELECT state FROM app WHERE id=1", [], |r| r.get(0))?;
                        let current: Observation = decode_record(&current)?;
                        if current
                            .session_identity
                            .as_ref()
                            .map(|identity| identity.session.as_str())
                            == Some(&id)
                        {
                            self.transcripts.remove(&id);
                        }
                        self.resume_delete()?;
                        Ok(Some(Completion::Deleted(id)))
                    }
                };
            };
            let entry = entry?;
            let name = entry.file_name();
            match &mut job {
                CleanupJob::List(page, _) => {
                    for row in &mut page.entries {
                        if Self::matching_chunk(&name, &row.id) {
                            match self.chunk_metadata(&name) {
                                Ok((allocated, true)) => {
                                    row.bytes =
                                        row.bytes.map(|bytes| bytes.saturating_add(allocated))
                                }
                                _ => {
                                    row.bytes = None;
                                    row.eligible = false;
                                    row.status = "unavailable: unsafe or unreadable chunk".into();
                                }
                            }
                        }
                    }
                }
                CleanupJob::Delete(id, _) if Self::matching_chunk(&name, id) => {
                    let (_, safe) = self.chunk_metadata(&name)?;
                    if !safe {
                        return Err("Unsafe transcript object; deletion remains incomplete".into());
                    }
                    self.fault("delete-unlink")?;
                    self.unlink_chunk(&name)?;
                    self.fault("delete-after-unlink")?;
                }
                _ => {}
            }
        }
        self.cleanup_job = Some(job);
        Ok(None)
    }
    fn restore(&mut self) -> Result<Restored> {
        let mut launches = self
            .db
            .prepare("SELECT result FROM requests LIMIT 1025")?
            .query_map([], |r| r.get::<_, String>(0))?
            .map(|row| Ok(serde_json::from_str(&row?)?))
            .collect::<Result<Vec<_>>>()?;
        if launches.len() > 1024 {
            return Err("Retained launch outcomes exceed recovery capacity; cannot safely truncate request identities".into());
        }
        let SessionRestore {
            sessions,
            creation_launches,
        } = self.restore_sessions(&mut launches)?;
        let row: Option<(String, Option<Vec<u8>>, String)> = self
            .db
            .query_row("SELECT state,checkout,task FROM app WHERE id=1", [], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .optional()?;
        let Some((json, path, task)) = row else {
            return Ok(Restored {
                sessions,
                state: Observation {
                    creation: crate::creation::View {
                        launches: creation_launches,
                        ..Default::default()
                    },
                    registry: crate::registry::snapshot(&self.db, None)?,
                    ..Observation::default()
                },
                checkout: None,
                task: uuid::Uuid::new_v4().to_string(),
                launches,
            });
        };
        let mut state: Observation = decode_record(&json)?;
        state.registry = crate::registry::snapshot(&self.db, Some(&task))?;
        state.creation.launches = creation_launches;
        state = self.recover_state(state, &mut launches)?;
        Ok(Restored {
            sessions,
            state,
            checkout: path,
            task,
            launches,
        })
    }
    fn recover_state(
        &mut self,
        mut state: Observation,
        launches: &mut Vec<crate::session::LaunchResult>,
    ) -> Result<Observation> {
        state.stopped = false;
        state.status = crate::native_status::Status::default();
        state.metadata_pending = false;
        if let Some(identity) = &state.session_identity {
            // Recovered bytes are the committed prefix. Prior process-local queues
            // and producer ownership cannot be restored or treated as pending work.
            let durable: u64 = self.db.query_row(
                "SELECT durable FROM sessions WHERE id=?1",
                [&identity.session],
                |row| row.get(0),
            )?;
            state.capture_loss_possible |= state.accepted_bytes > durable;
            state.accepted_bytes = durable;
            state.written_bytes = durable;
            state.durable_bytes = durable;
            state.capture_pending_bytes = 0;
            state.producer_wait_millis = 0;
            state.storage_wait_millis = 0;
            state.storage_error = None;
        }

        if state.execution.is_some()
            || matches!(
                state.lifecycle,
                Lifecycle::Running | Lifecycle::Stopping | Lifecycle::StopFailed
            )
            || (state.lifecycle == Lifecycle::Interrupted && !state.reconciled)
        {
            if state.execution.is_none() {
                state.execution = Some(crate::recovery::Execution::legacy()?);
            }
            if let Some(evidence) = state.execution.as_mut() {
                evidence.anchor_legacy()?;
            }
            state.lifecycle = Lifecycle::Interrupted;
            state.error = Some("Interrupted supervisor: retained history available; execution is uncertain. No automatic restart. u reconciles; absent native leader may require verified reboot.".into());
        }
        if state.execution.is_some() {
            if let Some(known) = launches
                .iter()
                .find(|result| result.request == state.launch_request)
            {
                state.launch_result = Some(known.clone());
            } else {
                if launches.len() >= 1024 {
                    return Err(
                        "Interrupted launch outcome cannot fit within retained request capacity"
                            .into(),
                    );
                }
                let result = crate::session::LaunchResult { request: state.launch_request.clone(), identity: state.identity.clone(), error: Some("Interrupted launch: native execution outcome uncertain; reconcile explicitly with u. No automatic restart.".into()) };
                self.db.execute(
                    "INSERT INTO requests VALUES(?1,?2) ON CONFLICT(id) DO NOTHING",
                    params![result.request, serde_json::to_string(&result)?],
                )?;
                launches.push(result.clone());
                state.launch_result = Some(result);
            }
            if let Some(identity) = &state.session_identity {
                // Anchor uncertainty without changing the selected workspace.
                self.db.execute(
                    "UPDATE sessions SET state=?2 WHERE id=?1",
                    params![identity.session, serde_json::to_string(&state)?],
                )?;
            }
        }
        Ok(state)
    }
    fn restore_sessions(
        &mut self,
        launches: &mut Vec<crate::session::LaunchResult>,
    ) -> Result<SessionRestore> {
        let requests = self
            .db
            .prepare("SELECT id FROM workspace_creations LIMIT 257")?
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<std::collections::BTreeSet<_>, _>>()?;
        if requests.len() > 256 {
            return Err("Creation receipt recovery exceeds capacity".into());
        }
        let mut creation_launches = std::collections::BTreeMap::new();
        // Existing launch outcomes cap the history population at 1024. Never
        // silently truncate occupancy evidence if a newer/legacy DB exceeds it.
        let rows = self.db.prepare("SELECT s.id,s.state,w.path,w.task,s.identity FROM sessions s LEFT JOIN workspace_sessions m ON m.session=s.id LEFT JOIN workspaces w ON w.id=m.workspace ORDER BY s.rowid DESC LIMIT 1025")?
            .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, Option<Vec<u8>>>(2)?, row.get::<_, Option<String>>(3)?, row.get::<_, String>(4)?)))?
            .collect::<std::result::Result<Vec<_>,_>>()?;
        if rows.len() > 1024 {
            return Err("Retained session recovery exceeds 1024 records; cannot safely omit execution reservations".into());
        }
        let mut latest = std::collections::HashSet::new();
        let mut sessions = Vec::new();
        for (id, json, path, task, label) in rows {
            // Older rows may retain their identity only in the dedicated column.
            // Apply the same validated conversion as registry migration without
            // rewriting the original retained record merely to restore it.
            let mut state: Observation = serde_json::from_str(&json)?;
            if state.session_identity.is_none() && state.identity.is_none() {
                state.identity = Some(label);
            }
            let state = decode_record(&serde_json::to_string(&state)?)?;
            let identity = state
                .session_identity
                .as_ref()
                .ok_or("Retained session lacks attributable identity")?;
            if identity.session != id {
                return Err("Retained session identity does not match its durable row".into());
            }
            let task = task.unwrap_or_else(|| identity.task.clone());
            let is_latest = latest.insert(task.clone());
            let unresolved = state.execution.is_some()
                || matches!(
                    state.lifecycle,
                    Lifecycle::Running | Lifecycle::Stopping | Lifecycle::StopFailed
                )
                || state.lifecycle == Lifecycle::Interrupted && !state.reconciled;
            // Retain typed final presentation before omitting an old stopped
            // row from execution ownership. This creates no runtime/reservation.
            if requests.contains(&identity.request) {
                let receipt = crate::creation::LaunchReceipt {
                    request: identity.request.clone(),
                    session: Some(identity.session.clone()),
                    lifecycle: if unresolved {
                        Lifecycle::Interrupted
                    } else {
                        state.lifecycle
                    },
                    result: None,
                    pending: false,
                    error: None,
                };
                if creation_launches
                    .insert(identity.request.clone(), receipt)
                    .is_some()
                {
                    return Err(
                        "Creation launch request has conflicting retained session bindings".into(),
                    );
                }
            }
            if !is_latest && !unresolved {
                continue;
            }
            let checkout = path
                .map(|bytes| PathBuf::from(std::ffi::OsString::from_vec(bytes)))
                .or_else(|| {
                    state
                        .checkout_identity
                        .as_ref()
                        .map(|identity| identity.root.clone())
                });
            let state = self.recover_state(state, launches)?;
            sessions.push(RestoredSession {
                state,
                checkout,
                task,
            });
        }
        for (request, receipt) in &mut creation_launches {
            receipt.result = launches.iter().find(|r| &r.request == request).cloned();
            receipt.error = receipt.result.as_ref().and_then(|r| r.error.clone());
        }
        Ok(SessionRestore {
            sessions,
            creation_launches: creation_launches.into_values().collect(),
        })
    }
    fn select_workspace(&mut self, task: &str) -> Result<crate::registry::Registry> {
        let registry = crate::registry::snapshot(&self.db, Some(task))?;
        let workspace = registry
            .workspaces
            .iter()
            .find(|workspace| {
                workspace.task == task || workspace.legacy_tasks.iter().any(|old| old == task)
            })
            .ok_or("Unknown retained workspace selection")?;
        let saved: Option<String> = self.db.query_row("SELECT s.state FROM sessions s JOIN workspace_sessions m ON m.session=s.id WHERE m.workspace=?1 ORDER BY s.rowid DESC LIMIT 1", [&workspace.id], |row| row.get(0)).optional()?;
        let state = if let Some(json) = saved {
            decode_record(&json)?
        } else {
            Observation {
                checkout_identity: workspace.checkout.clone(),
                adopted: workspace.path.as_ref().map(|bytes| {
                    PathBuf::from(std::ffi::OsString::from_vec(bytes.clone()))
                        .display()
                        .to_string()
                }),
                ..Observation::default()
            }
        };
        self.db.execute("INSERT INTO app VALUES(1,?1,?2,?3) ON CONFLICT(id) DO UPDATE SET state=excluded.state,checkout=excluded.checkout,task=excluded.task", params![serde_json::to_string(&state)?, workspace.path, workspace.task])?;
        crate::registry::snapshot(&self.db, Some(&workspace.task))
    }
    // Isolated debug proof boundary: all file inspection and injected waits run on
    // this sole storage worker, never the UI or supervisor control loop.
    fn fault(&self, boundary: &str) -> Result<()> {
        #[cfg(debug_assertions)]
        {
            let path = self.directory.join(format!("proof-storage-{boundary}"));
            while let Ok(value) = std::fs::read_to_string(&path) {
                if value.trim() == "stall" {
                    thread::sleep(Duration::from_millis(10));
                    continue;
                }
                if value.trim() == "error" {
                    return Err(io::Error::from_raw_os_error(libc::ENOSPC).into());
                }
                if let Ok(millis) = value.trim().parse::<u64>() {
                    thread::sleep(Duration::from_millis(millis.min(60_000)));
                }
                break;
            }
        }
        let _ = boundary;
        Ok(())
    }
    fn session_fault(&self, id: &str, boundary: &str) -> Result<()> {
        #[cfg(debug_assertions)]
        if std::fs::read_to_string(self.directory.join("proof-storage-session"))
            .is_ok_and(|target| target.trim() != id)
        {
            return Ok(());
        }
        let _ = id;
        self.fault(boundary)
    }
    fn session_partial_fault(&self, id: &str) -> Option<usize> {
        #[cfg(debug_assertions)]
        if std::fs::read_to_string(self.directory.join("proof-storage-session"))
            .is_ok_and(|target| target.trim() != id)
        {
            return None;
        }
        let _ = id;
        self.partial_fault()
    }
    fn partial_fault(&self) -> Option<usize> {
        #[cfg(debug_assertions)]
        return std::fs::read_to_string(self.directory.join("proof-storage-partial"))
            .ok()
            .and_then(|value| value.trim().parse().ok());
        #[cfg(not(debug_assertions))]
        None
    }
    fn archives(&self) -> Result<Vec<crate::archive::Record>> {
        let rows = self
            .db
            .prepare("SELECT record FROM workspace_archives ORDER BY rowid")?
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(|r| Ok(serde_json::from_str(&r)?))
            .collect()
    }
    fn save_archive(
        &mut self,
        record: &crate::archive::Record,
    ) -> Result<crate::registry::Registry> {
        self.fault(match record.phase {
            crate::archive::Phase::Waiting => "archive-intent",
            crate::archive::Phase::Prepared => "archive-prepared",
            crate::archive::Phase::Archived => match record.branch_phase {
                crate::archive::BranchPhase::Cleaned | crate::archive::BranchPhase::Retained => {
                    "archive-branch-result"
                }
                crate::archive::BranchPhase::RestorePending => "archive-restore-intent",
                crate::archive::BranchPhase::Restored
                | crate::archive::BranchPhase::RestoreFailed => "archive-restore-result",
                _ => "archive-result",
            },
            _ => "archive-phase",
        })?;
        self.fault("registry")?;
        if serde_json::to_vec(record)?.len() > 32768 {
            return Err(
                "Archive metadata exceeds 32 KiB; files retained and workspace unarchived".into(),
            );
        }
        let tx = self.db.transaction()?;
        let previous: Option<String> = tx
            .query_row(
                "SELECT record FROM workspace_archives WHERE request=?1",
                [&record.request],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(previous) = previous {
            let old: crate::archive::Record = serde_json::from_str(&previous)?;
            if old.workspace != record.workspace
                || old.checkout != record.checkout
                || old.reference != record.reference
                || old.owned_branch != record.owned_branch
                || old.delete_target != record.delete_target
                || old.protected_base != record.protected_base
                || old.saved.is_some() && old.saved != record.saved
            {
                return Err("Archive identity changed".into());
            }
            if old.tree.is_some()
                && (old.tree != record.tree
                    || old.before != record.before
                    || old.branch != record.branch
                    || old.index_tree != record.index_tree
                    || old.index_hash != record.index_hash)
            {
                return Err(
                    "Archive prepared snapshot changed; use a new request for new work".into(),
                );
            }
        } else if tx.query_row("SELECT count(*) FROM workspace_archives", [], |r| {
            r.get::<_, u64>(0)
        })? >= 64
        {
            return Err("Archive receipt capacity reached; retained code preserved".into());
        }
        let known: Option<String> = tx.query_row(
            "SELECT checkout FROM workspaces WHERE id=?1",
            [&record.workspace],
            |r| r.get(0),
        )?;
        if known
            .as_deref()
            .map(serde_json::from_str::<crate::git::CheckoutIdentity>)
            .transpose()?
            .as_ref()
            != Some(&record.checkout)
        {
            return Err("Archive checkout conflicts with workspace registry".into());
        }
        tx.execute("INSERT INTO workspace_archives VALUES(?1,?2,?3) ON CONFLICT(request) DO UPDATE SET record=excluded.record",rusqlite::params![record.request,record.workspace,serde_json::to_string(record)?])?;
        if record.phase == crate::archive::Phase::Archived {
            if record.saved.is_none() {
                return Err("Missing protected archive commit".into());
            }
            let archived = i64::from(record.branch_phase != crate::archive::BranchPhase::Restored);
            tx.execute("INSERT INTO workspace_history_facts VALUES(?1,?2) ON CONFLICT(workspace) DO UPDATE SET archived=excluded.archived",rusqlite::params![&record.workspace,archived])?;
        }
        tx.commit()?;
        let selected: Option<String> = self
            .db
            .query_row("SELECT task FROM app WHERE id=1", [], |r| r.get(0))
            .optional()?;
        crate::registry::snapshot(&self.db, selected.as_deref())
    }
    fn creations(&self) -> Result<Vec<crate::creation::Record>> {
        let json = self
            .db
            .prepare("SELECT record FROM workspace_creations ORDER BY rowid")?
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        json.into_iter()
            .map(|j| Ok(serde_json::from_str(&j)?))
            .collect()
    }
    fn save_creation(
        &mut self,
        record: &crate::creation::Record,
    ) -> Result<crate::registry::Registry> {
        self.fault("registry")?;
        if record.phase == crate::creation::Phase::Created {
            self.fault("creation-result")?;
        }
        let tx = self.db.transaction()?;
        let previous: Option<String> = tx
            .query_row(
                "SELECT record FROM workspace_creations WHERE id=?1",
                [&record.intent.request],
                |row| row.get(0),
            )
            .optional()?;
        let new_intent = previous.is_none();
        if let Some(previous) = previous {
            let previous: crate::creation::Record = serde_json::from_str(&previous)?;
            if serde_json::to_string(&previous.intent)? != serde_json::to_string(&record.intent)?
                || previous.common_dir != record.common_dir
            {
                return Err("Creation request is already bound to another intent".into());
            }
        } else if tx.query_row("SELECT count(*) FROM workspace_creations", [], |r| {
            r.get::<_, u64>(0)
        })? >= 256
        {
            return Err("Creation receipt capacity reached; retained outcomes preserved".into());
        }
        if new_intent {
            tx.execute("INSERT INTO workspace_setup_policies VALUES(?1,(SELECT command FROM project_setup WHERE project=?2))",rusqlite::params![record.intent.request,record.intent.project])?;
        }
        if let Some(checkout) = &record.checkout {
            let path = crate::session::path_bytes(&checkout.root);
            let state = Observation {
                checkout_identity: Some(checkout.clone()),
                ..Observation::default()
            };
            crate::registry::record(&tx, &state, Some(&path), &record.intent.request)?;
            if let (Some(reference), Some(commit)) = (&record.base_ref, &record.commit) {
                tx.execute("INSERT OR IGNORE INTO workspace_comparison_bases SELECT workspace,?1,?2 FROM workspace_tasks WHERE task=?3",rusqlite::params![reference.as_bytes(),commit,record.intent.request])?;
            }
            if let Some(name) = record
                .intent
                .prompt
                .as_deref()
                .and_then(crate::registry::prompt_name)
            {
                crate::registry::name_from_prompt(&tx, &record.intent.request, &name)?;
            }
        }
        tx.execute("INSERT INTO workspace_creations VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET record=excluded.record",rusqlite::params![record.intent.request,serde_json::to_string(record)?])?;
        tx.commit()?;
        crate::registry::snapshot(&self.db, None)
    }
    fn configure_setup(
        &mut self,
        project: &str,
        command: &Option<String>,
    ) -> Result<crate::registry::Registry> {
        crate::setup::validate_command(command)?;
        self.fault("registry")?;
        if !self.db.query_row(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE id=?1)",
            [project],
            |r| r.get::<_, bool>(0),
        )? {
            return Err("Project unavailable".into());
        }
        let tx = self.db.transaction()?;
        if let Some(command) = command {
            tx.execute("INSERT INTO project_setup VALUES(?1,?2) ON CONFLICT(project) DO UPDATE SET command=excluded.command",rusqlite::params![project,command])?;
        } else {
            tx.execute("DELETE FROM project_setup WHERE project=?1", [project])?;
        }
        tx.commit()?;
        crate::registry::snapshot(&self.db, None)
    }
    fn setups(&mut self) -> Result<Vec<crate::setup::Record>> {
        let mut records: Vec<crate::setup::Record> = self
            .db
            .prepare("SELECT record FROM workspace_setup ORDER BY rowid LIMIT 32")?
            .query_map([], |r| r.get::<_, String>(0))?
            .map(|j| Ok(serde_json::from_str(&j?)?))
            .collect::<Result<_>>()?;
        let obligations=self.db.prepare("SELECT c.record,p.command FROM workspace_setup_policies p JOIN workspace_creations c ON c.id=p.request WHERE p.command IS NOT NULL ORDER BY c.rowid LIMIT 256")?.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?)))?.collect::<std::result::Result<Vec<_>,_>>()?;
        for (json, command) in obligations {
            let creation: crate::creation::Record = serde_json::from_str(&json)?;
            if creation.phase != crate::creation::Phase::Created
                || records
                    .iter()
                    .any(|r| r.creation.intent.request == creation.intent.request)
            {
                continue;
            }
            if records.len() >= 32 {
                return Err("Setup receipt capacity reached while recovering a retained obligation; no execution admitted".into());
            }
            let record=crate::setup::Record {attempt:uuid::Uuid::new_v4().to_string(),creation,command,phase:crate::setup::Phase::Failed,execution:crate::recovery::Execution::prepared()?,cleanup_confirmed:true,output:String::new(),discarded:0,message:"Setup never started: creation completed before any setup Prepared receipt committed. Explicit Retry or Continue without setup required; coding stays stopped".into()};
            self.save_setup(&record)?;
            records.push(record);
        }
        Ok(records)
    }
    fn save_setup(&mut self, record: &crate::setup::Record) -> Result<()> {
        crate::setup::validate_command(&Some(record.command.clone()))?;
        uuid::Uuid::parse_str(&record.attempt)?;
        if serde_json::to_vec(record)?.len() > 65536 {
            return Err("Setup receipt exceeds 64 KiB; retained outcome requires repair".into());
        }
        self.fault(match record.phase {
            crate::setup::Phase::Prepared => "setup-prepare",
            crate::setup::Phase::Authorized => "setup-authorize",
            crate::setup::Phase::Running => "setup-running",
            _ => "setup-result",
        })?;
        let tx = self.db.transaction()?;
        let previous: Option<String> = tx
            .query_row(
                "SELECT record FROM workspace_setup WHERE attempt=?1",
                [&record.attempt],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(previous) = previous {
            let old: crate::setup::Record = serde_json::from_str(&previous)?;
            if old.command != record.command
                || serde_json::to_string(&old.creation)? != serde_json::to_string(&record.creation)?
            {
                return Err("Setup attempt is bound to another command or workspace".into());
            }
        } else if tx.query_row("SELECT count(*) FROM workspace_setup", [], |r| {
            r.get::<_, usize>(0)
        })? >= 32
        {
            return Err("Setup receipt capacity reached".into());
        }
        tx.execute("INSERT INTO workspace_setup VALUES(?1,?2) ON CONFLICT(attempt) DO UPDATE SET record=excluded.record",rusqlite::params![record.attempt,serde_json::to_string(record)?])?;
        tx.commit()?;
        Ok(())
    }
    fn register(
        &mut self,
        checkout: crate::git::CheckoutIdentity,
    ) -> Result<crate::registry::Registry> {
        self.fault("registry")?;
        let current: Option<String> = self
            .db
            .query_row("SELECT task FROM app WHERE id=1", [], |row| row.get(0))
            .optional()?;
        let registry = crate::registry::snapshot(&self.db, current.as_deref())?;
        let task = registry
            .checkout(&checkout)
            .map(|workspace| workspace.task.clone())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let path = crate::session::path_bytes(&checkout.root);
        let state = Observation {
            checkout_identity: Some(checkout),
            ..Observation::default()
        };
        let tx = self.db.transaction()?;
        crate::registry::record(&tx, &state, Some(&path), &task)?;
        tx.commit()?;
        crate::registry::snapshot(&self.db, current.as_deref())
    }
    fn comparison_base(
        &mut self,
        workspace: &str,
        base: &crate::working_diff::Base,
    ) -> Result<crate::registry::Registry> {
        self.fault("registry")?;
        let tx = self.db.transaction()?;
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM workspaces WHERE id=?1)",
            [workspace],
            |r| r.get(0),
        )?;
        if !exists {
            return Err("Workspace unavailable; base not saved".into());
        }
        tx.execute("INSERT INTO workspace_comparison_bases(workspace,reference,base_commit) VALUES(?1,?2,?3) ON CONFLICT(workspace) DO UPDATE SET reference=excluded.reference,base_commit=excluded.base_commit", rusqlite::params![workspace,base.reference,base.commit])?;
        tx.commit()?;
        let current: Option<String> = self
            .db
            .query_row("SELECT task FROM app WHERE id=1", [], |row| row.get(0))
            .optional()?;
        crate::registry::snapshot(&self.db, current.as_deref())
    }
    fn rename(&mut self, workspace: &str, name: &str) -> Result<crate::registry::Registry> {
        self.fault("registry")?;
        let tx = self.db.transaction()?;
        crate::registry::rename(&tx, workspace, name)?;
        tx.commit()?;
        let current: Option<String> = self
            .db
            .query_row("SELECT task FROM app WHERE id=1", [], |row| row.get(0))
            .optional()?;
        crate::registry::snapshot(&self.db, current.as_deref())
    }
    fn save_named(
        &mut self,
        state: &Observation,
        path: Option<Vec<u8>>,
        task: String,
        workspace_name: Option<String>,
    ) -> Result<()> {
        let mut state = state.clone();
        state.screen.clear();
        state.formatted.clear();
        state.decoded = None;
        state.history = None;
        state.cleanup = None;
        state.cleanup_message = None;
        state.error = None;
        let json = serde_json::to_string(&state)?;
        let tx = self.db.transaction()?;
        tx.execute("INSERT INTO app VALUES(1,?1,?2,?3) ON CONFLICT(id) DO UPDATE SET state=excluded.state,checkout=excluded.checkout,task=excluded.task WHERE app.task=excluded.task", params![json,path,task])?;
        crate::registry::record(&tx, &state, path.as_deref(), &task)?;
        if let Some(name) = workspace_name {
            crate::registry::name_from_prompt(&tx, &task, &name)?;
        }
        if let Some(identity) = state.session_identity.as_ref() {
            tx.execute("INSERT INTO sessions(id,identity,state,created) VALUES(?1,?2,?3,unixepoch()) ON CONFLICT(id) DO UPDATE SET state=excluded.state", params![identity.session,identity.label(),json])?;
        }
        if let Some(result) = &state.launch_result {
            tx.execute(
                "INSERT INTO requests VALUES(?1,?2) ON CONFLICT(id) DO NOTHING",
                params![result.request, serde_json::to_string(result)?],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
    fn file(&self, id: &str, chunk: u64) -> PathBuf {
        self.directory.join(format!("{id}-{chunk:08}.transcript"))
    }
    fn append(&mut self, id: &str, bytes: &[u8], accepted: Instant) -> Result<usize> {
        let mut transcript = self.transcripts.remove(id).unwrap_or_default();
        let result = (|| {
            let retained: bool = self.db.query_row(
                "SELECT history_state='retained' FROM sessions WHERE id=?1",
                [id],
                |r| r.get(0),
            )?;
            if !retained {
                return Err("Selected transcript has a deletion tombstone".into());
            }
            if bytes.len() > 4096 {
                return Err("Capture block exceeds 4096 bytes".into());
            }
            if transcript
                .active
                .as_ref()
                .is_some_and(|(active, _, offset)| {
                    active != id || *offset > 0 && *offset % CHUNK == 0
                })
            {
                self.flush_transcript(id, &mut transcript)?;
                transcript.active = None;
            }
            if transcript.active.is_none() {
                let offset: u64 =
                    self.db
                        .query_row("SELECT durable FROM sessions WHERE id=?1", [id], |r| {
                            r.get(0)
                        })?;
                let path = self.file(id, offset / CHUNK);

                let file = OpenOptions::new()
                    .create(true)
                    .truncate(false)
                    .read(true)
                    .write(true)
                    .mode(0o600)
                    .custom_flags(libc::O_NOFOLLOW)
                    .open(path)?;
                // Uncommitted tails are preserved outside the durable prefix; never count them.
                if file.metadata()?.len() != offset % CHUNK {
                    return Err("Uncommitted transcript tail requires explicit recovery".into());
                }
                // Retry directory durability even when a previous creation attempt failed.
                self.session_fault(id, "directory-sync")?;
                File::open(&self.directory)?.sync_all()?;
                transcript.active = Some((id.to_owned(), file, offset));
            }
            transcript.oldest.get_or_insert(accepted);
            let offset = transcript.active.as_ref().unwrap().2;
            let count = bytes.len().min((CHUNK - offset % CHUNK) as usize);
            self.session_fault(id, "write")?;
            let partial = self.session_partial_fault(id);
            let (_, file, offset) = transcript.active.as_mut().unwrap();
            file.seek(SeekFrom::Start(*offset % CHUNK))?;
            if let Some(prefix) = partial {
                file.write_all(&bytes[..count.min(prefix)])?;
                return Err(io::Error::from_raw_os_error(libc::ENOSPC).into());
            }
            file.write_all(&bytes[..count])?;
            *offset += count as u64;
            transcript.active_dirty = true;
            Ok(count)
        })();
        self.transcripts.insert(id.to_owned(), transcript);
        result
    }
    fn flush_session(&mut self, id: &str) -> Result<()> {
        let Some(mut transcript) = self.transcripts.remove(id) else {
            return Ok(());
        };
        let result = self.flush_transcript(id, &mut transcript);
        self.transcripts.insert(id.to_owned(), transcript);
        result
    }
    fn flush_transcript(&mut self, id: &str, transcript: &mut Transcript) -> Result<()> {
        if !transcript.active_dirty || transcript.oldest.is_none() {
            return Ok(());
        }
        self.session_fault(id, "sync")?;
        if let Some((id, file, offset)) = &transcript.active {
            let started = Instant::now();
            file.sync_all()?;
            let tx = self.db.transaction()?;
            tx.execute(
                "UPDATE sessions SET durable=?2 WHERE id=?1",
                params![id, offset],
            )?;
            tx.commit()?;
            transcript.total = *offset;
            transcript.active_dirty = false;
            transcript.sync_micros = started.elapsed().as_micros() as u64;
            transcript.completions += 1;
            transcript.completed_millis = self.origin.elapsed().as_millis() as u64;
            let age = transcript.oldest.take().unwrap().elapsed();
            transcript.accepted_to_durable_millis = age.as_millis() as u64;
            if transcript.commits.len() == 1024 {
                transcript.commits.pop_front();
            }
            transcript.commits.push_back(DurableCommit {
                sequence: transcript.completions,
                prefix: transcript.total,
                completed_micros: self.origin.elapsed().as_micros() as u64,
                sync_commit_micros: transcript.sync_micros,
                oldest_accepted_micros: age.as_micros() as u64,
            });
        }
        Ok(())
    }
    /// Schedule a derivative read; raw retrieval remains its existing bounded seam.
    fn start_readable(
        &mut self,
        target: String,
        request: uuid::Uuid,
        index: usize,
        session: Option<String>,
        offset: u64,
    ) -> Result<HistoryPage> {
        self.history_jobs.retain(|job| job.target != target);
        if self.history_jobs.len() >= 16 {
            return Err("Readable history queue full; retry the selected passage".into());
        }
        let mut page = if let Some(id) = session {
            self.history_session(&id, offset)?
        } else {
            self.history(index, offset)?
        };
        let session = page.session.clone();
        let durable: u64 = self.db.query_row(
            "SELECT durable FROM sessions WHERE id=?1 AND history_state='retained'",
            [&session],
            |row| row.get(0),
        )?;
        let end = page.next.unwrap_or(durable);
        let (position, projection) = self
            .history_checkpoints
            .iter()
            .filter(|(id, position, _)| id == &session && *position <= page.offset)
            .max_by_key(|(_, position, _)| *position)
            .map(|(_, position, projection)| (*position, projection.clone()))
            .unwrap_or_default();
        page.projection_preparing = true;
        self.history_jobs.push_back(HistoryJob {
            target,
            request,
            session,
            page: page.clone(),
            position,
            end,
            projection,
            readable: Default::default(),
        });
        Ok(page)
    }
    /// At most 32 KiB of replay and one bounded page of output per worker turn.
    /// Capture, control, cleanup and flush keep their turns between replay slices.
    fn readable_step(&mut self) -> Option<Completion> {
        let mut job = self.history_jobs.pop_front()?;
        let result = (|| -> Result<bool> {
            let durable: Option<u64> = self
                .db
                .query_row(
                    "SELECT durable FROM sessions WHERE id=?1 AND history_state='retained'",
                    [&job.session],
                    |row| row.get(0),
                )
                .optional()?;
            if durable.is_none_or(|durable| durable < job.end) {
                self.history_checkpoints
                    .retain(|(id, _, _)| id != &job.session);
                return Err("Selected history was deleted or is no longer committed".into());
            }
            let end = (job.position + 32768).min(job.end);
            let raw = self.read_raw(&job.session, job.position, end)?;
            job.projection.feed(
                &raw,
                job.position,
                job.page.offset..job.end,
                &mut job.readable,
            );
            job.position = end;
            self.history_checkpoints
                .push_back((job.session.clone(), end, job.projection.clone()));
            while self.history_checkpoints.len() > 64 {
                self.history_checkpoints.pop_front();
            }
            Ok(end == job.end)
        })();
        match result {
            Ok(false) => {
                self.history_jobs.push_back(job);
                None
            }
            Ok(true) => {
                job.projection
                    .preview(job.end, job.page.offset..job.end, &mut job.readable);
                job.readable.raw_start = job.page.offset;
                job.readable.raw_end = job.end;
                job.readable.committed = job.page.committed;
                job.page.readable = Some(job.readable);
                job.page.projection_preparing = false;
                Some(Completion::HistoryTarget(
                    job.target,
                    job.request,
                    Ok(job.page),
                ))
            }
            Err(error) => Some(Completion::HistoryTarget(
                job.target,
                job.request,
                Err(error.to_string()),
            )),
        }
    }
    fn read_raw(&self, id: &str, mut position: u64, end: u64) -> Result<Vec<u8>> {
        let mut raw = Vec::with_capacity((end - position) as usize);
        while position < end {
            let mut file = OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW)
                .open(self.file(id, position / CHUNK))?;
            file.seek(SeekFrom::Start(position % CHUNK))?;
            let length = (end - position).min(CHUNK - position % CHUNK);
            let mut bytes = vec![0; length as usize];
            file.read_exact(&mut bytes)?;
            raw.extend(bytes);
            position += length;
        }
        Ok(raw)
    }
    fn history(&self, index: usize, offset: u64) -> Result<HistoryPage> {
        let (id, identity, durable, json): (String, String, u64, String) = self
            .db
            .query_row(
                "SELECT id,identity,durable,state FROM sessions WHERE history_state='retained' ORDER BY rowid DESC LIMIT 1 OFFSET ?1",
                [index],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()?
            .ok_or("No retained session at this position")?;
        self.history_record((id, identity, durable, json), offset)
    }
    fn resume_identity(
        &mut self,
        request: &str,
        workspace: &str,
        session: &str,
        task: &str,
    ) -> Result<ResumeIdentity> {
        let workspace: String = self.db.query_row("SELECT id FROM workspaces WHERE id=?1 UNION SELECT workspace FROM workspace_tasks WHERE task=?1 LIMIT 1",[workspace],|row|row.get(0)).optional()?.ok_or("Selected workspace is unavailable; readable history retained")?;
        let known: Option<(String, String, bool)> = self
            .db
            .query_row(
                "SELECT workspace,session,cancelled FROM resume_selections WHERE request=?1",
                [request],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        if let Some((old_workspace, old_session, cancelled)) = known {
            if cancelled || old_workspace != workspace || old_session != session {
                return Err("Resume request already belongs to a different or cancelled selection; explicitly select the conversation again with a new request".into());
            }
        } else {
            let count: u32 =
                self.db
                    .query_row("SELECT count(*) FROM resume_selections", [], |row| {
                        row.get(0)
                    })?;
            if count >= 1024 {
                return Err(
                    "Selected resume request capacity reached; retained choices are preserved"
                        .into(),
                );
            }
            self.db.execute(
                "INSERT INTO resume_selections(request,workspace,session) VALUES(?1,?2,?3)",
                params![request, workspace, session],
            )?;
        }
        self.fault("resume")?;
        let row: Option<(String, String)> = self.db.query_row(
            "SELECT s.state,s.identity FROM sessions s JOIN workspace_sessions m ON m.session=s.id JOIN workspace_tasks t ON t.workspace=m.workspace WHERE s.id=?1 AND (m.workspace=?2 OR m.workspace=(SELECT workspace FROM workspace_tasks WHERE task=?2)) AND t.task=?3",
            params![session,workspace,task], |row| Ok((row.get(0)?,row.get(1)?))).optional()?;
        let (json,label) = row.ok_or("Selected conversation is unavailable or belongs to another workspace; retained history is unchanged. Explicitly choose a new conversation with c/x")?;
        let mut state: Observation = serde_json::from_str(&json)?;
        if state.session_identity.is_none() && state.identity.is_none() {
            state.identity = Some(label);
        }
        let state = decode_record(&serde_json::to_string(&state)?)?;
        if state
            .session_identity
            .as_ref()
            .map(|id| id.session.as_str())
            != Some(session)
        {
            return Err("Selected segment identity disagrees with saved record".into());
        }
        let agent = match state.agent.as_deref() {
            Some("Claude Code") => crate::events::NativeAgent::Claude,
            Some("Codex CLI") => crate::events::NativeAgent::Codex,
            _ => return Err("Native resume unavailable: original tool is unsupported or missing; readable history retained. Explicitly choose c/x for a new conversation".into()),
        };
        let native = state.native_conversation.filter(|id| uuid::Uuid::parse_str(id).is_ok()).ok_or("Native resume unavailable: original native conversation UUID is missing; readable history retained. Explicitly choose c/x for a new conversation")?;
        Ok(ResumeIdentity { agent, native })
    }
    fn history_session(&self, id: &str, offset: u64) -> Result<HistoryPage> {
        let record = self.db.query_row("SELECT id,identity,durable,state FROM sessions WHERE id=?1 AND history_state='retained'", [id], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).optional()?.ok_or("No retained session at the selected identity")?;
        self.history_record(record, offset)
    }
    fn history_record(
        &self,
        (id, identity, durable, json): (String, String, u64, String),
        offset: u64,
    ) -> Result<HistoryPage> {
        let offset = offset.min(durable);
        let end = (offset + PAGE).min(durable);
        let mut raw = Vec::with_capacity(PAGE as usize);
        let mut position = offset;
        while position < end {
            let mut file = OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW)
                .open(self.file(&id, position / CHUNK))?;
            file.seek(SeekFrom::Start(position % CHUNK))?;
            let length = (end - position).min(CHUNK - position % CHUNK);
            let mut bytes = vec![0; length as usize];
            file.read_exact(&mut bytes)?;
            raw.extend(bytes);
            position += length;
        }
        let text = String::from_utf8_lossy(&raw)
            .chars()
            .flat_map(|ch| {
                if ch == '\n' || !ch.is_control() {
                    vec![ch]
                } else {
                    ch.escape_default().collect()
                }
            })
            .collect();
        let state: Observation = decode_record(&json)?;
        Ok(HistoryPage {
            session: id,
            committed: durable,
            identity,
            checkout: state.adopted.unwrap_or_default(),
            lifecycle: state.lifecycle,
            process_exit: state.process_exit,
            text,
            readable: None,
            projection_preparing: false,
            offset,
            next: (end < durable).then_some(end),
        })
    }
}
/// Explicit compatibility boundary for pre-typed durable records. Runtime key access
/// never parses display text. Refuse malformed/oversized records visibly; retain files.
fn decode_record(json: &str) -> Result<Observation> {
    let mut state: Observation = serde_json::from_str(json)?;
    if state.session_identity.is_none()
        && let Some(label) = &state.identity
    {
        if label.len() > 512 {
            return Err("Legacy session identity exceeds bounded format".into());
        }
        let (task, rest) = label
            .strip_prefix("task=")
            .and_then(|value| value.split_once(" session="))
            .ok_or("Invalid legacy task/session identity")?;
        let (session, request) = rest
            .split_once(" request=")
            .ok_or("Missing legacy request identity")?;
        if task.is_empty() || task.len() > 128 || request.is_empty() || request.len() > 128 {
            return Err("Invalid legacy identity lengths".into());
        }
        state.session_identity = Some(crate::events::SessionIdentity {
            task: task.to_owned(),
            session: uuid::Uuid::parse_str(session)?.to_string(),
            request: request.to_owned(),
        });
    }
    if let Some(identity) = &state.session_identity {
        if uuid::Uuid::parse_str(&identity.session)?.to_string() != identity.session {
            return Err("Invalid durable session UUID".into());
        }
        state.identity = Some(identity.label());
    }
    Ok(state)
}

#[cfg(test)]
mod concurrent_tests {
    use super::*;
    fn saved(worker: &Worker, session: &str) {
        saved_at(worker, session, None);
    }
    fn saved_at(worker: &Worker, session: &str, checkout: Option<&Path>) {
        let state = Observation {
            adopted: checkout.map(|path| path.display().to_string()),
            session_identity: Some(crate::events::SessionIdentity {
                session: session.into(),
                task: uuid::Uuid::new_v4().to_string(),
                request: "capture-test".into(),
            }),
            lifecycle: Lifecycle::Stopped,
            ..Observation::default()
        };
        worker
            .commands
            .send(Command::Save(Snapshot::new(
                &state,
                checkout,
                &state.session_identity.as_ref().unwrap().task,
            )))
            .unwrap();
        loop {
            match worker.events.recv_timeout(Duration::from_secs(3)).unwrap() {
                Completion::Committed(_) => break,
                Completion::Failed(_, error) => panic!("{error}"),
                _ => {}
            }
        }
    }
    #[test]
    fn selected_resume_choices_survive_restart_and_transcript_deletion() {
        let directory = tempfile::tempdir().unwrap();
        let (worker, _) = Worker::start(directory.path()).unwrap();
        let task = uuid::Uuid::new_v4().to_string();
        let session = uuid::Uuid::new_v4().to_string();
        let native = "11111111-2222-4333-8444-555555555555";
        let state = Observation {
            session_identity: Some(crate::events::SessionIdentity {
                task: task.clone(),
                session: session.clone(),
                request: "original".into(),
            }),
            agent: Some("Claude Code".into()),
            native_conversation: Some(native.into()),
            lifecycle: Lifecycle::Exited,
            ..Default::default()
        };
        worker
            .commands
            .send(Command::Save(Snapshot::new(&state, None, &task)))
            .unwrap();
        let workspace = loop {
            if let Completion::Registry(registry) =
                worker.events.recv_timeout(Duration::from_secs(3)).unwrap()
            {
                break registry
                    .workspaces
                    .iter()
                    .find(|w| w.task == task)
                    .unwrap()
                    .id
                    .clone();
            }
        };
        let resolve = |worker: &Worker, request: &str, selected: &str| {
            worker
                .commands
                .send(Command::ResumeIdentity {
                    target: "owner".into(),
                    request: request.into(),
                    workspace: workspace.clone(),
                    session: selected.into(),
                    task: task.clone(),
                })
                .unwrap();
            loop {
                if let Completion::ResumeIdentity(target, id, result) =
                    worker.events.recv_timeout(Duration::from_secs(3)).unwrap()
                {
                    assert_eq!(target, "owner");
                    assert_eq!(id, request);
                    break result;
                }
            }
        };
        assert_eq!(
            resolve(&worker, "selected-request", &session)
                .unwrap()
                .native,
            native
        );
        for (agent, native_id, message) in [
            ("Claude Code", None, "native conversation UUID is missing"),
            (
                "unsupported tool",
                Some(native.to_owned()),
                "tool is unsupported or missing",
            ),
        ] {
            let mut unavailable = state.clone();
            let unavailable_session = uuid::Uuid::new_v4().to_string();
            unavailable.session_identity.as_mut().unwrap().session = unavailable_session.clone();
            unavailable.agent = Some(agent.into());
            unavailable.native_conversation = native_id;
            let snapshot = Snapshot::new(&unavailable, None, &task);
            let operation = snapshot.operation.clone();
            worker.commands.send(Command::Save(snapshot)).unwrap();
            loop {
                if let Completion::Committed(done) =
                    worker.events.recv_timeout(Duration::from_secs(3)).unwrap()
                    && done == operation
                {
                    break;
                }
            }
            assert!(
                resolve(
                    &worker,
                    &uuid::Uuid::new_v4().to_string(),
                    &unavailable_session
                )
                .err()
                .unwrap()
                .contains(message)
            );
        }
        worker
            .commands
            .send(Command::Seal(Operation::new(&state, Purpose::Seal)))
            .unwrap();
        loop {
            if let Completion::Committed(operation) =
                worker.events.recv_timeout(Duration::from_secs(3)).unwrap()
                && operation.purpose == Purpose::Seal
            {
                break;
            }
        }
        worker
            .commands
            .send(Command::Delete(session.clone()))
            .unwrap();
        loop {
            if let Completion::Deleted(id) =
                worker.events.recv_timeout(Duration::from_secs(3)).unwrap()
            {
                assert_eq!(id, session);
                break;
            }
        }
        assert_eq!(
            resolve(&worker, "after-delete", &session).unwrap().native,
            native
        );
        worker
            .commands
            .send(Command::CancelResume {
                target: "owner".into(),
                request: "selected-request".into(),
            })
            .unwrap();
        loop {
            if let Completion::ResumeCancelled(_, id, result) =
                worker.events.recv_timeout(Duration::from_secs(3)).unwrap()
            {
                assert_eq!(id, "selected-request");
                result.unwrap();
                break;
            }
        }
        drop(worker);
        let (worker, _) = Worker::start(directory.path()).unwrap();
        assert!(
            resolve(&worker, "selected-request", &session)
                .err()
                .unwrap()
                .contains("cancelled")
        );
        assert!(
            resolve(&worker, "after-delete", &uuid::Uuid::new_v4().to_string())
                .err()
                .unwrap()
                .contains("different")
        );
        let descriptor = resolve(&worker, "new-explicit-request", &session).unwrap();
        assert_eq!(descriptor.native, native);
        assert!(matches!(
            descriptor.agent,
            crate::events::NativeAgent::Claude
        ));
    }
    fn projected(worker: &Worker, session: &str, offset: u64) -> HistoryPage {
        let request = uuid::Uuid::new_v4();
        worker
            .commands
            .send(Command::HistoryTarget {
                target: "readable-test".into(),
                request,
                index: 0,
                session: Some(session.into()),
                offset,
            })
            .unwrap();
        loop {
            if let Completion::HistoryTarget(target, actual, result) =
                worker.events.recv_timeout(Duration::from_secs(5)).unwrap()
                && target == "readable-test"
                && actual == request
            {
                let page = result.unwrap();
                if !page.projection_preparing {
                    return page;
                }
            }
        }
    }
    fn capture_bytes(worker: &Worker, session: &str, raw: &[u8]) {
        let capture = worker.capture_for(session).unwrap();
        for bytes in raw.chunks(4096) {
            capture.accept(bytes.len());
            capture
                .send((session.into(), bytes.to_vec(), Instant::now()), None)
                .unwrap();
        }
        let operation = final_operation(session);
        worker
            .commands
            .send(Command::Flush(operation.clone()))
            .unwrap();
        committed(worker, &operation);
    }
    #[test]
    fn search_opens_exact_committed_passage_after_long_terminal_controls_and_restart() {
        let root = tempfile::tempdir().unwrap();
        let session = uuid::Uuid::new_v4().to_string();
        let (worker, _) = Worker::start(root.path()).unwrap();
        saved(&worker, &session);
        let mut raw = "café ".as_bytes().to_vec();
        raw.extend(b"\x1b]52;c;");
        raw.extend(vec![b'x'; 9000]);
        raw.extend(b"\x07needle\n");
        capture_bytes(&worker, &session, &raw);
        let result = search_until(&worker, "needle");
        assert_eq!(result.rows.len(), 1);
        let hit = result.rows[0].passage.clone().unwrap();
        drop(worker);
        thread::sleep(Duration::from_millis(30));
        OpenOptions::new()
            .append(true)
            .open(root.path().join(format!("{session}-00000000.transcript")))
            .unwrap()
            .write_all(b"UNCOMMITTEDSECRET")
            .unwrap();
        let (worker, _) = Worker::start(root.path()).unwrap();
        let uncommitted = search::Query::new(search::Mode::History, "UNCOMMITTEDSECRET");
        let uncommitted_request = uncommitted.request.clone();
        worker.commands.send(Command::Browse(uncommitted)).unwrap();
        loop {
            if let Completion::Browse(response) =
                worker.events.recv_timeout(Duration::from_secs(5)).unwrap()
                && response.request == uncommitted_request
                && !response.preparing
            {
                assert!(response.rows.is_empty());
                break;
            }
        }
        let request = uuid::Uuid::new_v4().to_string();
        worker
            .commands
            .send(Command::Browse(search::Query::open(&request, hit)))
            .unwrap();
        loop {
            if let Completion::Browse(response) =
                worker.events.recv_timeout(Duration::from_secs(5)).unwrap()
                && response.request == request
            {
                assert_eq!(response.opened.unwrap().text, "café needle\n");
                break;
            }
        }
    }
    fn search_until(worker: &Worker, words: &str) -> search::Response {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let query = search::Query::new(search::Mode::History, words);
            let request = query.request.clone();
            worker.commands.send(Command::Browse(query)).unwrap();
            loop {
                if let Completion::Browse(response) =
                    worker.events.recv_timeout(Duration::from_secs(5)).unwrap()
                    && response.request == request
                {
                    assert!(response.error.is_none(), "{:?}", response.error);
                    if response.preparing {
                        continue;
                    }
                    if !response.rows.is_empty() {
                        return response;
                    }
                    break;
                }
            }
            assert!(Instant::now() < deadline, "index did not catch up");
            thread::sleep(Duration::from_millis(10));
        }
    }
    #[test]
    fn deleted_search_text_never_reappears_after_restart_but_session_identity_remains() {
        let root = tempfile::tempdir().unwrap();
        let session = uuid::Uuid::new_v4().to_string();
        let (worker, _) = Worker::start(root.path()).unwrap();
        saved(&worker, &session);
        capture_bytes(&worker, &session, b"deletable secretword\n");
        let hit = search_until(&worker, "secretword").rows[0]
            .passage
            .clone()
            .unwrap();
        let seal = Operation {
            purpose: Purpose::Seal,
            ..final_operation(&session)
        };
        worker.commands.send(Command::Seal(seal.clone())).unwrap();
        committed(&worker, &seal);
        worker
            .commands
            .send(Command::Delete(session.clone()))
            .unwrap();
        loop {
            if let Completion::Deleted(id) =
                worker.events.recv_timeout(Duration::from_secs(5)).unwrap()
                && id == session
            {
                break;
            }
        }
        drop(worker);
        thread::sleep(Duration::from_millis(30));
        let (worker, _) = Worker::start(root.path()).unwrap();
        let query = search::Query::new(search::Mode::History, "secretword");
        let request = query.request.clone();
        worker.commands.send(Command::Browse(query)).unwrap();
        loop {
            if let Completion::Browse(response) =
                worker.events.recv_timeout(Duration::from_secs(5)).unwrap()
                && response.request == request
                && !response.preparing
            {
                assert!(response.rows.is_empty());
                break;
            }
        }
        worker
            .commands
            .send(Command::Browse(search::Query::open("deleted-open", hit)))
            .unwrap();
        loop {
            if let Completion::Browse(response) =
                worker.events.recv_timeout(Duration::from_secs(5)).unwrap()
                && response.request == "deleted-open"
            {
                assert!(response.opened.is_none());
                assert!(response.error.unwrap().contains("deleted"));
                break;
            }
        }
        let query = search::Query::new(search::Mode::Sessions, "");
        let request = query.request.clone();
        worker.commands.send(Command::Browse(query)).unwrap();
        loop {
            if let Completion::Browse(response) =
                worker.events.recv_timeout(Duration::from_secs(5)).unwrap()
                && response.request == request
                && !response.preparing
            {
                assert_eq!(response.rows[0].session.as_deref(), Some(session.as_str()));
                assert!(response.rows[0].label.contains("history deleted"));
                break;
            }
        }
    }
    #[test]
    fn retained_browser_includes_archived_missing_git_workspace_branch_and_cancelled_queries() {
        let root = tempfile::tempdir().unwrap();
        let checkout = root.path().join("historical-project");
        assert!(
            std::process::Command::new("git")
                .args(["init", "-b", "searchbranch11"])
                .arg(&checkout)
                .output()
                .unwrap()
                .status
                .success()
        );
        let identity = crate::git::resolve_checkout(&checkout, &AtomicBool::new(false)).unwrap();
        let session = uuid::Uuid::new_v4().to_string();
        let task = uuid::Uuid::new_v4().to_string();
        let state = Observation {
            checkout_identity: Some(identity),
            session_identity: Some(crate::events::SessionIdentity {
                session: session.clone(),
                task: task.clone(),
                request: "search-fixture".into(),
            }),
            lifecycle: Lifecycle::Stopped,
            ..Default::default()
        };
        let (worker, _) = Worker::start(root.path()).unwrap();
        let snapshot = Snapshot::new(&state, Some(&checkout), &task);
        let operation = snapshot.operation.clone();
        worker.commands.send(Command::Save(snapshot)).unwrap();
        committed(&worker, &operation);
        worker
            .commands
            .send(Command::ObservedBranches(vec![(
                crate::session::path_bytes(&checkout),
                "searchbranch11".into(),
            )]))
            .unwrap();
        loop {
            if matches!(
                worker.events.recv_timeout(Duration::from_secs(5)).unwrap(),
                Completion::Sealed
            ) {
                break;
            }
        }
        capture_bytes(
            &worker,
            &session,
            "archived searchable café answer\n".as_bytes(),
        );
        search_until(&worker, "searchable");
        drop(worker);
        thread::sleep(Duration::from_millis(30));
        // Seed future archive metadata through its schema contract before reopening the real worker.
        let db = Connection::open(root.path().join("history.sqlite3")).unwrap();
        db.execute(
            "INSERT INTO workspace_history_facts(workspace,archived) SELECT id,1 FROM workspaces",
            [],
        )
        .unwrap();
        let tx = db.unchecked_transaction().unwrap();
        for index in 0..1000 {
            tx.execute(
                "INSERT INTO workspaces(id,task,name) VALUES(?1,?1,?2)",
                params![format!("fixture-{index}"), format!("workspace {index}")],
            )
            .unwrap();
        }
        tx.commit().unwrap();
        drop(db);
        std::fs::remove_dir_all(&checkout).unwrap();
        let (worker, _) = Worker::start(root.path()).unwrap();
        let response = search_until(&worker, "searchable");
        let label = &response.rows[0].label;
        for fact in [
            "historical-project",
            "searchbranch11",
            "archived",
            "checkout missing",
        ] {
            assert!(label.contains(fact), "{label}");
        }
        let old = search::Query::new(search::Mode::Workspaces, "never-matches");
        let old_request = old.request.clone();
        let mut cancel = search::Query::new(search::Mode::Workspaces, "");
        cancel.cancel = true;
        let request = cancel.request.clone();
        worker.commands.send(Command::Browse(old)).unwrap();
        worker.commands.send(Command::Browse(cancel)).unwrap();
        loop {
            if let Completion::Browse(response) =
                worker.events.recv_timeout(Duration::from_secs(5)).unwrap()
            {
                assert!(
                    response.request != old_request || response.preparing,
                    "cancelled scan published final results"
                );
                if response.request == request {
                    assert!(response.rows.is_empty());
                    break;
                }
            }
        }
    }
    #[test]
    fn search_checkpoint_restarts_split_utf8_and_rolls_back_failed_index_commit() {
        let root = tempfile::tempdir().unwrap();
        let session = uuid::Uuid::new_v4().to_string();
        let (worker, _) = Worker::start(root.path()).unwrap();
        saved(&worker, &session);
        let mut prefix = b"checkpointprefix\n".to_vec();
        prefix.resize(511, b'x');
        prefix.push(0xc3);
        capture_bytes(&worker, &session, &prefix);
        search_until(&worker, "checkpointprefix");
        drop(worker);
        thread::sleep(Duration::from_millis(30));
        std::fs::write(root.path().join("proof-storage-index-commit"), "error").unwrap();
        let (worker, _) = Worker::start(root.path()).unwrap();
        capture_bytes(
            &worker,
            &session,
            &[0xa9, b' ', b'n', b'e', b'e', b'd', b'l', b'e', b'\n'],
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let query = search::Query::new(search::Mode::History, "needle");
            let request = query.request.clone();
            worker.commands.send(Command::Browse(query)).unwrap();
            let response = loop {
                if let Completion::Browse(response) =
                    worker.events.recv_timeout(Duration::from_secs(5)).unwrap()
                    && response.request == request
                    && !response.preparing
                {
                    break response;
                }
            };
            assert!(
                response.rows.is_empty(),
                "failed index transaction published text"
            );
            if response.error.is_some_and(|e| e.contains("History index")) {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "index failure did not become visible"
            );
            thread::sleep(Duration::from_millis(10));
        }
        drop(worker);
        thread::sleep(Duration::from_millis(30));
        std::fs::remove_file(root.path().join("proof-storage-index-commit")).unwrap();
        let (worker, _) = Worker::start(root.path()).unwrap();
        let response = search_until(&worker, "needle");
        assert_eq!(response.rows.len(), 1);
        let passage = response.rows[0].passage.clone().unwrap();
        worker
            .commands
            .send(Command::Browse(search::Query::open("split-open", passage)))
            .unwrap();
        loop {
            if let Completion::Browse(response) =
                worker.events.recv_timeout(Duration::from_secs(5)).unwrap()
                && response.request == "split-open"
            {
                let page = response.opened.unwrap();
                assert!(page.text.ends_with("é needle\n"));
                assert!(!page.text.contains('�'));
                break;
            }
        }
    }
    #[test]
    fn readable_committed_history_replays_across_pages_and_chunks_after_restart_without_a_checkout()
    {
        let root = tempfile::tempdir().unwrap();
        let session = uuid::Uuid::new_v4().to_string();
        let mut raw = b"before\n\x1b]52;c;".to_vec();
        raw.extend(vec![b'H'; 70000]);
        raw.extend("\x07Prompt café\n\x1b[32mAnswer\x1b[0m\ncommand output\n".as_bytes());
        let (worker, _) = Worker::start(root.path()).unwrap();
        let checkout = root.path().join("removed-checkout");
        std::fs::create_dir(&checkout).unwrap();
        saved_at(&worker, &session, Some(&checkout));
        capture_bytes(&worker, &session, &raw);
        let offset = raw.len() as u64 / PAGE * PAGE;
        let page = projected(&worker, &session, offset);
        let readable = page.readable.unwrap();
        assert_eq!(readable.text, "Prompt café\nAnswer\ncommand output\n");
        assert_eq!(readable.passages[0].raw_start, 70015);
        assert_eq!(page.checkout, checkout.display().to_string());
        assert_eq!(page.committed, raw.len() as u64);
        std::fs::remove_dir(&checkout).unwrap();
        drop(worker);
        thread::sleep(Duration::from_millis(20));
        // An uncommitted tail must never appear in either display.
        let mut tail = OpenOptions::new()
            .append(true)
            .open(root.path().join(format!("{session}-00000001.transcript")))
            .unwrap();
        tail.write_all(b"UNCOMMITTED-SECRET").unwrap();
        let (worker, _) = Worker::start(root.path()).unwrap();
        let restored = projected(&worker, &session, offset);
        assert!(!checkout.exists());
        assert_eq!(restored.checkout, checkout.display().to_string());
        assert_eq!(restored.readable.unwrap().text, readable.text);
        assert!(!restored.text.contains("UNCOMMITTED"));
        // Exact retained output is independent of the derivative text.
        assert_eq!(
            std::fs::read(root.path().join(format!("{session}-00000000.transcript"))).unwrap(),
            raw[..65536]
        );
    }
    #[test]
    fn deleting_history_cancels_replay_and_cannot_resurrect_checkpointed_text() {
        let root = tempfile::tempdir().unwrap();
        let (worker, _) = Worker::start(root.path()).unwrap();
        let session = uuid::Uuid::new_v4().to_string();
        let other = uuid::Uuid::new_v4().to_string();
        saved(&worker, &session);
        saved(&worker, &other);
        capture_bytes(&worker, &session, &vec![b'x'; 200000]);
        capture_bytes(&worker, &other, b"other retained answer\n");
        let seal = Operation {
            purpose: Purpose::Seal,
            ..final_operation(&session)
        };
        worker.commands.send(Command::Seal(seal.clone())).unwrap();
        committed(&worker, &seal);
        let request = uuid::Uuid::new_v4();
        worker
            .commands
            .send(Command::HistoryTarget {
                target: "deleting-reader".into(),
                request,
                index: 0,
                session: Some(session.clone()),
                offset: 190000,
            })
            .unwrap();
        worker
            .commands
            .send(Command::Delete(session.clone()))
            .unwrap();
        let mut cancelled = false;
        let mut deleted = false;
        while !cancelled || !deleted {
            match worker.events.recv_timeout(Duration::from_secs(5)).unwrap() {
                Completion::HistoryTarget(target, actual, Err(error))
                    if target == "deleting-reader" && actual == request =>
                {
                    assert!(error.contains("deleted") || error.contains("committed"));
                    cancelled = true;
                }
                Completion::Deleted(id) if id == session => deleted = true,
                Completion::HistoryTarget(_, actual, Ok(page)) if actual == request => {
                    assert!(page.projection_preparing)
                }
                Completion::Error(kind, error) => panic!("{kind}: {error}"),
                _ => {}
            }
        }
        assert_eq!(
            projected(&worker, &other, 0).readable.unwrap().text,
            "other retained answer\n"
        );
        worker
            .commands
            .send(Command::HistoryTarget {
                target: "deleted-reader".into(),
                request,
                index: 0,
                session: Some(session),
                offset: 0,
            })
            .unwrap();
        loop {
            if let Completion::HistoryTarget(target, actual, result) =
                worker.events.recv_timeout(Duration::from_secs(5)).unwrap()
                && target == "deleted-reader"
                && actual == request
            {
                assert!(result.is_err());
                break;
            }
        }
    }
    #[test]
    #[ignore = "explicit current native committed corpus qualification; no child is launched"]
    fn qualify_readable_native_committed_corpus() {
        let source = PathBuf::from(
            std::env::var_os("RAT_HISTORY_CORPUS").expect("RAT_HISTORY_CORPUS required"),
        );
        let expected = std::env::var("RAT_HISTORY_MARKER").expect("RAT_HISTORY_MARKER required");
        let output = std::env::var("RAT_HISTORY_OUTPUT").expect("RAT_HISTORY_OUTPUT required");
        let command = std::env::var("RAT_HISTORY_COMMAND").expect("RAT_HISTORY_COMMAND required");
        let db = Connection::open_with_flags(
            source.join("history.sqlite3"),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .unwrap();
        let (session, durable): (String, u64) = db.query_row("SELECT id,durable FROM sessions WHERE history_state='retained' ORDER BY rowid DESC LIMIT 1", [], |row| Ok((row.get(0)?, row.get(1)?))).unwrap();
        let mut raw = Vec::new();
        while raw.len() < durable as usize {
            let bytes = std::fs::read(source.join(format!(
                "{session}-{:08}.transcript",
                raw.len() as u64 / CHUNK
            )))
            .unwrap();
            raw.extend_from_slice(&bytes[..bytes.len().min(durable as usize - raw.len())]);
        }
        let root = tempfile::tempdir().unwrap();
        let (worker, _) = Worker::start(root.path()).unwrap();
        saved(&worker, &session);
        capture_bytes(&worker, &session, &raw);
        let mut pages = Vec::new();
        let mut offset = 0;
        loop {
            let page = projected(&worker, &session, offset);
            let next = page.next;
            pages.push(page);
            let Some(next) = next else {
                break;
            };
            offset = next;
        }
        let text = pages
            .iter()
            .map(|page| page.readable.as_ref().unwrap().text.as_str())
            .collect::<String>();
        assert!(
            text.contains(&expected),
            "native response marker missing: {text}"
        );
        assert!(
            text.contains(&command),
            "native visible command missing: {text}"
        );
        assert!(
            text.contains(&output),
            "native command output missing: {text}"
        );
        let directory = PathBuf::from(
            std::env::var_os("RAT_HISTORY_EVIDENCE").expect("RAT_HISTORY_EVIDENCE required"),
        );
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join(format!("{expected}-readable.json")), serde_json::to_vec_pretty(&serde_json::json!({"source_state":source,"source_session":session,"source_committed":durable,"pages":pages})).unwrap()).unwrap();
        std::fs::write(directory.join(format!("{expected}-committed.raw")), raw).unwrap();
    }
    #[test]
    #[ignore = "explicit retained native corpus search replay; no native process is launched"]
    fn qualify_search_native_committed_corpus() {
        let source =
            PathBuf::from(std::env::var_os("RAT_HISTORY_CORPUS").expect("RAT_HISTORY_CORPUS"));
        let expected = std::env::var("RAT_HISTORY_MARKER").expect("RAT_HISTORY_MARKER");
        let evidence =
            PathBuf::from(std::env::var_os("RAT_HISTORY_EVIDENCE").expect("RAT_HISTORY_EVIDENCE"));
        let root = tempfile::tempdir().unwrap();
        for entry in std::fs::read_dir(&source).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_file() {
                std::fs::copy(entry.path(), root.path().join(entry.file_name())).unwrap();
            }
        }
        let db = Connection::open_with_flags(
            source.join("history.sqlite3"),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .unwrap();
        let (session,durable,state):(String,u64,String)=db.query_row("SELECT id,durable,state FROM sessions WHERE history_state='retained' ORDER BY rowid DESC LIMIT 1",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
        let original = decode_record(&state).unwrap();
        let (worker, restored) = Worker::start(root.path()).unwrap();
        assert_eq!(
            restored.state.native_conversation,
            original.native_conversation
        );
        assert_eq!(restored.state.agent, original.agent);
        let response = search_until(&worker, &expected);
        let mut opened = Vec::new();
        for row in &response.rows {
            assert_eq!(row.session.as_deref(), Some(session.as_str()));
            let passage = row.passage.clone().unwrap();
            assert!(passage.raw_end <= durable);
            let request = uuid::Uuid::new_v4().to_string();
            worker
                .commands
                .send(Command::Browse(search::Query::open(&request, passage)))
                .unwrap();
            loop {
                if let Completion::Browse(result) =
                    worker.events.recv_timeout(Duration::from_secs(5)).unwrap()
                    && result.request == request
                {
                    opened.push(result.opened.unwrap());
                    break;
                }
            }
        }
        assert!(
            opened.iter().any(|p| p.text.contains(&expected)),
            "matching native marker did not reopen"
        );
        std::fs::create_dir_all(&evidence).unwrap();
        std::fs::write(evidence.join(format!("{expected}-search.json")),serde_json::to_vec_pretty(&serde_json::json!({"source_corpus":source,"source_session":session,"source_committed":durable,"source_native_conversation":original.native_conversation,"source_agent":original.agent,"query":expected,"response":response,"opened":opened,"scope":"current search replay of retained08 native corpus; no new native run or native search timing"})).unwrap()).unwrap();
    }
    #[test]
    fn independent_capture_prefixes_and_scoped_final_flush() {
        let root = std::env::temp_dir().join(format!("rat-capture-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let (worker, _) = Worker::start(&root).unwrap();
        let a = uuid::Uuid::new_v4().to_string();
        let b = uuid::Uuid::new_v4().to_string();
        saved(&worker, &a);
        saved(&worker, &b);
        let ca = worker.capture_for(&a).unwrap();
        let cb = worker.capture_for(&b).unwrap();
        for _ in 0..16 {
            ca.accept(3);
            ca.send((a.clone(), b"AAA".to_vec(), Instant::now()), None)
                .unwrap();
            cb.accept(5);
            cb.send((b.clone(), b"bbbbb".to_vec(), Instant::now()), None)
                .unwrap();
        }
        let operation = Operation {
            id: uuid::Uuid::new_v4(),
            session: Some(a.clone()),
            request: "capture-test".into(),
            purpose: Purpose::FinalFlush,
        };
        worker
            .commands
            .send(Command::Flush(operation.clone()))
            .unwrap();
        loop {
            if let Completion::Committed(done) =
                worker.events.recv_timeout(Duration::from_secs(3)).unwrap()
                && done == operation
            {
                break;
            }
        }
        let pa = worker.progress_for(Some(&a));
        assert_eq!(
            (pa.accepted, pa.written, pa.durable, pa.pending_bytes),
            (48, 48, 48, 0)
        );
        assert_eq!(worker.progress_for(Some(&b)).accepted, 80);
        worker
            .commands
            .send(Command::HistorySession(a.clone(), 0))
            .unwrap();
        loop {
            if let Completion::History(page) =
                worker.events.recv_timeout(Duration::from_secs(3)).unwrap()
            {
                assert_eq!(page.text, "AAA".repeat(16));
                break;
            }
        }
        worker
            .commands
            .send(Command::Flush(Operation {
                session: Some(b.clone()),
                ..operation
            }))
            .unwrap();
        loop {
            match worker.events.recv_timeout(Duration::from_secs(3)).unwrap() {
                Completion::Committed(_) => break,
                Completion::Failed(_, error) => panic!("{error}"),
                _ => {}
            }
        }
        assert_eq!(worker.progress_for(Some(&b)).durable, 80);
        drop(worker);
        std::fs::remove_dir_all(root).unwrap();
    }
    fn wait_for(mut check: impl FnMut() -> bool) {
        let until = Instant::now() + Duration::from_secs(5);
        while !check() {
            assert!(Instant::now() < until, "capture progress deadline exceeded");
            thread::sleep(Duration::from_millis(5));
        }
    }
    fn final_operation(session: &str) -> Operation {
        Operation {
            id: uuid::Uuid::new_v4(),
            session: Some(session.into()),
            request: "capture-test".into(),
            purpose: Purpose::FinalFlush,
        }
    }
    fn committed(worker: &Worker, operation: &Operation) {
        loop {
            match worker.events.recv_timeout(Duration::from_secs(5)).unwrap() {
                Completion::Committed(done) if &done == operation => return,
                Completion::Failed(done, error) if &done == operation => panic!("{error}"),
                _ => {}
            }
        }
    }
    #[test]
    fn switching_capture_streams_does_not_require_a_commit() {
        let root = tempfile::tempdir().unwrap();
        let (worker, _) = Worker::start(root.path()).unwrap();
        let a = uuid::Uuid::new_v4().to_string();
        let b = uuid::Uuid::new_v4().to_string();
        saved(&worker, &a);
        saved(&worker, &b);
        // Freeze every durable commit. Both streams must still append their full
        // interleaving; switching a handle cannot introduce a durability barrier.
        std::fs::write(root.path().join("proof-storage-sync"), "error").unwrap();
        let ca = worker.capture_for(&a).unwrap();
        let cb = worker.capture_for(&b).unwrap();
        for _ in 0..32 {
            ca.accept(3);
            ca.send((a.clone(), b"AAA".to_vec(), Instant::now()), None)
                .unwrap();
            cb.accept(5);
            cb.send((b.clone(), b"bbbbb".to_vec(), Instant::now()), None)
                .unwrap();
        }
        wait_for(|| {
            worker.progress_for(Some(&a)).written == 96
                && worker.progress_for(Some(&b)).written == 160
        });
        assert_eq!(worker.progress_for(Some(&a)).durable, 0);
        assert_eq!(worker.progress_for(Some(&b)).durable, 0);
        std::fs::remove_file(root.path().join("proof-storage-sync")).unwrap();
        for id in [&a, &b] {
            let operation = final_operation(id);
            worker
                .commands
                .send(Command::Flush(operation.clone()))
                .unwrap();
            committed(&worker, &operation);
        }
        assert_eq!(worker.progress_for(Some(&a)).durable, 96);
        assert_eq!(worker.progress_for(Some(&b)).durable, 160);
    }
    #[test]
    fn faulted_stream_retains_exact_bytes_while_other_stream_flushes_and_seals() {
        let root = tempfile::tempdir().unwrap();
        let (worker, _) = Worker::start(root.path()).unwrap();
        let a = uuid::Uuid::new_v4().to_string();
        let b = uuid::Uuid::new_v4().to_string();
        saved(&worker, &a);
        saved(&worker, &b);
        let ca = worker.capture_for(&a).unwrap();
        let cb = worker.capture_for(&b).unwrap();
        std::fs::write(root.path().join("proof-storage-session"), &b).unwrap();
        std::fs::write(root.path().join("proof-storage-partial"), "2").unwrap();
        cb.accept(5);
        cb.send((b.clone(), b"bbbbb".to_vec(), Instant::now()), None)
            .unwrap();
        wait_for(|| worker.progress_for(Some(&b)).error.is_some());
        ca.accept(3);
        ca.send((a.clone(), b"AAA".to_vec(), Instant::now()), None)
            .unwrap();
        let operation = final_operation(&a);
        worker
            .commands
            .send(Command::Flush(operation.clone()))
            .unwrap();
        committed(&worker, &operation);
        assert_eq!(worker.progress_for(Some(&a)).durable, 3);
        let before = worker.progress_for(Some(&b));
        assert_eq!(
            (
                before.accepted,
                before.written,
                before.durable,
                before.pending_bytes
            ),
            (5, 0, 0, 5)
        );
        assert!(before.error.is_some());
        let seal = Operation {
            purpose: Purpose::Seal,
            ..final_operation(&a)
        };
        worker.commands.send(Command::Seal(seal.clone())).unwrap();
        committed(&worker, &seal);
        let during = worker.progress_for(Some(&b));
        assert_eq!(
            (during.accepted, during.written, during.pending_bytes),
            (5, 0, 5)
        );
        assert!(during.error.is_some());
        std::fs::remove_file(root.path().join("proof-storage-partial")).unwrap();
        wait_for(|| worker.progress_for(Some(&b)).written == 5);
        let operation = final_operation(&b);
        worker
            .commands
            .send(Command::Flush(operation.clone()))
            .unwrap();
        committed(&worker, &operation);
        assert_eq!(worker.progress_for(Some(&b)).durable, 5);
        worker.commands.send(Command::HistorySession(b, 0)).unwrap();
        loop {
            if let Completion::History(page) =
                worker.events.recv_timeout(Duration::from_secs(5)).unwrap()
            {
                assert_eq!(page.text, "bbbbb");
                break;
            }
        }
    }
    #[test]
    fn saturated_capture_keeps_sparse_stream_and_control_progress() {
        let root = tempfile::tempdir().unwrap();
        let (worker, _) = Worker::start(root.path()).unwrap();
        let a = uuid::Uuid::new_v4().to_string();
        let b = uuid::Uuid::new_v4().to_string();
        saved(&worker, &a);
        saved(&worker, &b);
        let ca = worker.capture_for(&a).unwrap();
        let cb = worker.capture_for(&b).unwrap();
        std::fs::write(root.path().join("proof-storage-session"), &a).unwrap();
        std::fs::write(root.path().join("proof-storage-write"), "error").unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let drain = stop.clone();
        let id = a.clone();
        let producer = ca.clone();
        let reader = thread::spawn(move || {
            for _ in 0..66 {
                producer.accept(4096);
                if let Some(retained) = producer
                    .send((id.clone(), vec![b'A'; 4096], Instant::now()), Some(&drain))
                    .unwrap()
                {
                    return retained;
                }
            }
            panic!("faulted bounded capture did not saturate")
        });
        wait_for(|| {
            let p = worker.progress_for(Some(&a));
            p.accepted == 270336 && p.producer_wait.is_some()
        });
        let saturated = worker.progress_for(Some(&a));
        assert_eq!(
            (
                saturated.written,
                saturated.durable,
                saturated.pending_bytes
            ),
            (0, 0, 270336)
        );
        cb.accept(5);
        cb.send((b.clone(), b"sparse"[..5].to_vec(), Instant::now()), None)
            .unwrap();
        let operation = final_operation(&b);
        worker
            .commands
            .send(Command::Flush(operation.clone()))
            .unwrap();
        committed(&worker, &operation);
        assert_eq!(worker.progress_for(Some(&b)).durable, 5);
        assert!(worker.progress_for(Some(&a)).error.is_some());
        stop.store(true, Ordering::Release);
        let retained = reader.join().unwrap();
        assert_eq!(retained.1.len(), 4096);
        std::fs::remove_file(root.path().join("proof-storage-write")).unwrap();
        ca.send(retained, None).unwrap();
        let operation = final_operation(&a);
        worker
            .commands
            .send(Command::Flush(operation.clone()))
            .unwrap();
        committed(&worker, &operation);
        let done = worker.progress_for(Some(&a));
        assert_eq!(
            (
                done.accepted,
                done.written,
                done.durable,
                done.pending_bytes
            ),
            (270336, 270336, 270336, 0)
        );
        let mut text = String::new();
        let mut offset = 0;
        loop {
            worker
                .commands
                .send(Command::HistorySession(a.clone(), offset))
                .unwrap();
            let page = loop {
                if let Completion::History(page) =
                    worker.events.recv_timeout(Duration::from_secs(5)).unwrap()
                {
                    break page;
                }
            };
            text.push_str(&page.text);
            match page.next {
                Some(next) => offset = next,
                None => break,
            }
        }
        assert_eq!(text, "A".repeat(270336));
    }
    #[test]
    fn restored_session_reports_its_committed_prefix() {
        let root = tempfile::tempdir().unwrap();
        let (worker, _) = Worker::start(root.path()).unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        saved(&worker, &id);
        let capture = worker.capture_for(&id).unwrap();
        capture.accept(3);
        capture
            .send((id.clone(), b"AAA".to_vec(), Instant::now()), None)
            .unwrap();
        let operation = final_operation(&id);
        worker
            .commands
            .send(Command::Flush(operation.clone()))
            .unwrap();
        committed(&worker, &operation);
        drop(capture);
        drop(worker);
        let (_restored_worker, restored) = Worker::start(root.path()).unwrap();
        assert_eq!(restored.state.session_identity.unwrap().session, id);
        assert_eq!(
            (
                restored.state.accepted_bytes,
                restored.state.written_bytes,
                restored.state.durable_bytes,
                restored.state.capture_pending_bytes
            ),
            (3, 3, 3, 0)
        );
    }
    #[test]
    fn live_capture_admission_is_finite_and_seal_releases_the_reservation() {
        let root = tempfile::tempdir().unwrap();
        let (worker, _) = Worker::start(root.path()).unwrap();
        let mut sessions = Vec::new();
        for _ in 0..32 {
            let id = uuid::Uuid::new_v4().to_string();
            saved(&worker, &id);
            let capture = worker.capture_for(&id).unwrap();
            sessions.push((id, capture));
        }
        let next = uuid::Uuid::new_v4().to_string();
        saved(&worker, &next);
        assert!(worker.capture_for(&next).is_err());
        let operation = final_operation(&sessions[0].0);
        worker
            .commands
            .send(Command::Flush(operation.clone()))
            .unwrap();
        committed(&worker, &operation);
        let operation = Operation {
            purpose: Purpose::Seal,
            ..final_operation(&sessions[0].0)
        };
        worker
            .commands
            .send(Command::Seal(operation.clone()))
            .unwrap();
        committed(&worker, &operation);
        let capture = worker.capture_for(&next).unwrap();
        capture.accept(3);
        capture
            .send((next.clone(), b"new".to_vec(), Instant::now()), None)
            .unwrap();
        let operation = final_operation(&next);
        worker
            .commands
            .send(Command::Flush(operation.clone()))
            .unwrap();
        committed(&worker, &operation);
        assert_eq!(worker.progress_for(Some(&next)).durable, 3);
        assert!(
            sessions[0]
                .1
                .send(
                    (sessions[0].0.clone(), b"late".to_vec(), Instant::now()),
                    None
                )
                .is_err()
        );
    }
    #[test]
    fn restore_retains_unselected_uncertain_execution_and_latest_workspace_context() {
        let root = tempfile::tempdir().unwrap();
        let (worker, _) = Worker::start(root.path()).unwrap();
        let task = uuid::Uuid::new_v4().to_string();
        let mut ids = Vec::new();
        let mut states = Vec::new();
        for index in 0..3 {
            let session = uuid::Uuid::new_v4().to_string();
            let state = Observation {
                session_identity: Some(crate::events::SessionIdentity {
                    session: session.clone(),
                    task: if index < 2 {
                        task.clone()
                    } else {
                        uuid::Uuid::new_v4().to_string()
                    },
                    request: format!("restore-{index}"),
                }),
                launch_request: format!("restore-{index}"),
                lifecycle: if index == 0 {
                    Lifecycle::Running
                } else {
                    Lifecycle::Stopped
                },
                execution: if index == 0 {
                    Some(crate::recovery::Execution::prepared().unwrap())
                } else {
                    None
                },
                ..Observation::default()
            };
            worker
                .commands
                .send(Command::Save(Snapshot::new(
                    &state,
                    None,
                    &state.session_identity.as_ref().unwrap().task,
                )))
                .unwrap();
            loop {
                if matches!(
                    worker.events.recv_timeout(Duration::from_secs(5)).unwrap(),
                    Completion::Committed(_)
                ) {
                    break;
                }
            }
            ids.push(session);
            states.push(state);
        }
        let selected_task = states[2].session_identity.as_ref().unwrap().task.clone();
        worker
            .commands
            .send(Command::SelectWorkspace(
                uuid::Uuid::new_v4(),
                selected_task.clone(),
            ))
            .unwrap();
        loop {
            if matches!(
                worker.events.recv_timeout(Duration::from_secs(5)).unwrap(),
                Completion::Selected(_, Ok(_))
            ) {
                break;
            }
        }
        // A late background save must update A's facts while preserving selected B.
        worker
            .commands
            .send(Command::Save(Snapshot::new(&states[0], None, &task)))
            .unwrap();
        loop {
            if matches!(
                worker.events.recv_timeout(Duration::from_secs(5)).unwrap(),
                Completion::Committed(_)
            ) {
                break;
            }
        }
        drop(worker);
        let (_worker, restored) = Worker::start(root.path()).unwrap();
        assert_eq!(restored.task, selected_task);
        assert_eq!(
            restored.state.session_identity.as_ref().unwrap().session,
            ids[2]
        );
        let uncertain = restored
            .sessions
            .iter()
            .find(|saved| saved.state.session_identity.as_ref().unwrap().session == ids[0])
            .unwrap();
        assert_eq!(uncertain.task, task);
        assert_eq!(uncertain.state.lifecycle, Lifecycle::Interrupted);
        assert!(uncertain.state.execution.is_some());
        assert!(
            uncertain.checkout.is_none(),
            "unknown legacy path must remain unknown"
        );
        for id in &ids[1..] {
            assert!(
                restored.sessions.iter().any(|saved| saved
                    .state
                    .session_identity
                    .as_ref()
                    .unwrap()
                    .session
                    == *id)
            );
        }
        assert_eq!(restored.sessions.len(), 3);
    }
}
