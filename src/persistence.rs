//! The supervisor's sole SQLite/file writer. No disk operation runs on the UI thread.
use crate::session::{Lifecycle, Observation};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    os::{
        fd::AsRawFd,
        unix::{ffi::OsStrExt, fs::OpenOptionsExt},
    },
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
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
    pub checkout: String,
    pub lifecycle: Lifecycle,
    pub text: String,
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
    sender: SyncSender<(String, Vec<u8>, Instant)>,
    progress: Arc<Mutex<Progress>>,
}
impl Capture {
    pub fn send(&self, block: (String, Vec<u8>, Instant)) -> std::result::Result<(), ()> {
        let size = block.1.len();
        {
            let mut p = self.progress.lock().unwrap();
            p.accepted += size as u64;
            p.pending_bytes += size;
            p.pending_high_water = p.pending_high_water.max(p.pending_bytes);
        }
        match self.sender.try_send(block) {
            Ok(()) => Ok(()),
            Err(mpsc::TrySendError::Disconnected(_)) => Err(()),
            Err(mpsc::TrySendError::Full(block)) => {
                let since = Instant::now();
                {
                    let mut p = self.progress.lock().unwrap();
                    p.backpressure_count += 1;
                    p.producer_wait = Some(since);
                }
                let result = self.sender.send(block).map_err(|_| ());
                {
                    let mut p = self.progress.lock().unwrap();
                    p.backpressure_micros += since.elapsed().as_micros() as u64;
                    p.producer_wait = None;
                }
                result
            }
        }
    }
}
pub fn metadata(state: &Observation) -> Box<Observation> {
    let mut saved = state.clone();
    saved.screen = String::new();
    saved.formatted = String::new();
    saved.decoded = None;
    saved.history = None;
    saved.cleanup = None;
    saved.cleanup_message = None;
    saved.durable_commits.clear();
    saved.status = crate::native_status::Status::default();
    Box::new(saved)
}
pub enum Command {
    Prepare(Box<Observation>, Option<Vec<u8>>, String),
    Authorize(Box<Observation>, Option<Vec<u8>>, String),
    Save(Box<Observation>, Option<Vec<u8>>, String),
    History(usize, u64),
    Cleanup(usize),
    Delete(String),
    Seal(String),
    Retry,
    Flush,
}
pub enum Completion {
    Retried,
    Sealed,
    Cleanup(crate::cleanup::Page),
    Deleted(String),
    Prepared(String),
    Authorized(String),
    Saved,
    History(HistoryPage),
    Flushed,
    Error(&'static str, String),
}
pub struct Restored {
    pub state: Observation,
    pub checkout: Option<Vec<u8>>,
    pub task: String,
    pub launches: Vec<crate::session::LaunchResult>,
}
pub struct Worker {
    pub commands: SyncSender<Command>,
    pub output: Capture,
    pub events: Receiver<Completion>,
    pub progress: Arc<Mutex<Progress>>,
}
impl Worker {
    pub fn start(directory: &Path) -> Result<(Self, Restored)> {
        let directory = directory.to_owned();
        let (ready_tx, ready) = mpsc::sync_channel(1);
        let (commands, control) = mpsc::sync_channel(16);
        // At most 64 * 4096 bytes queued, plus one reader buffer and one worker buffer.
        let (output, capture) = mpsc::sync_channel::<(String, Vec<u8>, Instant)>(64);
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
            let mut pending: Option<(String, Vec<u8>, Instant)> = None;
            let mut since = Instant::now();
            let mut capacity_since = Instant::now() - Duration::from_secs(1);
            loop {
                // Reserve control capacity independently from capture traffic.
                if let Ok(command) = control.try_recv() {
                    let kind = match &command { Command::Save(..) | Command::Prepare(..) | Command::Authorize(..) => "metadata", Command::History(..) => "history", Command::Cleanup(..) | Command::Delete(..) | Command::Seal(..) => "cleanup", Command::Retry => "retry", Command::Flush => "flush" };
                    status.lock().unwrap().operation = Some((kind, Instant::now()));
                    let result = match command {
                        Command::Cleanup(index) => store.start_cleanup(index).map(|_| Completion::Sealed),
                        Command::Delete(id) => store.start_delete(&id).map(|_| Completion::Sealed),
                        Command::Seal(id) => store.seal(&id).map(|_| Completion::Sealed),
                        Command::Retry => { since = Instant::now() - FLUSH; Ok(Completion::Retried) },
                        Command::Prepare(state, checkout, task) => store.fault("prepare").and_then(|_| store.save(&state, checkout, task)).map(|_| Completion::Prepared(state.launch_request.clone())),
                        Command::Authorize(state, checkout, task) => store.fault("authorize").and_then(|_| store.save(&state, checkout, task)).map(|_| Completion::Authorized(state.launch_request.clone())),
                        Command::Save(state, checkout, task) => store.fault("save").and_then(|_| store.save(&state, checkout, task)).map(|_| Completion::Saved),
                        Command::History(index, offset) => store.history(index, offset).map(Completion::History),
                        Command::Flush => {
                            // Called only after PTY EOF: drain everything already transferred.
                            (|| {
                                loop {
                                    if pending.is_none() { pending = capture.try_recv().ok(); }
                                    let Some((id, bytes, accepted)) = &mut pending else { break; };
                                    let count = store.append(id, bytes, *accepted)?;
                                    { let mut p = status.lock().unwrap(); p.pending_bytes -= count; p.written = store.active.as_ref().map_or(0, |(_, _, offset)| *offset); }
                                    bytes.drain(..count);
                                    if bytes.is_empty() { pending = None; }
                                }
                                store.flush()?;
                                Ok(Completion::Flushed)
                            })()
                        }
                    };
                    status.lock().unwrap().operation = None;
                    store.publish(&mut status.lock().unwrap());
                    let event = result.unwrap_or_else(|e: Box<dyn std::error::Error + Send + Sync>| Completion::Error(kind, e.to_string()));
                    if tx.send(event).is_err() { break; }
                }
                if capacity_since.elapsed() >= Duration::from_secs(1) {
                    status.lock().unwrap().capacity = crate::cleanup::Capacity::sample(&store.directory);
                    capacity_since = Instant::now();
                }
                if store.cleanup_job.is_some() {
                    status.lock().unwrap().operation = Some(("cleanup", Instant::now()));
                    match store.cleanup_step() {
                        Ok(Some(event)) => {
                            if matches!(event, Completion::Deleted(_)) { status.lock().unwrap().capacity = crate::cleanup::Capacity::sample(&store.directory); }
                            store.publish(&mut status.lock().unwrap());
                            if tx.send(event).is_err() { break; }
                        },
                        Ok(None) => {},
                        Err(error) => { store.cleanup_job = None; let _ = tx.send(Completion::Error("cleanup", format!("History cleanup incomplete: {error}; g reopen then explicitly retry selected history"))); }
                    }
                }
                status.lock().unwrap().operation = None;
                if pending.is_none() {
                    match capture.recv_timeout(Duration::from_millis(5)) {
                        Ok(item) => pending = Some(item),
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        Err(_) => {}
                    }
                }
                if let Some((id, bytes, accepted)) = &mut pending {
                    status.lock().unwrap().operation = Some(("write", Instant::now()));
                    match store.append(id, bytes, *accepted) {
                        Ok(count) => { { let mut p = status.lock().unwrap(); p.pending_bytes -= count; p.written = store.active.as_ref().map_or(0, |(_, _, offset)| *offset); } bytes.drain(..count); if bytes.is_empty() { pending = None; } }
                        Err(error) => { status.lock().unwrap().error = Some(format!("History capture blocked: {error}; output retained; repair storage then press y to retry or s to stop")); thread::sleep(Duration::from_millis(100)); },
                    }
                }
                status.lock().unwrap().operation = None;
                if since.elapsed() >= FLUSH {
                    status.lock().unwrap().operation = Some(("sync", Instant::now()));
                    match store.flush() {
                        Ok(()) => { if pending.is_none() { status.lock().unwrap().error = None; } }
                        Err(error) => status.lock().unwrap().error = Some(format!("History sync failed: {error}; quit requires successful flush")),
                    }
                    status.lock().unwrap().operation = None;
                    since = Instant::now();
                }
                store.publish(&mut status.lock().unwrap());
            }
        })?;
        let restored = ready.recv()?.map_err(io::Error::other)?;
        Ok((
            Self {
                commands,
                output: Capture {
                    sender: output,
                    progress: progress.clone(),
                },
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
struct Store {
    cleanup_job: Option<CleanupJob>,
    directory_handle: File,
    directory: PathBuf,
    db: Connection,
    active: Option<(String, File, u64)>,
    total: u64,
    active_dirty: bool,
    oldest: Option<Instant>,
    origin: Instant,
    sync_micros: u64,
    completions: u64,
    completed_millis: u64,
    accepted_to_durable_millis: u64,
    commits: std::collections::VecDeque<DurableCommit>,
}
impl Store {
    fn publish(&self, progress: &mut Progress) {
        progress.commits.clone_from(&self.commits);
        progress.written = self
            .active
            .as_ref()
            .map_or(self.total, |(_, _, offset)| *offset);
        progress.durable = self.total;
        progress.last_flush_micros = self.sync_micros;
        progress.flush_count = self.completions;
        progress.completed_millis = self.completed_millis;
        progress.accepted_to_durable_millis = self.accepted_to_durable_millis;
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
        if version > 3 {
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
        let mut store = Self {
            cleanup_job: None,
            directory_handle,
            directory,
            db,
            active: None,
            total: 0,
            active_dirty: false,
            oldest: None,
            origin: Instant::now(),
            sync_micros: 0,
            completions: 0,
            completed_millis: 0,
            accepted_to_durable_millis: 0,
            commits: std::collections::VecDeque::with_capacity(1024),
        };
        // Only a previously committed explicit intent authorizes restart work.
        store.resume_delete()?;
        Ok(store)
    }
    fn seal(&mut self, id: &str) -> Result<()> {
        let json: String =
            self.db
                .query_row("SELECT state FROM sessions WHERE id=?1", [id], |r| r.get(0))?;
        let state: Observation = serde_json::from_str(&json)?;
        if state.execution.is_some()
            || !matches!(state.lifecycle, Lifecycle::Stopped | Lifecycle::Exited)
        {
            return Err("History still has active or uncertain execution".into());
        }
        if self
            .active
            .as_ref()
            .is_some_and(|(active, _, _)| active == id)
        {
            if self.oldest.is_some() {
                return Err("History final flush still pending".into());
            }
            self.active = None;
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
            let state: Observation = serde_json::from_str(&row.get::<_, String>(2)?)?;
            let sealed: bool = row.get(4)?;
            let history_state: String = row.get(5)?;
            let eligible = sealed
                && state.execution.is_none()
                && matches!(state.lifecycle, Lifecycle::Stopped | Lifecycle::Exited);
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
        let state: Observation = serde_json::from_str(&json)?;
        if history == "deleted" {
            return Err("History already deleted".into());
        }
        if !sealed
            || state.execution.is_some()
            || !matches!(state.lifecycle, Lifecycle::Stopped | Lifecycle::Exited)
        {
            return Err("History deletion refused: active, uncertain or unflushed session".into());
        }
        // The accepted EOF/final-flush seal closed any retained file handle.
        if self
            .active
            .as_ref()
            .is_some_and(|(active, _, _)| active == id)
            || self.oldest.is_some()
                && self
                    .active
                    .as_ref()
                    .is_some_and(|(active, _, _)| active == id)
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
                        let current: Observation = serde_json::from_str(&current)?;
                        if current
                            .identity
                            .as_deref()
                            .and_then(|identity| session_id(identity).ok())
                            .as_deref()
                            == Some(&id)
                        {
                            self.total = 0;
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
        let launches = self
            .db
            .prepare("SELECT result FROM requests LIMIT 1024")?
            .query_map([], |r| r.get::<_, String>(0))?
            .map(|row| Ok(serde_json::from_str(&row?)?))
            .collect::<Result<Vec<_>>>()?;
        let row: Option<(String, Option<Vec<u8>>, String)> = self
            .db
            .query_row("SELECT state,checkout,task FROM app WHERE id=1", [], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .optional()?;
        let Some((json, path, task)) = row else {
            return Ok(Restored {
                state: Observation::default(),
                checkout: None,
                task: uuid::Uuid::new_v4().to_string(),
                launches,
            });
        };
        let mut state: Observation = serde_json::from_str(&json)?;
        state.stopped = false;
        state.status = crate::native_status::Status::default();
        state.metadata_pending = false;
        if let Some(identity) = &state.identity {
            self.total = self.db.query_row(
                "SELECT durable FROM sessions WHERE id=?1",
                [session_id(identity)?],
                |r| r.get(0),
            )?;
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
        if state.execution.is_some()
            && !launches
                .iter()
                .any(|result: &crate::session::LaunchResult| result.request == state.launch_request)
        {
            state.launch_result = Some(crate::session::LaunchResult { request: state.launch_request.clone(), identity: state.identity.clone(), error: Some("Interrupted launch: native execution outcome uncertain; reconcile explicitly with u. No automatic restart.".into()) });
            self.save(&state, path.clone(), task.clone())?;
        }
        let mut launches = launches;
        if let Some(result) = &state.launch_result
            && !launches.iter().any(|known| known.request == result.request)
        {
            launches.push(result.clone());
        }
        Ok(Restored {
            state,
            checkout: path,
            task,
            launches,
        })
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
    fn partial_fault(&self) -> Option<usize> {
        #[cfg(debug_assertions)]
        return std::fs::read_to_string(self.directory.join("proof-storage-partial"))
            .ok()
            .and_then(|value| value.trim().parse().ok());
        #[cfg(not(debug_assertions))]
        None
    }
    fn save(&mut self, state: &Observation, path: Option<Vec<u8>>, task: String) -> Result<()> {
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
        tx.execute("INSERT INTO app VALUES(1,?1,?2,?3) ON CONFLICT(id) DO UPDATE SET state=excluded.state,checkout=excluded.checkout,task=excluded.task", params![json,path,task])?;
        if let Some(identity) = state.identity.as_ref() {
            tx.execute("INSERT INTO sessions(id,identity,state,created) VALUES(?1,?2,?3,unixepoch()) ON CONFLICT(id) DO UPDATE SET state=excluded.state", params![session_id(identity)?,identity,json])?;
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
        if self
            .active
            .as_ref()
            .is_some_and(|(active, _, offset)| active != id || *offset > 0 && *offset % CHUNK == 0)
        {
            self.flush()?;
            self.active = None;
        }
        if self.active.is_none() {
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
            self.fault("directory-sync")?;
            File::open(&self.directory)?.sync_all()?;
            self.active = Some((id.to_owned(), file, offset));
        }
        self.oldest.get_or_insert(accepted);
        let offset = self.active.as_ref().unwrap().2;
        let count = bytes.len().min((CHUNK - offset % CHUNK) as usize);
        self.fault("write")?;
        let partial = self.partial_fault();
        let (_, file, offset) = self.active.as_mut().unwrap();
        file.seek(SeekFrom::Start(*offset % CHUNK))?;
        if let Some(prefix) = partial {
            file.write_all(&bytes[..count.min(prefix)])?;
            return Err(io::Error::from_raw_os_error(libc::ENOSPC).into());
        }
        file.write_all(&bytes[..count])?;
        *offset += count as u64;
        self.active_dirty = true;
        Ok(count)
    }
    fn flush(&mut self) -> Result<()> {
        if !self.active_dirty || self.oldest.is_none() {
            return Ok(());
        }
        self.fault("sync")?;
        if let Some((id, file, offset)) = &self.active {
            let started = Instant::now();
            file.sync_all()?;
            let tx = self.db.transaction()?;
            tx.execute(
                "UPDATE sessions SET durable=?2 WHERE id=?1",
                params![id, offset],
            )?;
            tx.commit()?;
            self.total = *offset;
            self.active_dirty = false;
            self.sync_micros = started.elapsed().as_micros() as u64;
            self.completions += 1;
            self.completed_millis = self.origin.elapsed().as_millis() as u64;
            let age = self.oldest.take().unwrap().elapsed();
            self.accepted_to_durable_millis = age.as_millis() as u64;
            if self.commits.len() == 1024 {
                self.commits.pop_front();
            }
            self.commits.push_back(DurableCommit {
                sequence: self.completions,
                prefix: self.total,
                completed_micros: self.origin.elapsed().as_micros() as u64,
                sync_commit_micros: self.sync_micros,
                oldest_accepted_micros: age.as_micros() as u64,
            });
        }
        Ok(())
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
        let state: Observation = serde_json::from_str(&json)?;
        Ok(HistoryPage {
            identity,
            checkout: state.adopted.unwrap_or_default(),
            lifecycle: state.lifecycle,
            text,
            offset,
            next: (end < durable).then_some(end),
        })
    }
}
pub fn session_id(identity: &str) -> Result<String> {
    let id = identity
        .split_whitespace()
        .find_map(|part| part.strip_prefix("session="))
        .ok_or("Missing session identity")?;
    Ok(uuid::Uuid::parse_str(id)?.to_string())
}
