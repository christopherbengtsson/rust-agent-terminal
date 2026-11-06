//! Thin scripted-session proof. All blocking transport and PTY operations live here.
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs::{self, File, OpenOptions},
    io::{self, BufRead, BufReader, Read, Write},
    os::unix::{
        ffi::{OsStrExt, OsStringExt},
        fs::{DirBuilderExt, FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt},
        net::{UnixListener, UnixStream},
        process::CommandExt,
    },
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, SyncSender},
    },
    thread,
    time::{Duration, Instant},
};

pub const ROWS: u16 = 24;
pub const COLS: u16 = 100;
pub const MAX_ROWS: u16 = 80;
pub const MAX_COLS: u16 = 200;
const MAX_FRAME: usize = 4 * 1024 * 1024;
#[derive(Debug, Serialize, Deserialize)]
pub enum Request {
    Adopt(Vec<u8>),
    Launch(String),
    LaunchClaude(String),
    LaunchCodex(String),
    Resume(String),
    QueryLaunch(String),
    Reconcile,
    History { index: usize, offset: u64 },
    Input(Vec<u8>),
    Key(crossterm::event::KeyEvent),
    Paste(String),
    Screen,
    Stop,
    Resize { rows: u16, cols: u16 },
    Shutdown,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Lifecycle {
    #[default]
    Idle,
    Running,
    Stopping,
    Exited,
    Stopped,
    StopFailed,
    Interrupted,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LaunchResult {
    pub request: String,
    pub identity: Option<String>,
    pub error: Option<String>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Observation {
    #[serde(default)]
    pub connection_lost: bool,
    #[serde(default)]
    pub capture_pending_bytes: usize,
    #[serde(default)]
    pub capture_high_water: usize,
    #[serde(default)]
    pub capture_backpressure_count: u64,
    #[serde(default)]
    pub capture_backpressure_micros: u64,
    #[serde(default)]
    pub subscriber_frame_high_water: usize,
    #[serde(default)]
    pub execution: Option<crate::recovery::Execution>,
    #[serde(default)]
    pub reconciled: bool,
    #[serde(default)]
    pub history: Option<crate::persistence::HistoryPage>,
    #[serde(default)]
    pub history_index: usize,
    #[serde(default)]
    /// Durable raw prefix for the current session; output_bytes is owner-lifetime.
    /// Baseline output_bytes at each launch before comparing these counters.
    pub durable_bytes: u64,
    #[serde(default)]
    pub metadata_pending: bool,
    #[serde(default)]
    pub durable_flush_count: u64,
    #[serde(default)]
    pub durable_completed_millis: u64,
    #[serde(default)]
    pub sync_commit_micros: u64,
    #[serde(default)]
    pub accepted_to_durable_millis: u64,
    #[serde(default)]
    pub launch_request: String,
    #[serde(default)]
    pub launch_result: Option<LaunchResult>,
    pub adopted: Option<String>,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub native_conversation: Option<String>,
    pub identity: Option<String>,
    #[serde(default)]
    pub status: crate::native_status::Status,
    pub screen: String,
    pub formatted: String,
    pub rows: u16,
    pub cols: u16,
    /// All bytes through this boundary have been applied exactly once to the screen.
    pub output_bytes: u64,
    #[serde(skip)]
    pub decoded: Option<vt100::Screen>,
    pub error: Option<String>,
    pub stopped: bool,
    #[serde(default)]
    pub lifecycle: Lifecycle,
    pub control_denied: bool,
}
pub struct Client {
    pub requests: SyncSender<Request>,
    pub events: Receiver<Observation>,
}
impl Client {
    pub fn start(directory: PathBuf) -> io::Result<Self> {
        let (requests, rx) = mpsc::sync_channel(32);
        let (tx, events) = mpsc::sync_channel(1);
        thread::Builder::new()
            .name("session-transport".into())
            .spawn(move || {
                if let Err(error) = transport(directory, rx, &tx) {
                    let _ = tx.send(Observation {
                        connection_lost: true,
                        error: Some(format!("{error}. Dashboard disconnected; press d to detach locally, then reopen. Process state unknown; stop unconfirmed.")),
                        ..Observation::default()
                    });
                }
            })?;
        Ok(Self { requests, events })
    }
    pub fn send(&self, request: Request) -> Result<(), String> {
        self.requests
            .try_send(request)
            .map_err(|e| format!("Session queue unavailable: {e}; retry the action"))
    }
}
fn transport(
    directory: PathBuf,
    requests: Receiver<Request>,
    events: &SyncSender<Observation>,
) -> Result<(), Box<dyn std::error::Error>> {
    private_directory(&directory)?;
    let socket = directory.join("supervisor.sock");
    // Every startup participates in election, including recovery of a stale socket.
    let mut supervisor = Command::new(std::env::current_exe()?);
    // SAFETY: the child only calls the async-signal-safe setsid before exec.
    unsafe {
        supervisor.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut owner = supervisor
        .arg("--supervisor")
        .arg(&directory)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    thread::spawn(move || {
        let _ = owner.wait();
    });
    let mut connection = None;
    for _ in 0..100 {
        match UnixStream::connect(&socket) {
            Ok(stream) => {
                connection = Some(stream);
                break;
            }
            Err(_) => thread::sleep(Duration::from_millis(20)),
        }
    }
    let mut stream = connection
        .ok_or("Cannot acquire supervisor; ownership unresolved. Verify the private state directory; do not remove a live endpoint")?;
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;
    stream.set_write_timeout(Some(Duration::from_secs(3)))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut greeting: Observation = serde_json::from_slice(&read_frame(&mut reader)?)?;
    decode_screen(&mut greeting);
    if greeting.control_denied {
        events.send(greeting)?;
        return Ok(());
    }
    events.send(greeting)?;
    loop {
        let request = match requests.recv_timeout(Duration::from_millis(40)) {
            Ok(request) => request,
            Err(mpsc::RecvTimeoutError::Timeout) => Request::Screen,
            Err(_) => return Ok(()),
        };
        let reliable = !matches!(request, Request::Screen);
        serde_json::to_writer(&mut stream, &request)?;
        stream.write_all(b"\n")?;
        let mut observation: Observation = serde_json::from_slice(&read_frame(&mut reader)?)?;
        decode_screen(&mut observation);
        if reliable || observation.stopped {
            let stopped = observation.stopped;
            events.send(observation)?;
            if stopped {
                return Ok(());
            }
            continue;
        }
        // A lagging UI receives the next coherent snapshot, never a growing output queue.
        match events.try_send(observation) {
            Ok(()) | Err(mpsc::TrySendError::Full(_)) => {}
            Err(_) => return Ok(()),
        }
    }
}

fn decode_screen(observation: &mut Observation) {
    if observation.rows == 0 || observation.cols == 0 {
        return;
    }
    let mut parser = vt100::Parser::new(
        observation.rows.clamp(1, MAX_ROWS).try_into().unwrap(),
        observation.cols.clamp(1, MAX_COLS).try_into().unwrap(),
        0,
    );
    parser.process(observation.formatted.as_bytes());
    observation.decoded = Some(parser.screen().clone());
}

struct ScreenState {
    parser: vt100::Parser,
    output_bytes: u64,
}
impl ScreenState {
    fn snapshot(&self, state: &mut Observation) {
        let screen = self.parser.screen();
        state.screen = screen.contents();
        state.formatted = screen.state_formatted();
        let (rows, cols) = screen.size();
        state.rows = rows.get();
        state.cols = cols.get();
        state.output_bytes = self.output_bytes;
    }
}

fn read_frame(reader: &mut impl BufRead) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .take((MAX_FRAME + 1) as u64)
        .read_until(b'\n', &mut bytes)?;
    if bytes.len() > MAX_FRAME || bytes.last() != Some(&b'\n') {
        return Err(io::Error::other("Invalid/oversized supervisor frame"));
    }
    Ok(bytes)
}
pub fn path_bytes(path: &Path) -> Vec<u8> {
    path.as_os_str().as_bytes().to_vec()
}

// A directory supplied by another user or through a symlink is never repaired.
fn private_directory(directory: &Path) -> io::Result<()> {
    if !directory.exists() {
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(directory)?;
    }
    let metadata = fs::symlink_metadata(directory)?;
    // SAFETY: geteuid has no preconditions and does not access Rust memory.
    let uid = unsafe { libc::geteuid() };
    if !metadata.is_dir() || metadata.uid() != uid || metadata.mode() & 0o077 != 0 {
        return Err(io::Error::other(
            "Supervisor state must be a user-owned private directory (0700), without a symlink",
        ));
    }
    Ok(())
}

fn elect_owner(directory: &Path) -> io::Result<Option<File>> {
    private_directory(directory)?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(directory.join("supervisor.lock"))?;
    let metadata = lock.metadata()?;
    // SAFETY: geteuid has no preconditions.
    if !metadata.is_file()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o077 != 0
    {
        return Err(io::Error::other("Supervisor ownership lock is not private"));
    }
    match lock.try_lock() {
        Ok(()) => Ok(Some(lock)),
        Err(std::fs::TryLockError::WouldBlock) => Ok(None),
        Err(error) => Err(io::Error::other(error)),
    }
}

fn send_observation(stream: &mut UnixStream, state: &Observation) -> io::Result<()> {
    serde_json::to_writer(&mut *stream, state)?;
    stream.write_all(b"\n")
}

// Exactly one finite response in flight. Partial writes retain their cursor;
// the supervisor never waits for a subscriber and never starts a second frame
// before finishing the first. A stalled subscriber must reconnect to resync.
struct Delivery {
    bytes: Vec<u8>,
    offset: usize,
    since: Instant,
    stopped: bool,
}
impl Delivery {
    fn new(state: &mut Observation) -> io::Result<Self> {
        let mut bytes = serde_json::to_vec(state)?;
        bytes.push(b'\n');
        if bytes.len() > MAX_FRAME {
            return Err(io::Error::other("Supervisor snapshot exceeds frame cap"));
        }
        state.subscriber_frame_high_water = state.subscriber_frame_high_water.max(bytes.len());
        Ok(Self {
            bytes,
            offset: 0,
            since: Instant::now(),
            stopped: state.stopped,
        })
    }
    fn advance(&mut self, stream: &mut UnixStream) -> io::Result<bool> {
        if self.since.elapsed() >= Duration::from_millis(500) {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Slow dashboard disconnected after 500ms; reopen to resynchronize the current screen; history capture continues",
            ));
        }
        // Bound transport work per supervisor pass as well as retained bytes.
        let end = (self.offset + 65536).min(self.bytes.len());
        match stream.write(&self.bytes[self.offset..end]) {
            Ok(0) => return Err(io::Error::new(io::ErrorKind::WriteZero, "Dashboard closed")),
            Ok(n) => self.offset += n,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(error) => return Err(error),
        }
        Ok(self.offset == self.bytes.len())
    }
}

pub fn supervisor(directory: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let Some(_ownership) = elect_owner(directory)? else {
        return Ok(());
    };
    let socket = directory.join("supervisor.sock");
    if let Ok(metadata) = fs::symlink_metadata(&socket) {
        if !metadata.file_type().is_socket() {
            return Err("Supervisor endpoint is not a socket; refusing recovery".into());
        }
        // Only a refused connection proves a stale socket. Other failures leave
        // ownership uncertain and must never displace an existing endpoint.
        match UnixStream::connect(&socket) {
            Err(error) if error.kind() == io::ErrorKind::ConnectionRefused => {}
            _ => {
                return Err(
                    "Supervisor ownership is unresolved; refusing to displace a live endpoint"
                        .into(),
                );
            }
        }
        fs::remove_file(&socket)?;
    }
    let listener = UnixListener::bind(&socket)?;
    fs::set_permissions(&socket, fs::Permissions::from_mode(0o600))?;
    listener.set_nonblocking(true)?;
    let (storage, restored) = match crate::persistence::Worker::start(directory) {
        Ok(ready) => ready,
        Err(error) => {
            let failure = Observation {
                error: Some(format!(
                    "History storage unavailable: {error}; repair storage then reopen the dashboard"
                )),
                control_denied: true,
                ..Observation::default()
            };
            let deadline = std::time::Instant::now() + Duration::from_secs(3);
            while std::time::Instant::now() < deadline {
                if let Ok((mut incoming, _)) = listener.accept() {
                    let _ = send_observation(&mut incoming, &failure);
                    break;
                }
                thread::sleep(Duration::from_millis(5));
            }
            fs::remove_file(&socket)?;
            return Ok(());
        }
    };
    let mut state = restored.state;
    let mut task_identity = restored.task;
    if state.launch_request.is_empty() {
        state.launch_request = uuid::Uuid::new_v4().to_string();
    }
    let mut launches: HashMap<String, LaunchResult> = HashMap::new();
    for result in restored.launches {
        launches.insert(result.request.clone(), result);
    }
    let mut checkout = restored
        .checkout
        .map(|bytes| PathBuf::from(std::ffi::OsString::from_vec(bytes)));
    storage
        .commands
        .try_send(crate::persistence::Command::Save(
            crate::persistence::metadata(&state),
            checkout.as_ref().map(|p| path_bytes(p)),
            task_identity.clone(),
        ))?;
    let mut save_pending = 1usize;
    let mut pending_launch: Option<Request> = None;
    let mut prepared_ready = false;
    let mut gate_release: Option<SyncSender<()>> = None;
    let mut gate_receipt: Option<Receiver<crate::recovery::Receipt>> = None;
    let mut outcome_reply = false;
    state.metadata_pending = true;
    let mut metadata_failed = false;
    let mut flush_pending = false;
    let mut final_flush = false;
    let mut capture_done: Option<Receiver<()>> = None;
    let mut eof = true;
    let mut child: Option<Box<dyn portable_pty::Child + Send + Sync>> = None;
    let mut writer: Option<SyncSender<Vec<u8>>> = None;
    let (write_errors, input_errors) = mpsc::sync_channel(1);
    let mut process_group = 0;
    let mut stopping: Option<Receiver<crate::lifecycle::Outcome>> = None;
    let mut monitor: Option<crate::native_status::Monitor> = None;
    let mut codex_monitor: Option<crate::codex_status::Monitor> = None;
    let mut shutdown = false;
    let mut native_exit = false;
    let screen = Arc::new(Mutex::new(ScreenState {
        parser: vt100::Parser::new(ROWS.try_into().unwrap(), COLS.try_into().unwrap(), 0),
        output_bytes: 0,
    }));
    let mut master: Option<Arc<Mutex<Box<dyn portable_pty::MasterPty + Send>>>> = None;
    let mut controller: Option<UnixStream> = None;
    let mut delivery: Option<Delivery> = None;
    let mut pending = Vec::new();
    loop {
        while let Ok(event) = storage.events.try_recv() {
            use crate::persistence::Completion;
            match event {
                Completion::Prepared => {
                    save_pending = save_pending.saturating_sub(1);
                    prepared_ready = true;
                }
                Completion::Authorized => {
                    save_pending = save_pending.saturating_sub(1);
                    if let Some(release) = gate_release.take() {
                        let _ = release.try_send(());
                    }
                }
                Completion::Saved => {
                    save_pending = save_pending.saturating_sub(1);
                    if save_pending == 0 {
                        metadata_failed = false;
                    }
                }
                Completion::History(page) => state.history = Some(page),
                Completion::Flushed => {
                    flush_pending = false;
                    final_flush = true;
                }
                Completion::Error(kind, error) => {
                    if kind == "history" {
                        state.error = Some(error);
                        continue;
                    }
                    if kind == "metadata" {
                        save_pending = save_pending.saturating_sub(1);
                        metadata_failed = true;
                        pending_launch = None;
                        prepared_ready = false;
                        gate_release = None;
                    }
                    if outcome_reply || pending_launch.is_none() {
                        outcome_reply = false;
                        state.metadata_pending = true;
                        state.error = Some(format!(
                            "Launch/storage outcome not durable: {error}; stop or repair storage before retrying"
                        ));
                        if controller.is_some() && delivery.is_none() {
                            delivery = Some(Delivery::new(&mut state)?);
                        }
                    }
                    flush_pending = false;
                    shutdown = false;
                    final_flush = false;
                    state.error = Some(format!(
                        "History storage failed: {error}; not saved, retry quit"
                    ));
                }
            }
        }
        if let Some(receipt) = &gate_receipt
            && let Ok(result) = receipt.try_recv()
        {
            gate_receipt = None;
            crate::recovery::crash_barrier(directory, "before-result");
            match result {
                crate::recovery::Receipt::Executed => {
                    let verified = state
                        .execution
                        .as_ref()
                        .ok_or_else(|| io::Error::other("Launch stopped before live exec receipt"))
                        .and_then(|evidence| evidence.verify_live());
                    if let Err(error) = verified {
                        state.error = Some(format!(
                            "Exec receipt lost its live process identity: {error}; launch outcome uncertain"
                        ));
                    }
                }
                crate::recovery::Receipt::ExecFailed(error) => {
                    state.execution = None;
                    state.error = Some(if state.agent.as_deref() == Some("Claude Code") {
                        format!(
                            "Claude Code launch failed: {error}; adopted checkout retained. Install claude on PATH and run claude auth login in your usual terminal"
                        )
                    } else if state.agent.as_deref() == Some("Codex CLI") {
                        format!(
                            "Codex CLI launch failed: {error}; adopted checkout retained. Install codex on PATH and run codex login in your usual terminal"
                        )
                    } else {
                        format!(
                            "Native executable launch failed: {error}; adopted checkout retained"
                        )
                    });
                }
                crate::recovery::Receipt::Unknown(error) => {
                    state.error = Some(format!(
                        "Native exec receipt uncertain: {error}; stop managed execution or reconcile explicitly"
                    ))
                }
            }
            let result = LaunchResult {
                request: state.launch_request.clone(),
                identity: state.identity.clone(),
                error: state.error.clone(),
            };
            launches.insert(result.request.clone(), result.clone());
            state.launch_result = Some(result);
            match storage.commands.try_send(crate::persistence::Command::Save(
                crate::persistence::metadata(&state),
                checkout.as_ref().map(|p| path_bytes(p)),
                task_identity.clone(),
            )) {
                Ok(()) => {
                    save_pending += 1;
                    outcome_reply = true;
                }
                Err(error) => {
                    metadata_failed = true;
                    state.error = Some(format!(
                        "Launch outcome not durable: {error}; reconcile before replacement"
                    ));
                }
            }
        }
        if outcome_reply && save_pending == 0 && !metadata_failed {
            crate::recovery::crash_barrier(directory, "after-result");
            outcome_reply = false;
            state.metadata_pending = false;
            screen.lock().unwrap().snapshot(&mut state);
            if controller.is_some() && delivery.is_none() {
                delivery = Some(Delivery::new(&mut state)?);
            }
        }
        if let Some(done) = &capture_done
            && done.try_recv().is_ok()
        {
            eof = true;
            capture_done = None;
        }
        let progress = storage.progress.lock().unwrap().clone();
        state.capture_pending_bytes = progress.pending_bytes;
        state.capture_high_water = progress.pending_high_water;
        state.capture_backpressure_count = progress.backpressure_count;
        state.capture_backpressure_micros = progress.backpressure_micros;
        state.durable_bytes = progress.durable;
        state.durable_flush_count = progress.flush_count;
        state.durable_completed_millis = progress.completed_millis;
        state.sync_commit_micros = progress.last_flush_micros;
        state.accepted_to_durable_millis = progress.accepted_to_durable_millis;
        if let Some(error) = progress.error {
            state.error = Some(error);
        }
        state.metadata_pending = save_pending > 0 || metadata_failed;
        if (shutdown || matches!(state.lifecycle, Lifecycle::Exited | Lifecycle::Stopped))
            && child.is_none()
            && stopping.is_none()
            && eof
            && !flush_pending
            && !final_flush
            && storage
                .commands
                .try_send(crate::persistence::Command::Flush)
                .is_ok()
        {
            flush_pending = true;
        }
        state.stopped = shutdown && final_flush && save_pending == 0 && !metadata_failed;
        if let Ok(error) = input_errors.try_recv() {
            state.error = Some(format!("Native input failed: {error}; stop or retry"));
        }
        if stopping.is_none()
            && state.lifecycle != Lifecycle::StopFailed
            && let Some(active) = child.as_mut()
        {
            match active.try_wait() {
                Ok(Some(_)) => {
                    native_exit = true;
                    writer = None;
                    state.lifecycle = Lifecycle::Stopping;
                    stopping = Some(crate::lifecycle::stop(child.take().unwrap(), process_group));
                }
                Ok(None) => {}
                Err(error) => {
                    state.error = Some(format!(
                        "Cannot observe execution: {error}; stop explicitly"
                    ))
                }
            }
        }
        if let Some(receiver) = &stopping
            && let Ok(outcome) = receiver.try_recv()
        {
            stopping = None;
            child = outcome.child;
            if let Some(error) = outcome.error {
                state.lifecycle = Lifecycle::StopFailed;
                state.error = Some(error);
                shutdown = false;
            } else {
                state.lifecycle = if native_exit {
                    Lifecycle::Exited
                } else {
                    Lifecycle::Stopped
                };
                state.execution = None;
                final_flush = false;
                if storage
                    .commands
                    .try_send(crate::persistence::Command::Save(
                        crate::persistence::metadata(&state),
                        checkout.as_ref().map(|p| path_bytes(p)),
                        task_identity.clone(),
                    ))
                    .is_ok()
                {
                    save_pending += 1;
                    state.metadata_pending = true;
                } else {
                    metadata_failed = true;
                    state.metadata_pending = true;
                    state.error = Some("Lifecycle metadata not saved; retry quit".into());
                    shutdown = false;
                }
            }
        }
        if state.stopped && controller.is_none() {
            fs::remove_file(&socket)?;
            return Ok(());
        }
        if monitor.is_some() || codex_monitor.is_some() {
            let current = screen.lock().unwrap();
            if current.output_bytes != state.output_bytes {
                current.snapshot(&mut state);
            }
        }
        if let Some(monitor) = monitor.as_mut() {
            state.status = monitor.poll(
                state.lifecycle == Lifecycle::Running,
                &state.screen,
                state.output_bytes,
            );
        }
        if let Some(monitor) = codex_monitor.as_mut() {
            state.status = monitor.poll(
                state.lifecycle == Lifecycle::Running,
                &state.screen,
                state.output_bytes,
            );
        }
        if let Some(id) = codex_monitor.as_ref().and_then(|m| m.native_session())
            && state.native_conversation.as_deref() != Some(id)
            && uuid::Uuid::parse_str(id).is_ok()
        {
            let mut saved = state.clone();
            saved.native_conversation = Some(id.to_owned());
            if storage
                .commands
                .try_send(crate::persistence::Command::Save(
                    crate::persistence::metadata(&saved),
                    checkout.as_ref().map(|p| path_bytes(p)),
                    task_identity.clone(),
                ))
                .is_ok()
            {
                state.native_conversation = saved.native_conversation;
                save_pending += 1;
                state.metadata_pending = true;
            }
        }
        // Accept/reject independently of the active client's commands or output.
        for _ in 0..if pending_launch.is_some() || gate_receipt.is_some() || outcome_reply {
            0
        } else {
            32
        } {
            match listener.accept() {
                Ok((mut incoming, _)) => {
                    incoming.set_nonblocking(true)?;
                    if controller.is_some() {
                        let refusal = Observation {
                            control_denied: true,
                            error: Some("Another dashboard controls this supervisor; close it before reconnecting".into()),
                            ..Observation::default()
                        };
                        let _ = send_observation(&mut incoming, &refusal);
                    } else {
                        screen.lock().unwrap().snapshot(&mut state);
                        delivery = Some(Delivery::new(&mut state)?);
                        state.subscriber_frame_high_water = state
                            .subscriber_frame_high_water
                            .max(delivery.as_ref().unwrap().bytes.len());
                        controller = Some(incoming);
                        pending.clear();
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) => return Err(error.into()),
            }
        }
        if let Some(stream) = controller.as_mut()
            && let Some(response) = delivery.as_mut()
        {
            let stopped = response.stopped;
            match response.advance(stream) {
                Ok(true) => {
                    delivery = None;
                }
                Ok(false) => {
                    thread::sleep(Duration::from_millis(1));
                    continue;
                }
                Err(error) => {
                    if error.kind() == io::ErrorKind::TimedOut {
                        state.error = Some(error.to_string());
                    }
                    controller = None;
                    delivery = None;
                    pending.clear();
                    continue;
                }
            }
            if stopped {
                fs::remove_file(&socket)?;
                return Ok(());
            }
        }
        if (pending_launch.is_some() && !prepared_ready) || gate_receipt.is_some() || outcome_reply
        {
            thread::sleep(Duration::from_millis(5));
            continue;
        }
        if !(prepared_ready && pending_launch.is_some()) {
            let Some(stream) = controller.as_mut() else {
                thread::sleep(Duration::from_millis(5));
                continue;
            };
            let mut incoming = [0; 4096];
            match stream.read(&mut incoming) {
                Ok(0) => {
                    controller = None;
                    pending.clear();
                    continue;
                }
                Ok(n) => pending.extend_from_slice(&incoming[..n]),
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
                Err(_) => {
                    controller = None;
                    pending.clear();
                    continue;
                }
            }
            if pending.len() > MAX_FRAME {
                controller = None;
                pending.clear();
                continue;
            }
        }
        let continuing = prepared_ready && pending_launch.is_some();
        let request: Request = if continuing {
            prepared_ready = false;
            pending_launch.take().unwrap()
        } else {
            let Some(end) = pending.iter().position(|byte| *byte == b'\n') else {
                thread::sleep(Duration::from_millis(5));
                continue;
            };
            let bytes: Vec<_> = pending.drain(..=end).collect();
            match serde_json::from_slice(&bytes) {
                Ok(request) => request,
                Err(_) => {
                    controller = None;
                    pending.clear();
                    continue;
                }
            }
        };
        let mut deferred = false;
        if !matches!(request, Request::Screen) {
            state.error = None;
        }
        let save_history =
            matches!(&request, Request::History { index, .. } if *index != state.history_index);
        let save = save_history
            || matches!(
                request,
                Request::Adopt(_)
                    | Request::Launch(_)
                    | Request::LaunchClaude(_)
                    | Request::Reconcile
                    | Request::LaunchCodex(_)
                    | Request::Resume(_)
                    | Request::Stop
                    | Request::Shutdown
            );
        let resume = matches!(&request, Request::Resume(_));
        let claude = matches!(&request, Request::LaunchClaude(_))
            || (resume && state.agent.as_deref() == Some("Claude Code"));
        let codex = matches!(&request, Request::LaunchCodex(_))
            || (resume && state.agent.as_deref() == Some("Codex CLI"));
        let launch_request = match &request {
            Request::Launch(id)
            | Request::LaunchClaude(id)
            | Request::LaunchCodex(id)
            | Request::Resume(id) => Some(id.clone()),
            _ => None,
        };
        let recorded = match &request {
            Request::Launch(id)
            | Request::LaunchClaude(id)
            | Request::LaunchCodex(id)
            | Request::Resume(id)
            | Request::QueryLaunch(id) => launches.get(id).cloned(),
            _ => None,
        };
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            if let Some(recorded) = &recorded {
                state.launch_result = Some(recorded.clone());
                state.error = recorded.error.clone();
                return Ok(());
            }
            if (pending_launch.is_some() || gate_receipt.is_some())
                && matches!(
                    request,
                    Request::Adopt(_)
                        | Request::Launch(_)
                        | Request::LaunchClaude(_)
                        | Request::LaunchCodex(_)
                        | Request::Resume(_)
                )
            {
                return Err("Launch reconciliation pending; wait for durable outcome".into());
            }
            match request {
                Request::Adopt(bytes) => {
                    let path =
                            PathBuf::from(std::ffi::OsString::from_vec(bytes)).canonicalize().map_err(|error| format!("Checkout unavailable: {error}; refresh and choose an existing working checkout"))?;
                    let output = Command::new("git")
                        .arg("-C")
                        .arg(&path)
                        .args(["rev-parse", "--show-toplevel"])
                        .output()?;
                    if !output.status.success() {
                        return Err("Checkout unavailable or bare; choose an existing working checkout and refresh".into());
                    }
                    let mut root = output.stdout;
                    if root.last() == Some(&b'\n') {
                        root.pop();
                    }
                    let path = PathBuf::from(std::ffi::OsString::from_vec(root)).canonicalize()?;
                    if (child.is_some() || stopping.is_some()) && checkout.as_ref() != Some(&path) {
                        return Err(
                            "Stop the existing session before adopting another checkout".into()
                        );
                    }
                    if state.execution.is_some() && child.is_none() && checkout.as_ref() != Some(&path) {
                        return Err("Interrupted execution is uncertain; inspect history before changing adoption".into());
                    }
                    if checkout.as_ref() != Some(&path) {
                        state.identity = None;
                        state.lifecycle = Lifecycle::Idle;
                        state.native_conversation = None;
                        state.agent = None;
                        task_identity = uuid::Uuid::new_v4().to_string();
                    }
                    state.adopted = Some(path.display().to_string());
                    checkout = Some(path);
                }
                Request::Reconcile => {
                    if child.is_some() || stopping.is_some() { return Err("Session owned by current supervisor; stop it normally".into()); }
                    if let Some(evidence) = &state.execution { evidence.reconcile()?; }
                    state.execution = None;
                    state.reconciled = true;
                    state.error = Some("Reconciled: no released coding execution survives; history retained. Explicitly resume with v or launch a new conversation with c/x when ready.".into());
                }
                Request::QueryLaunch(_) => return Err("Launch request unknown to this supervisor; do not replay uncertain activity after supervisor loss".into()),
                Request::Launch(request) | Request::LaunchClaude(request) | Request::LaunchCodex(request) | Request::Resume(request) => {
                    if state.execution.is_some() && !continuing && child.is_none() { return Err("Interrupted execution is uncertain; u reconciles process evidence. Verified reboot resolves unknown descendants; no automatic coding restart.".into()); }
                    if request.is_empty() || request.len() > 128 {
                        return Err("Launch request must contain 1–128 bytes".into());
                    }
                    if launches.len() >= 1024 {
                        return Err("Launch request capacity reached; retained outcomes are preserved and further launch is blocked".into());
                    }
                    state.launch_request = request.clone();
                    if child.is_some() || stopping.is_some() {
                        return Err("A session already exists; attach to it".into());
                    }
                    let path = checkout.as_ref().ok_or("Adopt a checkout before launch")?;
                    if !path.is_dir() { return Err("Checkout unavailable: cannot continue in place; h opens retained history. Restore the checkout before explicitly resuming.".into()); }
                    if resume && (!claude && !codex || state.native_conversation.as_deref().and_then(|id| uuid::Uuid::parse_str(id).ok()).is_none()) {
                        return Err("Native resume unavailable: no supported saved conversation identity. h opens retained history; c Claude / x Codex explicitly starts a new conversation.".into());
                    }
                    if !continuing {
                        if !resume { state.native_conversation = None; }
                        state.execution = Some(crate::recovery::Execution::prepared()?);
                        state.reconciled = false;
                        let session = uuid::Uuid::new_v4().to_string();
                        state.identity = Some(format!("task={} session={} request={request}", task_identity, session));
                        state.launch_result = None;
                        state.lifecycle = Lifecycle::Interrupted;
                        storage.commands.try_send(crate::persistence::Command::Prepare(crate::persistence::metadata(&state), Some(path_bytes(path)), task_identity.clone()))?;
                        save_pending += 1;
                        pending_launch = Some(if resume { Request::Resume(request) } else if claude { Request::LaunchClaude(request) } else if codex { Request::LaunchCodex(request) } else { Request::Launch(request) });
                        deferred = true;
                        return Ok(());
                    }
                    let (rows, cols) = screen.lock().unwrap().parser.screen().size();
                    let pty = native_pty_system().openpty(PtySize {
                        rows: rows.get(),
                        cols: cols.get(),
                        pixel_width: 0,
                        pixel_height: 0,
                    })?;
                    crate::recovery::crash_barrier(directory, "before-create");
                    let gate_path = directory.join("launch-gate.sock");
                    let _ = fs::remove_file(&gate_path);
                    let gate_listener = UnixListener::bind(&gate_path)?;
                    gate_listener.set_nonblocking(true)?;
                    fs::set_permissions(&gate_path, fs::Permissions::from_mode(0o600))?;
                    let nonce = uuid::Uuid::new_v4().to_string();
                    let claude_executable = if claude {
                        std::env::var_os("PATH").and_then(|paths| std::env::split_paths(&paths).find_map(|directory| {
                            let directory = if directory.is_absolute() { directory } else { path.join(directory) };
                            let candidate = directory.join("claude");
                            candidate.is_file().then(|| candidate.canonicalize().ok()).flatten()
                        }))
                    } else { None };
                    let mut command = if claude {
                        CommandBuilder::new(claude_executable.as_deref().unwrap_or_else(|| Path::new("claude")))
                    } else if codex {
                        let mut command = CommandBuilder::new("codex");
                        if resume {
                            command.arg("resume");
                            command.arg(state.native_conversation.as_ref().unwrap());
                        }
                        command.arg("--no-daemon");
                        command
                    } else {
                        let mut command = CommandBuilder::new(std::env::current_exe()?);
                        command.arg("--scripted-child");
                        command
                    };
                    if claude && resume {
                        command.arg("--resume");
                        command.arg(state.native_conversation.as_ref().unwrap());
                    }
                    if !resume { state.native_conversation = None; }
                    monitor = None;
                    codex_monitor = None;
                    state.status = crate::native_status::Status::default();
                    if codex {
                        match crate::codex_status::Monitor::start(directory) {
                            Ok(mut status_monitor) => {
                                if resume { status_monitor.expect_session(state.native_conversation.as_ref().unwrap()); }
                                if status_monitor.configure(&mut command).is_ok() { codex_monitor = Some(status_monitor); }
                                else { state.status.reason = "Codex status monitor unavailable".into(); }
                            }
                            Err(_) => state.status.reason = "Codex status monitor unavailable".into(),
                        }
                    } else {
                        match crate::native_status::Monitor::start(directory, !claude, if resume { state.native_conversation.as_deref() } else { None }) {

                            Ok(mut status_monitor) => {
                                status_monitor.native_executable(claude_executable.as_deref());
                                status_monitor.configure(&mut command, resume);
                                if claude { state.native_conversation = Some(status_monitor.native_session().to_owned()); }
                                monitor = Some(status_monitor);
                            }
                            Err(_) => {
                                state.status.reason = "status monitor unavailable".into();
                                if claude && !resume {
                                    let id = uuid::Uuid::new_v4().to_string();
                                    command.arg("--session-id");
                                    command.arg(&id);
                                    state.native_conversation = Some(id);
                                }
                            },
                        }

                    }
                    command.env("TERM", "xterm-256color");
                    command.cwd(path);
                    crate::recovery::wrap(&mut command, &gate_path, &nonce)?;
                    let launched = pty.slave.spawn_command(command).map_err(|error| {
                        if claude {
                            format!("Claude Code launch failed: {error}. Install claude on PATH, run claude auth login in your usual terminal, then explicitly retry c; adopted checkout retained")
                        } else if codex {
                            format!("Codex CLI launch failed: {error}. Install codex on PATH, run codex login in your usual terminal, then explicitly retry x; adopted checkout retained")
                        } else {
                            format!("Fixture launch failed: {error}; retry l")
                        }
                    })?;
                    crate::recovery::crash_barrier(directory, "after-create");
                    state.agent = Some(
                        if claude {
                            "Claude Code"
                        } else if codex {
                            "Codex CLI"
                        } else {
                            "Scripted PTY"
                        }
                        .into(),
                    );
                    drop(pty.slave);
                    process_group = launched
                        .process_id()
                        .ok_or("Missing managed process identity")?
                        .try_into()?;
                    state.execution.as_mut().ok_or("Missing durable intent")?.identify(process_group)?;
                    let (release, authorized) = mpsc::sync_channel(1);
                    let (receipt, result) = mpsc::sync_channel(1);
                    gate_release = Some(release);
                    gate_receipt = Some(result);
                    thread::spawn(move || {
                        let result = (|| -> io::Result<crate::recovery::Receipt> {
                            let deadline = std::time::Instant::now() + Duration::from_secs(2);
                            let mut stream = loop {
                                match gate_listener.accept() {
                                    Ok((stream, _)) => break stream,
                                    Err(error) if error.kind() == io::ErrorKind::WouldBlock && std::time::Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
                                    Err(error) => return Err(error),
                                }
                            };
                            // macOS accepted sockets inherit nonblocking status.
                            stream.set_nonblocking(false)?;
                            stream.set_read_timeout(Some(Duration::from_secs(5)))?;
                            let mut actual = vec![0; nonce.len()];
                            stream.read_exact(&mut actual)?;
                            if actual != nonce.as_bytes() { return Err(io::Error::other("Launch gate nonce mismatch")); }
                            authorized.recv().map_err(|_| io::Error::other("Launch authorization lost"))?;
                            stream.write_all(&[1])?;
                            let mut error = String::new();
                            stream.take(4096).read_to_string(&mut error)?;
                            if !error.is_empty() { return Ok(crate::recovery::Receipt::ExecFailed(error)); }
                            Ok(crate::recovery::Receipt::Executed)
                        })().unwrap_or_else(|e| crate::recovery::Receipt::Unknown(e.to_string()));
                        let _ = fs::remove_file(gate_path);
                        let _ = receipt.send(result);
                    });
                    storage.commands.try_send(crate::persistence::Command::Authorize(crate::persistence::metadata(&state), Some(path_bytes(path)), task_identity.clone()))?;
                    save_pending += 1;
                    deferred = true;
                    let mut sink = pty.master.take_writer()?;
                    let (input, writes) = mpsc::sync_channel::<Vec<u8>>(32);
                    let errors = write_errors.clone();
                    thread::spawn(move || {
                        while let Ok(bytes) = writes.recv() {
                            if let Err(error) = sink.write_all(&bytes) {
                                let _ = errors.try_send(error.to_string());
                                break;
                            }
                        }
                    });
                    writer = Some(input);
                    state.lifecycle = Lifecycle::Running;
                    state.stopped = false;
                    native_exit = false;
                    state.history_index = 0;
                    let session = crate::persistence::session_id(state.identity.as_ref().ok_or("Missing prepared identity")?).map_err(|e| e.to_string())?;
                    let capture = storage.output.clone();
                    let (finished, done) = mpsc::sync_channel(1);
                    capture_done = Some(done);
                    eof = false;
                    final_flush = false;
                    let mut source = pty.master.try_clone_reader()?;
                    master = Some(Arc::new(Mutex::new(pty.master)));
                    let parser = screen.clone();
                    thread::spawn(move || {
                        let mut bytes = [0; 4096];
                        while let Ok(n) = source.read(&mut bytes) {
                            if n == 0 {
                                break;
                            }
                            // Transfer capture ownership before parsing; never block with parser lock.
                            if capture.send((session.clone(), bytes[..n].to_vec(), std::time::Instant::now())).is_err() { break; }
                            let mut screen = parser.lock().unwrap();
                            screen.parser.process(&bytes[..n]);
                            screen.output_bytes = screen.output_bytes.saturating_add(n as u64);
                        }
                        let _ = finished.send(());
                    });
                    child = Some(launched);
                }
                Request::Input(bytes) => {
                    if let Some(monitor) = monitor.as_mut() { monitor.input(screen.lock().unwrap().output_bytes, bytes.as_slice() == [3]); }
                    if let Some(monitor) = codex_monitor.as_mut() { monitor.input(screen.lock().unwrap().output_bytes); }
                    if bytes.len() > 4096 {
                        return Err("Input exceeds prototype 4096-byte limit".into());
                    }
                    writer
                        .as_mut()
                        .ok_or("No session; launch first")?
                        .try_send(bytes)
                        .map_err(|e| format!("Native input queue unavailable: {e}; retry"))?;
                }
                Request::Key(key) => {
                    if let Some(monitor) = monitor.as_mut() { monitor.input(screen.lock().unwrap().output_bytes, matches!(key.code, crossterm::event::KeyCode::Char('c' | 'C')) && key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) && !key.modifiers.contains(crossterm::event::KeyModifiers::ALT)); }
                    if let Some(monitor) = codex_monitor.as_mut() { monitor.input(screen.lock().unwrap().output_bytes); }
                    let application_cursor =
                        screen.lock().unwrap().parser.screen().application_cursor();
                    let bytes = crate::native_input::encode(key, application_cursor)?;
                    writer
                        .as_mut()
                        .ok_or("No session; launch first")?
                        .try_send(bytes)
                        .map_err(|e| format!("Native input queue unavailable: {e}; retry"))?;
                }
                Request::Paste(text) => {
                    if let Some(monitor) = monitor.as_mut() { monitor.input(screen.lock().unwrap().output_bytes, false); }
                    if let Some(monitor) = codex_monitor.as_mut() { monitor.input(screen.lock().unwrap().output_bytes); }
                    if text.len() > 4084 {
                        return Err("Paste exceeds 4084-byte limit".into());
                    }
                    let bracketed = screen.lock().unwrap().parser.screen().bracketed_paste();
                    let bytes = if bracketed {
                        format!("\x1b[200~{text}\x1b[201~").into_bytes()
                    } else {
                        text.replace("\r\n", "\r").replace('\n', "\r").into_bytes()
                    };
                    writer
                        .as_mut()
                        .ok_or("No session; launch first")?
                        .try_send(bytes)
                        .map_err(|e| format!("Native input queue unavailable: {e}; retry"))?;
                }
                Request::Resize { rows, cols } => {
                    if rows == 0 || cols == 0 {
                        return Ok(());
                    }
                    let (rows, cols) = (rows.clamp(1, MAX_ROWS), cols.clamp(1, MAX_COLS));
                    // Serialize geometry changes against output parsing. Resize the PTY
                    // first; output induced by SIGWINCH cannot enter the old parser size.
                    let mut screen = screen.lock().unwrap();
                    if let Some(master) = master.as_ref() {
                        master.lock().unwrap().resize(PtySize {
                            rows,
                            cols,
                            pixel_width: 0,
                            pixel_height: 0,
                        })?;
                    }
                    screen
                        .parser
                        .set_size(rows.try_into().unwrap(), cols.try_into().unwrap());
                }
                Request::History { index, offset } => {
                    state.history_index = index;
                    storage.commands.try_send(crate::persistence::Command::History(index, offset)).map_err(|e| format!("History busy: {e}; retry"))?;
                }
                Request::Screen => {}
                Request::Stop | Request::Shutdown => {
                    gate_release = None;
                    if child.is_none() && state.execution.is_some() {
                        return Err("Interrupted execution remains uncertain; u reconciles evidence, d detaches. A missing native leader may require verified reboot before replacement or confirmed shutdown.".into());
                    }
                    shutdown |= matches!(request, Request::Shutdown);
                    if stopping.is_none() {
                        if let Some(active) = child.take() {
                            writer = None;
                            native_exit = false;
                            state.lifecycle = Lifecycle::Stopping;
                            stopping = Some(crate::lifecycle::stop(active, process_group));
                        } else {
                            final_flush = false;
                        }
                    }
                }
            }
            Ok(())
        })();
        if let Err(error) = result {
            state.error = Some(error.to_string());
        }
        let fresh_launch = launch_request.is_some() && recorded.is_none();
        if let Some(request) = launch_request
            && !deferred
            && recorded.is_none()
            && request.len() <= 128
            && !request.is_empty()
            && launches.len() < 1024
        {
            let result = LaunchResult {
                request: request.clone(),
                identity: if state.error.is_none() {
                    state.identity.clone()
                } else {
                    None
                },
                error: state.error.clone(),
            };
            launches.insert(request, result.clone());
            state.launch_result = Some(result);
        }
        if save && !deferred {
            match storage.commands.try_send(crate::persistence::Command::Save(
                crate::persistence::metadata(&state),
                checkout.as_ref().map(|p| path_bytes(p)),
                task_identity.clone(),
            )) {
                Ok(()) => {
                    save_pending += 1;
                    state.metadata_pending = true;
                    if fresh_launch {
                        outcome_reply = true;
                    }
                }
                Err(error) => {
                    metadata_failed = true;
                    state.metadata_pending = true;
                    state.error = Some(format!("Metadata not saved: {error}; retry"));
                    shutdown = false;
                }
            }
        }
        screen.lock().unwrap().snapshot(&mut state);
        if let Some(monitor) = monitor.as_mut() {
            state.status = monitor.poll(
                state.lifecycle == Lifecycle::Running,
                &state.screen,
                state.output_bytes,
            );
        }
        if !deferred && !outcome_reply && controller.is_some() {
            delivery = Some(Delivery::new(&mut state)?);
            state.subscriber_frame_high_water = state
                .subscriber_frame_high_water
                .max(delivery.as_ref().unwrap().bytes.len());
        }
    }
}
pub fn scripted_child() -> io::Result<()> {
    crossterm::terminal::enable_raw_mode()?;
    struct RawGuard;
    impl Drop for RawGuard {
        fn drop(&mut self) {
            let _ = crossterm::terminal::disable_raw_mode();
        }
    }
    let _guard = RawGuard;
    println!(
        "Scripted PTY · pid={} · cwd={}\r",
        std::process::id(),
        std::env::current_dir()?.display()
    );
    // Split both UTF-8 and a mode sequence to exercise incremental screen parsing.
    for bytes in [b"\xce".as_slice(), b"\xbb\r\n\x1b[?20", b"04hfixture> "] {
        io::stdout().write_all(bytes)?;
        io::stdout().flush()?;
    }
    let mut line = Vec::new();
    let mut approval = false;
    let mut flood_run = 0;
    let mut native_active = false;
    for byte in io::stdin().lock().bytes() {
        match byte? {
            3 => {
                line.clear();
                if native_active {
                    print!(
                        "\x1b[2J\x1b[HClaude Code v2.1.318\r\n  ⎿  Interrupted · What should Claude do instead?\r\n────────────────────────────────────────\r\n❯ \r\n────────────────────────────────────────\r\n  ⏵⏵ auto mode on (shift+tab to cycle)"
                    );
                    native_active = false;
                } else {
                    println!("interrupted: 03\r");
                    print!("fixture> ");
                }
            }
            b'\r' => {
                if line == b"exit" {
                    break;
                }
                if line == b"survive" {
                    // Isolated crash fixture: remain alive when the owning PTY closes.
                    unsafe {
                        libc::signal(libc::SIGHUP, libc::SIG_IGN);
                    }
                    println!("survival enabled\r");
                    io::stdout().flush()?;
                    loop {
                        thread::sleep(Duration::from_secs(60));
                    }
                } else if line == b"no-read" {
                    println!("input paused\r");
                    io::stdout().flush()?;
                    loop {
                        thread::sleep(Duration::from_secs(60));
                    }
                } else if line == b"descendant" {
                    // Both leader and descendant refuse TERM so bounded KILL is exercised.
                    ignore_term();
                    let mut command = Command::new(std::env::current_exe()?);
                    command.arg("--scripted-descendant");
                    let mut descendant = command.spawn()?;
                    thread::spawn(move || {
                        let _ = descendant.wait();
                    });
                } else if line == b"size" {
                    let (cols, rows) = crossterm::terminal::size()?;
                    print!("size: {cols}x{rows}\r\nfixture> ");
                } else if line == b"fixture-failed-test" {
                    println!("test result: FAILED; agent continues\r");
                } else if line == b"status-native-submit" {
                    crate::native_status::fixture("UserPromptSubmit");
                    print!("\x1b[2J\x1b[HClaude Code v2.1.318\r\n❯ proof\r\n");
                } else if line == b"status-native-active" {
                    native_active = true;
                    print!(
                        "\x1b[2J\x1b[HClaude Code v2.1.318\r\n⏺ Ownership controls when values are destroyed.\r\n\r\n✢ Noodling…\r\n────────────────────────────────────────\r\n❯ \r\n────────────────────────────────────────\r\n  ⏵⏵ auto mode on (shift+tab to cycle) · esc to interrupt"
                    );
                } else if line == b"status-native-blocked-stop" {
                    crate::native_status::fixture("Stop");
                    print!(
                        "\x1b[2J\x1b[HClaude Code v2.1.318\r\n✢ Running Stop hooks…\r\n────────────────────────────────────────\r\n❯ \r\n────────────────────────────────────────\r\n  ⏵⏵ auto mode on (shift+tab to cycle) · esc to interrupt"
                    );
                } else if line == b"status-native-old-done" {
                    crate::native_status::fixture("UserPromptSubmit");
                    io::stdout().flush()?;
                    thread::sleep(Duration::from_millis(80));
                    crate::native_status::fixture("Stop");
                    println!("\r");
                } else if line == b"status-native-done" {
                    crate::native_status::fixture("Stop");
                    io::stdout().flush()?;
                    thread::sleep(Duration::from_millis(80));
                    print!(
                        "\x1b[2J\x1b[HClaude Code v2.1.318\r\n⏺ proof answer\r\n\r\n✻ Cogitated for 1s · done 3:12 PM\r\n\r\n────────────────────────────────────────\r\n❯ \r\n────────────────────────────────────────\r\n  ⏵⏵ auto mode on (shift+tab to cycle) · ← for agents"
                    );
                } else if line == b"status-native-post" {
                    crate::native_status::fixture("PostToolUse");
                    println!("question answered; old dialog text is retained\r");
                } else if line == b"status-native-approval" {
                    crate::native_status::fixture("PostToolUse");
                    crate::native_status::fixture("FixtureNativeBashPre");
                    crate::native_status::fixture("FixtureNativeBashPermission");
                    print!(
                        "\x1b[2J\x1b[H────────────────────────────────────────\r\n Bash command\r\n Do you want to proceed?\r\n ❯ 1. Yes\r\n   2. No\r\n Esc to cancel · Tab to amend"
                    );
                } else if line == b"status-native-question" {
                    crate::native_status::fixture("PreToolUse");
                    print!(
                        "\x1b[2J\x1b[H────────────────────────────────────────\r\n ☐ Proof marker\r\nShould the proof marker be alpha or beta?\r\n❯ 1. Alpha\r\n  2. Beta\r\n────────────────────────────────────────\r\nEnter to select · ↑/↓ to navigate · Esc to cancel"
                    );
                } else if line.starts_with(b"status-") {
                    let event = match line.as_slice() {
                        b"status-working" => "FixtureWorking",
                        b"status-input" => "FixtureInput",
                        b"status-approval" => "FixtureApproval",
                        b"status-completed" => "FixtureCompleted",
                        b"status-interrupted" => "FixtureInterrupted",
                        b"status-wrong-session" => "FixtureWrongSession",
                        _ => "FixtureInvalid",
                    };
                    crate::native_status::fixture(event);
                    println!("fixture status evidence emitted\r");
                } else if line == b"screen" {
                    print!("\x1b[2J\x1b[H\x1b[1;31mstyled ");
                    io::stdout().flush()?;
                    for bytes in [
                        b"\xce".as_slice(),
                        b"\xbb\xe7\x95",
                        b"\x8c\x1b[0",
                        b"m\r\nfixture> ",
                    ] {
                        io::stdout().write_all(bytes)?;
                        io::stdout().flush()?;
                    }
                } else if line == b"alternate" {
                    print!(
                        "\x1b[?1049h\x1b[2J\x1b[H\x1b[?1003h\x1b[?1006halternate screen\r\nfixture> "
                    );
                } else if line == b"primary" {
                    print!("\x1b[?1003l\x1b[?1006l\x1b[?1049l\r\nprimary restored\r\nfixture> ");
                } else if line == b"mosaic" {
                    let (cols, rows) = crossterm::terminal::size()?;
                    for row in 1..=rows {
                        for col in 1..=cols {
                            print!(
                                "\x1b[{row};{col}H\x1b[1;38;2;{};{};7;48;2;1;2;3mX",
                                row % 256,
                                col % 256
                            );
                        }
                    }
                    print!("\x1b[0m\x1b[{rows};1Hmax geometry complete");
                } else if line == b"stream" {
                    for n in 0..200 {
                        println!("stream: {n:04}\r");
                        io::stdout().flush()?;
                        thread::sleep(Duration::from_millis(5));
                    }
                    print!("fixture> ");
                } else if line == b"flood13" {
                    flood_run += 1;
                    let start_ns = fixture_clock()?;
                    let since = Instant::now();
                    let mut max_write_micros = 0;
                    let payload = "X".repeat(111);
                    let mut stdout = io::stdout().lock();
                    for number in 0..32768 {
                        let record = format!("flood13:{number:06}:{payload}\r\n");
                        let write = Instant::now();
                        stdout.write_all(record.as_bytes())?;
                        max_write_micros = max_write_micros.max(write.elapsed().as_micros());
                    }
                    writeln!(
                        stdout,
                        "flood13-complete run={flood_run} bytes=4194304 elapsed_us={} max_write_us={max_write_micros} start_ns={start_ns} finish_ns={}\r",
                        since.elapsed().as_micros(),
                        fixture_clock()?
                    )?;
                    write!(stdout, "fixture> ")?;
                } else if line == b"history-bulk" {
                    for number in 0..6000 {
                        println!(
                            "bulk08:{number:05}:abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789--\r"
                        );
                    }
                    println!("history-bulk-complete\r");
                    print!("fixture> ");
                } else if line == b"history-controls" {
                    print!(
                        "erased-response-08\r\x1b[2Kreplacement-08\r\n\x1b[?1049halternate-response-08\x1b[?1049lafter-history-controls\r\nfixture> "
                    );
                } else if line == b"identity" {
                    println!("same-child pid={}\r", std::process::id());
                    print!("fixture> ");
                } else if line == b"cursor-on" {
                    print!("\x1b[?1hcursor application\r\nfixture> ");
                } else if line == b"paste-off" {
                    print!("\x1b[?2004lpaste plain\r\nfixture> ");
                } else if approval {
                    println!("approval: {}\r", if line == b"y" { "yes" } else { "no" });
                    approval = false;
                } else if line == b"approve" {
                    print!("Approve action? [y/n] ");
                    approval = true;
                } else {
                    if line == b"attach13-nonce" {
                        println!("input13-received_ns={}\r", fixture_clock()?);
                    }
                    println!(
                        "received: {}\r",
                        String::from_utf8_lossy(&line).escape_debug()
                    );
                    let hex: String = line.iter().map(|b| format!("{b:02x}")).collect();
                    println!("bytes: {hex}\r");
                    print!("fixture> ");
                }
                line.clear();
            }
            byte => {
                if line.len() < 4096 {
                    line.push(byte);
                } else {
                    return Err(io::Error::other("Fixture input exceeds 4096 bytes"));
                }
            }
        }
        io::stdout().flush()?;
    }
    print!("\x1b[?2004l");
    io::stdout().flush()
}

fn fixture_clock() -> io::Result<u64> {
    let mut time = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: writable timespec; the isolated fixture and harness share this kernel clock.
    if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut time) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(time.tv_sec as u64 * 1_000_000_000 + time.tv_nsec as u64)
}

fn ignore_term() {
    // SAFETY: installs the POSIX constant disposition, with no Rust signal handler.
    unsafe {
        libc::signal(libc::SIGTERM, libc::SIG_IGN);
    }
}
pub fn scripted_descendant() -> io::Result<()> {
    ignore_term();
    println!("descendant pid={}\r", std::process::id());
    io::stdout().flush()?;
    loop {
        thread::sleep(Duration::from_secs(60));
    }
}
