//! The supervisor's sole SQLite/file writer. No disk operation runs on the UI thread.
use crate::session::{Lifecycle, Observation};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    os::unix::fs::OpenOptionsExt,
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
#[derive(Clone, Debug, Default)]
pub struct Progress {
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
            p.pending_bytes += size;
            p.pending_high_water = p.pending_high_water.max(p.pending_bytes);
        }
        match self.sender.try_send(block) {
            Ok(()) => Ok(()),
            Err(mpsc::TrySendError::Disconnected(_)) => Err(()),
            Err(mpsc::TrySendError::Full(block)) => {
                let since = Instant::now();
                self.progress.lock().unwrap().backpressure_count += 1;
                let result = self.sender.send(block).map_err(|_| ());
                self.progress.lock().unwrap().backpressure_micros +=
                    since.elapsed().as_micros() as u64;
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
    saved.status = crate::native_status::Status::default();
    Box::new(saved)
}
pub enum Command {
    Save(Box<Observation>, Option<Vec<u8>>, String),
    History(usize, u64),
    Flush,
}
pub enum Completion {
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
            loop {
                // Reserve control capacity independently from capture traffic.
                if let Ok(command) = control.try_recv() {
                    let kind = match &command { Command::Save(..) => "metadata", Command::History(..) => "history", Command::Flush => "flush" };
                    let result = match command {
                        Command::Save(state, checkout, task) => store.save(&state, checkout, task).map(|_| Completion::Saved),
                        Command::History(index, offset) => store.history(index, offset).map(Completion::History),
                        Command::Flush => {
                            // Called only after PTY EOF: drain everything already transferred.
                            (|| {
                                loop {
                                    if pending.is_none() { pending = capture.try_recv().ok(); }
                                    let Some((id, bytes, accepted)) = &mut pending else { break; };
                                    let count = store.append(id, bytes, *accepted)?;
                                    status.lock().unwrap().pending_bytes -= count;
                                    bytes.drain(..count);
                                    if bytes.is_empty() { pending = None; }
                                }
                                store.flush()?;
                                Ok(Completion::Flushed)
                            })()
                        }
                    };
                    let event = result.unwrap_or_else(|e: Box<dyn std::error::Error + Send + Sync>| Completion::Error(kind, e.to_string()));
                    if tx.send(event).is_err() { break; }
                }
                if pending.is_none() {
                    match capture.recv_timeout(Duration::from_millis(5)) {
                        Ok(item) => pending = Some(item),
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        Err(_) => {}
                    }
                }
                if let Some((id, bytes, accepted)) = &mut pending {
                    match store.append(id, bytes, *accepted) {
                        Ok(count) => { status.lock().unwrap().pending_bytes -= count; bytes.drain(..count); if bytes.is_empty() { pending = None; } }
                        Err(error) => status.lock().unwrap().error = Some(format!("History capture blocked: {error}; output retained with backpressure")),
                    }
                }
                if since.elapsed() >= FLUSH {
                    match store.flush() {
                        Ok(()) => { if pending.is_none() { status.lock().unwrap().error = None; } }
                        Err(error) => status.lock().unwrap().error = Some(format!("History sync failed: {error}; quit requires successful flush")),
                    }
                    since = Instant::now();
                }
                let mut p = status.lock().unwrap();
                p.durable = store.total;
                p.last_flush_micros = store.sync_micros;
                p.flush_count = store.completions;
                p.completed_millis = store.completed_millis;
                p.accepted_to_durable_millis = store.accepted_to_durable_millis;
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
struct Store {
    directory: PathBuf,
    db: Connection,
    active: Option<(String, File, u64)>,
    total: u64,
    oldest: Option<Instant>,
    origin: Instant,
    sync_micros: u64,
    completions: u64,
    completed_millis: u64,
    accepted_to_durable_millis: u64,
}
impl Store {
    fn open(directory: PathBuf) -> Result<Self> {
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
        if version > 2 {
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
        Ok(Self {
            directory,
            db,
            active: None,
            total: 0,
            oldest: None,
            origin: Instant::now(),
            sync_micros: 0,
            completions: 0,
            completed_millis: 0,
            accepted_to_durable_millis: 0,
        })
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
        if matches!(
            state.lifecycle,
            Lifecycle::Running | Lifecycle::Stopping | Lifecycle::StopFailed
        ) {
            state.lifecycle = Lifecycle::Interrupted;
            state.error = Some("Interrupted supervisor: retained history available; execution is uncertain. No automatic restart.".into());
        }
        Ok(Restored {
            state,
            checkout: path,
            task,
            launches,
        })
    }
    fn save(&mut self, state: &Observation, path: Option<Vec<u8>>, task: String) -> Result<()> {
        let mut state = state.clone();
        state.screen.clear();
        state.formatted.clear();
        state.decoded = None;
        state.history = None;
        state.error = None;
        let json = serde_json::to_string(&state)?;
        let tx = self.db.transaction()?;
        tx.execute("INSERT INTO app VALUES(1,?1,?2,?3) ON CONFLICT(id) DO UPDATE SET state=excluded.state,checkout=excluded.checkout,task=excluded.task", params![json,path,task])?;
        if let Some(identity) = state.identity.as_ref() {
            tx.execute("INSERT INTO sessions(id,identity,state) VALUES(?1,?2,?3) ON CONFLICT(id) DO UPDATE SET state=excluded.state", params![session_id(identity)?,identity,json])?;
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
            let existed = path.exists();
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
            if !existed {
                File::open(&self.directory)?.sync_all()?;
            }
            self.active = Some((id.to_owned(), file, offset));
        }
        self.oldest.get_or_insert(accepted);
        let offset = self.active.as_ref().unwrap().2;
        let count = bytes.len().min((CHUNK - offset % CHUNK) as usize);
        let (_, file, offset) = self.active.as_mut().unwrap();
        file.seek(SeekFrom::Start(*offset % CHUNK))?;
        file.write_all(&bytes[..count])?;
        *offset += count as u64;
        Ok(count)
    }
    fn flush(&mut self) -> Result<()> {
        if self.oldest.is_none() {
            return Ok(());
        }
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
            self.sync_micros = started.elapsed().as_micros() as u64;
            self.completions += 1;
            self.completed_millis = self.origin.elapsed().as_millis() as u64;
            self.accepted_to_durable_millis =
                self.oldest.take().unwrap().elapsed().as_millis() as u64;
        }
        Ok(())
    }
    fn history(&self, index: usize, offset: u64) -> Result<HistoryPage> {
        let (id, identity, durable, json): (String, String, u64, String) = self
            .db
            .query_row(
                "SELECT id,identity,durable,state FROM sessions ORDER BY rowid DESC LIMIT 1 OFFSET ?1",
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
            let mut file = File::open(self.file(&id, position / CHUNK))?;
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
