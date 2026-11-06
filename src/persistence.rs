//! The supervisor's sole SQLite/file writer. No disk operation runs on the UI thread.
use crate::session::{Lifecycle, Observation};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, mpsc::{self, Receiver, SyncSender}},
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
    pub text: String,
    pub offset: u64,
    pub next: Option<u64>,
}
#[derive(Clone, Debug, Default)]
pub struct Progress {
    pub durable: u64,
    pub error: Option<String>,
    pub last_flush_micros: u64,
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
    Error(String),
}
pub struct Worker {
    pub commands: SyncSender<Command>,
    pub output: SyncSender<(String, Vec<u8>)>,
    pub events: Receiver<Completion>,
    pub progress: Arc<Mutex<Progress>>,
}
impl Worker {
    pub fn start(directory: &Path) -> Result<(Self, Observation, Option<Vec<u8>>, String)> {
        let directory = directory.to_owned();
        let (ready_tx, ready) = mpsc::sync_channel(1);
        let (commands, control) = mpsc::sync_channel(16);
        // At most 64 * 4096 bytes queued, plus one reader buffer and one worker buffer.
        let (output, capture) = mpsc::sync_channel::<(String, Vec<u8>)>(64);
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
            let mut pending: Option<(String, Vec<u8>)> = None;
            let mut since = Instant::now();
            loop {
                // Reserve control capacity independently from capture traffic.
                if let Ok(command) = control.try_recv() {
                    let result = match command {
                        Command::Save(state, checkout, task) => store.save(&state, checkout, task).map(|_| Completion::Saved),
                        Command::History(index, offset) => store.history(index, offset).map(Completion::History),
                        Command::Flush => {
                            // Called only after PTY EOF: drain everything already transferred.
                            let result = (|| {
                                if let Some((id, bytes)) = pending.take() { store.append(&id, &bytes)?; }
                                while let Ok((id, bytes)) = capture.try_recv() { store.append(&id, &bytes)?; }
                                store.flush()?;
                                Ok(Completion::Flushed)
                            })();
                            result
                        }
                    };
                    let event = result.unwrap_or_else(|e: Box<dyn std::error::Error + Send + Sync>| Completion::Error(e.to_string()));
                    if tx.send(event).is_err() { break; }
                }
                if pending.is_none() {
                    match capture.recv_timeout(Duration::from_millis(5)) {
                        Ok(item) => pending = Some(item),
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        Err(_) => {}
                    }
                }
                if let Some((id, bytes)) = &pending {
                    match store.append(id, bytes) {
                        Ok(()) => { pending = None; }
                        Err(error) => status.lock().unwrap().error = Some(format!("History capture blocked: {error}; output retained with backpressure")),
                    }
                }
                if since.elapsed() >= FLUSH {
                    let started = Instant::now();
                    match store.flush() {
                        Ok(()) => {
                            let mut p = status.lock().unwrap();
                            p.durable = store.total;
                            p.last_flush_micros = started.elapsed().as_micros() as u64;
                            if pending.is_none() { p.error = None; }
                        }
                        Err(error) => status.lock().unwrap().error = Some(format!("History sync failed: {error}; quit requires successful flush")),
                    }
                    since = Instant::now();
                }
            }
        })?;
        let (state, checkout, task) = ready.recv()?.map_err(io::Error::other)?;
        Ok((Self { commands, output, events, progress }, state, checkout, task))
    }
}
struct Store {
    directory: PathBuf,
    db: Connection,
    active: Option<(String, File, u64)>,
    total: u64,
}
impl Store {
    fn open(directory: PathBuf) -> Result<Self> {
        let path = directory.join("history.sqlite3");
        // Create privately before SQLite opens it; the parent is already validated.
        OpenOptions::new().create(true).truncate(false).write(true).mode(0o600).custom_flags(libc::O_NOFOLLOW).open(&path)?;
        let mut db = Connection::open(&path)?;
        db.execute_batch("PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL; PRAGMA fullfsync=ON;")?;
        let version: u32 = db.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version > 1 { return Err("History schema is newer than this executable".into()); }
        if version == 0 {
            let tx = db.transaction()?;
            tx.execute_batch("CREATE TABLE IF NOT EXISTS app (id INTEGER PRIMARY KEY CHECK(id=1), state TEXT NOT NULL, checkout BLOB, task TEXT NOT NULL);
                CREATE TABLE IF NOT EXISTS sessions (id TEXT PRIMARY KEY, identity TEXT NOT NULL, state TEXT NOT NULL, durable INTEGER NOT NULL DEFAULT 0);
                PRAGMA user_version=1;")?;
            tx.commit()?;
            File::open(&directory)?.sync_all()?;
        }
        Ok(Self { directory, db, active: None, total: 0 })
    }
    fn restore(&self) -> Result<(Observation, Option<Vec<u8>>, String)> {
        let row: Option<(String, Option<Vec<u8>>, String)> = self.db.query_row("SELECT state,checkout,task FROM app WHERE id=1", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).optional()?;
        let Some((json, path, task)) = row else { return Ok((Observation::default(), None, uuid::Uuid::new_v4().to_string())); };
        let mut state: Observation = serde_json::from_str(&json)?;
        state.stopped = false;
        if matches!(state.lifecycle, Lifecycle::Running | Lifecycle::Stopping | Lifecycle::StopFailed) {
            state.lifecycle = Lifecycle::Interrupted;
            state.error = Some("Interrupted supervisor: retained history available; execution is uncertain. No automatic restart.".into());
        }
        Ok((state, path, task))
    }
    fn save(&mut self, state: &Observation, path: Option<Vec<u8>>, task: String) -> Result<()> {
        let mut state = state.clone();
        state.screen.clear();
        state.history = None;
        state.error = None;
        let json = serde_json::to_string(&state)?;
        let tx = self.db.transaction()?;
        tx.execute("INSERT INTO app VALUES(1,?1,?2,?3) ON CONFLICT(id) DO UPDATE SET state=excluded.state,checkout=excluded.checkout,task=excluded.task", params![json,path,task])?;
        if let Some(identity) = state.identity.as_ref() {
            tx.execute("INSERT INTO sessions(id,identity,state) VALUES(?1,?2,?3) ON CONFLICT(id) DO UPDATE SET state=excluded.state", params![session_id(identity)?,identity,json])?;
        }
        tx.commit()?;
        Ok(())
    }
    fn file(&self, id: &str, chunk: u64) -> PathBuf { self.directory.join(format!("{id}-{chunk:08}.transcript")) }
    fn append(&mut self, id: &str, bytes: &[u8]) -> Result<()> {
        if bytes.len() > 4096 { return Err("Capture block exceeds 4096 bytes".into()); }
        if self.active.as_ref().is_some_and(|(active,_,_)| active != id) { self.flush()?; self.active = None; }
        if self.active.is_none() {
            let offset: u64 = self.db.query_row("SELECT durable FROM sessions WHERE id=?1", [id], |r| r.get(0))?;
            let path = self.file(id, offset / CHUNK);
            let existed = path.exists();
            let file = OpenOptions::new().create(true).truncate(false).read(true).write(true).mode(0o600).custom_flags(libc::O_NOFOLLOW).open(path)?;
            // Uncommitted tails are preserved outside the durable prefix; never count them.
            if file.metadata()?.len() != offset % CHUNK { return Err("Uncommitted transcript tail requires explicit recovery".into()); }
            if !existed { File::open(&self.directory)?.sync_all()?; }
            self.active = Some((id.to_owned(), file, offset));
        }
        let offset = self.active.as_ref().unwrap().2;
        if offset % CHUNK + bytes.len() as u64 > CHUNK {
            // A capture block may straddle the fixed chunk boundary.
            let split = (CHUNK - offset % CHUNK) as usize;
            self.append(id, &bytes[..split])?;
            self.append(id, &bytes[split..])?;
            return Ok(());
        }
        let (_,file,offset) = self.active.as_mut().unwrap();
        file.seek(SeekFrom::Start(*offset % CHUNK))?;
        file.write_all(bytes)?;
        *offset += bytes.len() as u64;
        if *offset % CHUNK == 0 { self.flush()?; self.active = None; }
        Ok(())
    }
    fn flush(&mut self) -> Result<()> {
        if let Some((id,file,offset)) = &self.active {
            file.sync_all()?;
            let tx = self.db.transaction()?;
            tx.execute("UPDATE sessions SET durable=?2 WHERE id=?1", params![id,offset])?;
            tx.commit()?;
            self.total = *offset;
        }
        Ok(())
    }
    fn history(&self, index: usize, offset: u64) -> Result<HistoryPage> {
        let (id, identity, durable): (String,String,u64) = self.db.query_row("SELECT id,identity,durable FROM sessions ORDER BY rowid DESC LIMIT 1 OFFSET ?1", [index], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?.ok_or("No retained session at this position")?;
        let offset = offset.min(durable);
        let end = (offset + PAGE).min(durable);
        let mut raw = Vec::with_capacity(PAGE as usize);
        let mut position = offset;
        while position < end {
            let mut file = File::open(self.file(&id, position / CHUNK))?;
            file.seek(SeekFrom::Start(position % CHUNK))?;
            let length = (end-position).min(CHUNK-position%CHUNK);
            let mut bytes = vec![0;length as usize];
            file.read_exact(&mut bytes)?;
            raw.extend(bytes);
            position += length;
        }
        let text = String::from_utf8_lossy(&raw).chars().flat_map(|ch| {
            if ch == '\n' || !ch.is_control() { vec![ch] } else { ch.escape_default().collect() }
        }).collect();
        Ok(HistoryPage { identity, text, offset, next: (end < durable).then_some(end) })
    }
}
pub fn session_id(identity: &str) -> Result<String> {
    let id = identity.split_whitespace().find_map(|part| part.strip_prefix("session=")).ok_or("Missing session identity")?;
    Ok(uuid::Uuid::parse_str(id)?.to_string())
}
