//! Thin scripted-session proof. All blocking transport and PTY operations live here.
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use std::{
    collections::{HashMap, VecDeque},
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
        atomic::{AtomicBool, Ordering},
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
pub use crate::events::{LaunchResult, Lifecycle, Observation, Request};
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
        if greeting.git_jobs_protocol != 1 || greeting.registry_protocol != 1 {
            events.send(greeting)?;
            return Ok(());
        }
        drop(reader);
        drop(stream);
        events.send(greeting.clone())?;
        while let Ok(request) = requests.recv() {
            greeting.discovery = None;
            match request {
                Request::Discover {
                    request,
                    repository,
                } => {
                    let path = PathBuf::from(std::ffi::OsString::from_vec(repository.clone()));
                    let result = crate::git::read_only_discovery(&path);
                    greeting.discovery = Some(crate::git::Discovery {
                        request,
                        repository,
                        result,
                    });
                }
                Request::Shutdown => greeting.stopped = true, // local exit only; no owner control
                _ => {} // a refused dashboard cannot send any session command
            }
            let stopped = greeting.stopped;
            events.send(greeting.clone())?;
            if stopped {
                return Ok(());
            }
        }
        return Ok(());
    }
    if greeting.git_jobs_protocol != 1 || greeting.registry_protocol != 1 {
        greeting.control_denied = true;
        greeting.error = Some("Existing supervisor does not support this dashboard's worktree protocol or registry protocol. Use its compatible dashboard to stop activity before upgrading; d detaches locally.".into());
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
// One in-flight creation owns its PTY and child even before the supervisor
// can install input/capture. The bounded receiver is also the launch reservation.
struct CreatedExecution {
    pty: portable_pty::PtyPair,
    child: crate::lifecycle::Child,
    listener: UnixListener,
    nonce: String,
}
struct CreatingExecution {
    request: Request,
    result: Receiver<Result<CreatedExecution, String>>,
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
        let mut visible = state.clone();
        if visible.metadata_pending {
            visible.launch_result = None;
        }
        let mut bytes = serde_json::to_vec(&visible)?;
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
    state.git_jobs_protocol = 1;
    state.registry_protocol = 1;
    state.registry_pending = false;
    state.discovery = None;
    state.adoption_pending = false;
    let mut task_identity = restored.task;
    if state.launch_request.is_empty() {
        state.launch_request = uuid::Uuid::new_v4().to_string();
    }
    let mut launches: HashMap<String, LaunchResult> = HashMap::new();
    for result in restored.launches {
        launches.insert(result.request.clone(), result);
    }
    let mut adoption: Option<crate::git::AdoptionJob> = None;
    let mut registering = false;
    let mut registry_operation = None;
    let mut discovery: Option<crate::git::GitWorker> = None;
    let mut pending_discovery: Option<(u64, PathBuf)> = None;
    let mut checkout = restored
        .checkout
        .map(|bytes| PathBuf::from(std::ffi::OsString::from_vec(bytes)));
    let mut writes = crate::persistence::Obligations::default();
    writes.send(
        &storage.commands,
        crate::persistence::Command::Save(crate::persistence::Snapshot::new(
            &state,
            checkout.as_deref(),
            &task_identity,
        )),
    )?;
    let mut pending_launch: Option<Request> = None;
    let mut prepared_ready = false;
    let mut creating: Option<CreatingExecution> = None;
    let mut created: Option<Result<CreatedExecution, String>> = None;
    let mut cancelled_launch = false;
    let mut gate_release: Option<SyncSender<()>> = None;
    let mut partial_gate: Option<UnixListener> = None;
    let mut gate_receipt: Option<Receiver<crate::recovery::Receipt>> = None;
    let mut outcome_reply = false;
    let mut outcome_operation = None;
    let mut result_failed = false;
    state.metadata_pending = true;
    let mut metadata_failed = false;
    let mut metadata_error: Option<String> = None;
    let mut flush_pending = false;
    let mut final_flush = false;
    let mut history_sealed = false;
    let mut seal_pending = false;
    let mut capture_started_here = false;
    let mut capture_done: Option<Receiver<()>> = None;
    let stop_drain = Arc::new(AtomicBool::new(false));
    let mut eof = true;
    let mut child: Option<Box<dyn portable_pty::Child + Send + Sync>> = None;
    let mut writer: Option<SyncSender<Vec<u8>>> = None;
    let (write_errors, input_errors) = mpsc::sync_channel(1);
    let mut owned_scope: Option<crate::child_scope::Scope> = None;
    let mut stopping: Option<Receiver<crate::lifecycle::Outcome>> = None;
    let mut monitor: Option<crate::native_status::Monitor> = None;
    let mut codex_monitor: Option<crate::codex_status::Monitor> = None;
    let mut shutdown = false;
    let mut retry_requested = false;
    let screen = Arc::new(Mutex::new(ScreenState {
        parser: vt100::Parser::new(ROWS.try_into().unwrap(), COLS.try_into().unwrap(), 0),
        output_bytes: 0,
    }));
    let mut master: Option<Arc<Mutex<Box<dyn portable_pty::MasterPty + Send>>>> = None;
    let mut controller: Option<UnixStream> = None;
    let mut delivery: Option<Delivery> = None;
    let mut pending = Vec::new();
    loop {
        if adoption
            .as_ref()
            .is_some_and(|job| !state.adoption_pending && job.is_finished())
        {
            adoption = None;
        }
        if discovery
            .as_ref()
            .is_some_and(|worker| worker.is_finished())
        {
            discovery = None;
        }
        if let Some(worker) = &discovery {
            while let Ok(crate::events::Event::DiscoveryCompleted {
                request,
                repository,
                result,
            }) = worker.events.try_recv()
            {
                if pending_discovery.as_ref() != Some(&(request, repository.clone())) {
                    continue;
                }
                pending_discovery = None;
                state.discovery = Some(crate::git::Discovery {
                    request,
                    repository: path_bytes(&repository),
                    result,
                });
            }
        }
        if let Some(result) = adoption
            .as_ref()
            .filter(|_| state.adoption_pending)
            .and_then(|job| job.result.try_recv().ok())
        {
            adoption = None;
            state.adoption_pending = false;
            if registering {
                registering = false;
                let operation = uuid::Uuid::new_v4();
                match result {
                    Ok(identity) => match storage
                        .commands
                        .try_send(crate::persistence::Command::Register(operation, identity))
                    {
                        Ok(()) => {
                            registry_operation = Some(operation);
                            state.registry_pending = true;
                        }
                        Err(error) => {
                            state.registry_pending = false;
                            state.error = Some(format!("Project not registered: {error}; retry"));
                        }
                    },
                    Err(error) => {
                        state.registry_pending = false;
                        state.error = Some(error);
                    }
                }
                continue;
            }
            let result = result.and_then(|identity| {
                let path = identity.root.clone();
                let same_checkout = state.checkout_identity.as_ref().map_or_else(
                    || checkout.as_ref() == Some(&path),
                    |previous| previous.git_dir == identity.git_dir && previous.common_dir == identity.common_dir,
                );
                if (child.is_some() || stopping.is_some() || !eof || capture_done.is_some() || storage.progress.lock().unwrap().pending_bytes > 0) && !same_checkout {
                    return Err("Stop the existing session before adopting another checkout".into());
                }
                if state.execution.is_some() && child.is_none() && !same_checkout {
                    return Err("Interrupted execution is uncertain; inspect history before changing adoption".into());
                }
                if !same_checkout {
                    state.identity = None;
                    state.session_identity = None;
                    state.process_exit = None;
                    state.lifecycle = Lifecycle::Idle;
                    state.native_conversation = None;
                    state.agent = None;
                    task_identity = state.registry.checkout(&identity).map(|workspace| workspace.task.clone()).unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                }
                state.adopted = Some(path.display().to_string());
                checkout = Some(path);
                state.checkout_identity = Some(identity);
                Ok(())
            });
            if let Err(error) = result {
                state.error = Some(error);
            }
            match writes.send(
                &storage.commands,
                crate::persistence::Command::Save(crate::persistence::Snapshot::new(
                    &state,
                    checkout.as_deref(),
                    &task_identity,
                )),
            ) {
                Ok(_) => {
                    state.metadata_pending = true;
                }
                Err(error) => {
                    metadata_failed = true;
                    state.metadata_pending = true;
                    state.error = Some(format!("Metadata not saved: {error}; retry"));
                    metadata_error = state.error.clone();
                }
            }
        }
        while let Ok(event) = storage.events.try_recv() {
            use crate::persistence::Completion;
            match event {
                Completion::Registry(registry) => state.registry = registry,
                Completion::Registered(operation, result) => {
                    if registry_operation != Some(operation) {
                        continue;
                    }
                    registry_operation = None;
                    state.registry_pending = false;
                    match result {
                        Ok(registry) => state.registry = registry,
                        Err(error) => {
                            state.error = Some(format!("Project not registered: {error}; retry"))
                        }
                    }
                }
                Completion::Retried | Completion::Sealed => {}
                Completion::Cleanup(page) => state.cleanup = Some(page),
                Completion::Deleted(id) => {
                    state.cleanup_message = Some(format!(
                        "History deletion completed: {id}. Capacity may remain low; y retry capture / s stop."
                    ));
                    state.history = None;
                    state.history_index = 0;
                    history_sealed = false;
                    state.cleanup = None;
                    let _ = storage
                        .commands
                        .try_send(crate::persistence::Command::Cleanup(0));
                }
                Completion::Committed(operation) => {
                    if !writes.complete(&operation) {
                        continue;
                    }
                    let current = operation.matches(&state);
                    if current {
                        match operation.purpose {
                            crate::persistence::Purpose::Prepare => {
                                prepared_ready = !cancelled_launch && pending_launch.is_some();
                            }
                            crate::persistence::Purpose::Authorize if !cancelled_launch => {
                                if let Some(release) = gate_release.take() {
                                    let _ = release.try_send(());
                                }
                            }
                            crate::persistence::Purpose::FinalFlush => {
                                flush_pending = false;
                                final_flush = true;
                            }
                            crate::persistence::Purpose::Seal => {
                                seal_pending = false;
                                history_sealed = true;
                            }
                            crate::persistence::Purpose::LaunchResult
                                if outcome_operation.as_ref() == Some(&operation) =>
                            {
                                crate::recovery::crash_barrier(directory, "after-result");
                                outcome_operation = None;
                                outcome_reply = false;
                                result_failed = false;
                            }
                            _ => {}
                        }
                    }
                    if current
                        && !matches!(
                            operation.purpose,
                            crate::persistence::Purpose::FinalFlush
                                | crate::persistence::Purpose::Seal
                        )
                        && !writes.metadata_pending()
                        && !result_failed
                    {
                        if metadata_failed && metadata_error.as_ref() == state.error.as_ref() {
                            state.error = None;
                        }
                        metadata_failed = false;
                        metadata_error = None;
                    }
                }
                Completion::Failed(operation, error) => {
                    if !writes.complete(&operation) || !operation.matches(&state) {
                        continue;
                    }
                    match operation.purpose {
                        crate::persistence::Purpose::Prepare
                        | crate::persistence::Purpose::Authorize => {
                            metadata_failed = true;
                            pending_launch = None;
                            prepared_ready = false;
                            gate_release = None;
                        }
                        crate::persistence::Purpose::LaunchResult
                            if outcome_operation.as_ref() == Some(&operation) =>
                        {
                            metadata_failed = true;
                            outcome_operation = None;
                            result_failed = true;
                        }
                        crate::persistence::Purpose::FinalFlush => {
                            flush_pending = false;
                            final_flush = false;
                        }
                        crate::persistence::Purpose::Seal => {
                            seal_pending = false;
                            history_sealed = false;
                        }
                        _ => {
                            metadata_failed = true;
                        }
                    }
                    shutdown = false;
                    state.error = Some(format!(
                        "Launch/storage outcome not durable: {error}; stop or repair storage before retrying"
                    ));
                    if metadata_failed {
                        metadata_error = state.error.clone();
                    }
                }
                Completion::History(page) => state.history = Some(page),
                Completion::Error(kind, error) => {
                    if kind == "cleanup" {
                        state.cleanup_message = Some(format!(
                            "History cleanup failed/incomplete: {error}; no recovery claimed; g reopen / select / retry, y capture retry / s stop"
                        ));
                        continue;
                    }
                    if kind == "history" {
                        state.error = Some(error);
                        continue;
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
        if let Some(creation) = creating.as_ref()
            && let Ok(result) = creation.result.try_recv()
        {
            pending_launch = Some(creating.take().unwrap().request);
            prepared_ready = true;
            created = Some(result);
        }
        if let Some(receipt) = &gate_receipt
            && let Ok(result) = receipt.try_recv()
        {
            gate_receipt = None;
            crate::recovery::crash_barrier(directory, "before-result");
            match result {
                _ if cancelled_launch => {
                    state.error.get_or_insert_with(|| {
                        "Launch cancelled by Stop; metadata may still be pending".into()
                    });
                }
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
            match writes.send(
                &storage.commands,
                crate::persistence::Command::Save(crate::persistence::Snapshot::result(
                    &state,
                    checkout.as_deref(),
                    &task_identity,
                )),
            ) {
                Ok(operation) => {
                    outcome_operation = Some(operation);
                    outcome_reply = true;
                }
                Err(error) => {
                    metadata_failed = true;
                    result_failed = true;
                    outcome_reply = true;
                    state.error = Some(format!(
                        "Launch outcome not durable: {error}; reconcile before replacement"
                    ));
                    metadata_error = state.error.clone();
                }
            }
        }
        if let Some(done) = &capture_done
            && done.try_recv().is_ok()
        {
            eof = true;
            capture_done = None;
        }
        if retry_requested {
            if metadata_failed {
                let snapshot = if result_failed {
                    crate::persistence::Snapshot::result(
                        &state,
                        checkout.as_deref(),
                        &task_identity,
                    )
                } else {
                    crate::persistence::Snapshot::new(&state, checkout.as_deref(), &task_identity)
                };
                if let Ok(operation) = writes.send(
                    &storage.commands,
                    crate::persistence::Command::Save(snapshot),
                ) {
                    if result_failed {
                        outcome_operation = Some(operation);
                        outcome_reply = true;
                    }
                    retry_requested = false;
                }
            } else if storage
                .commands
                .try_send(crate::persistence::Command::Retry)
                .is_ok()
            {
                retry_requested = false;
            }
        }
        let progress = storage.progress.lock().unwrap().clone();
        state.capacity = progress.capacity.clone();
        state.accepted_bytes = progress.accepted;
        state.written_bytes = progress.written;
        state.storage_wait_millis = progress
            .operation
            .map_or(0, |(_, since)| since.elapsed().as_millis() as u64);
        state.producer_wait_millis = progress
            .producer_wait
            .map_or(0, |since| since.elapsed().as_millis() as u64);
        state.storage_error = progress.error.clone();
        state.capture_pending_bytes = progress.pending_bytes;
        state.capture_high_water = progress.pending_high_water;
        state.capture_backpressure_count = progress.backpressure_count;
        state.capture_backpressure_micros = progress.backpressure_micros;
        state.durable_bytes = progress.durable;
        state.durable_flush_count = progress.flush_count;
        state.durable_commits.clone_from(&progress.commits);
        state.durable_completed_millis = progress.completed_millis;
        state.sync_commit_micros = progress.last_flush_micros;
        state.accepted_to_durable_millis = progress.accepted_to_durable_millis;

        state.metadata_pending = writes.metadata_pending()
            || metadata_failed
            || pending_launch.is_some()
            || creating.is_some()
            || gate_receipt.is_some()
            || outcome_reply;
        if (shutdown
            || matches!(
                state.lifecycle,
                Lifecycle::Exited | Lifecycle::Stopped | Lifecycle::Failed
            ))
            && child.is_none()
            && stopping.is_none()
            && eof
            && !flush_pending
            && !final_flush
            && writes
                .send(
                    &storage.commands,
                    crate::persistence::Command::Flush(crate::persistence::Operation::new(
                        &state,
                        crate::persistence::Purpose::FinalFlush,
                    )),
                )
                .is_ok()
        {
            flush_pending = true;
        }
        if capture_started_here
            && !history_sealed
            && !seal_pending
            && final_flush
            && !state.metadata_pending
            && child.is_none()
            && stopping.is_none()
            && eof
            && capture_done.is_none()
            && progress.pending_bytes == 0
            && matches!(
                state.lifecycle,
                Lifecycle::Stopped | Lifecycle::Exited | Lifecycle::Failed
            )
            && state.session_identity.is_some()
            && writes
                .send(
                    &storage.commands,
                    crate::persistence::Command::Seal(crate::persistence::Operation::new(
                        &state,
                        crate::persistence::Purpose::Seal,
                    )),
                )
                .is_ok()
        {
            seal_pending = true;
        }
        state.stopped = shutdown
            && adoption.is_none()
            && discovery.is_none()
            && final_flush
            && writes.is_empty()
            && !metadata_failed
            && pending_launch.is_none()
            && creating.is_none()
            && gate_receipt.is_none()
            && !outcome_reply;
        if let Ok(error) = input_errors.try_recv() {
            state.error = Some(format!("Native input failed: {error}; stop or retry"));
        }
        if stopping.is_none()
            && state.lifecycle != Lifecycle::StopFailed
            && owned_scope.is_some()
            && let Some(active) = child.as_mut()
        {
            match active.try_wait() {
                Ok(Some(status)) => {
                    state.process_exit = Some(crate::events::ProcessExit {
                        code: status.exit_code(),
                        signal: status.signal().map(str::to_owned),
                    });
                    writer = None;
                    state.lifecycle = Lifecycle::Stopping;
                    stop_drain.store(true, Ordering::Release);
                    stopping = Some(crate::lifecycle::stop(
                        child.take().unwrap(),
                        owned_scope.as_ref().unwrap().clone(),
                    ));
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
                owned_scope = None;
                partial_gate = None;
                state.lifecycle = match state.process_exit.as_ref() {
                    Some(outcome) if outcome.failed() => Lifecycle::Failed,
                    Some(_) => Lifecycle::Exited,
                    None => Lifecycle::Stopped,
                };
                state.execution = None;
                final_flush = false;
                if writes
                    .send(
                        &storage.commands,
                        crate::persistence::Command::Save(crate::persistence::Snapshot::new(
                            &state,
                            checkout.as_deref(),
                            &task_identity,
                        )),
                    )
                    .is_ok()
                {
                    state.metadata_pending = true;
                } else {
                    metadata_failed = true;
                    state.metadata_pending = true;
                    state.error = Some("Lifecycle metadata not saved; retry quit".into());
                    metadata_error = state.error.clone();
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
        if creating.is_none()
            && created.is_none()
            && let Some(monitor) = monitor.as_mut()
        {
            state.status = monitor.poll(
                state.lifecycle == Lifecycle::Running,
                &state.screen,
                state.output_bytes,
            );
        }
        if creating.is_none()
            && created.is_none()
            && let Some(monitor) = codex_monitor.as_mut()
        {
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
            if writes
                .send(
                    &storage.commands,
                    crate::persistence::Command::Save(crate::persistence::Snapshot::new(
                        &saved,
                        checkout.as_deref(),
                        &task_identity,
                    )),
                )
                .is_ok()
            {
                state.native_conversation = saved.native_conversation;
                state.metadata_pending = true;
            }
        }
        // Accept/reject independently of the active client's commands or output.
        for _ in 0..32 {
            match listener.accept() {
                Ok((mut incoming, _)) => {
                    incoming.set_nonblocking(true)?;
                    if controller.is_some() {
                        let refusal = Observation {
                            git_jobs_protocol: 1,
                            registry_protocol: 1,
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
        if !matches!(request, Request::Screen) && !metadata_failed {
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
            // A provisional launch owns its result slot until the matching
            // result commit. Another launch/query cannot replace its request or
            // result with a cached or newly rejected request in the meantime.
            if !continuing
                && creating.is_some()
                && launch_request.is_some()
                && launch_request.as_deref() != Some(state.launch_request.as_str())
            {
                deferred = true;
                state.error = Some("Process creation still owns a launch reservation; Stop cancels intent, then wait for cleanup".into());
                return Ok(());
            }
            if !continuing
                && (pending_launch.is_some()
                    || creating.is_some()
                    || gate_receipt.is_some()
                    || outcome_reply)
                && (launch_request.is_some() || matches!(request, Request::QueryLaunch(_)))
            {
                deferred = true;
                return Ok(());
            }
            if let Some(recorded) = &recorded {
                state.launch_result = Some(recorded.clone());
                state.error = recorded.error.clone();
                return Ok(());
            }
            if !continuing
                && creating.is_some()
                && matches!(
                    request,
                    Request::Adopt(_)
                        | Request::Reconcile
                        | Request::Launch(_)
                        | Request::LaunchClaude(_)
                        | Request::LaunchCodex(_)
                        | Request::Resume(_)
                )
            {
                return Err("Process creation still owns a launch reservation; Stop cancels intent, then wait for cleanup".into());
            }
            if !continuing && child.is_some() && launch_request.is_some() {
                return Err("A session already exists; attach to it".into());
            }
            if adoption.is_some() && launch_request.is_some() {
                return Err("Adoption pending; wait for the checkout result before launch".into());
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
                Request::Discover { request, repository } => {
                    if repository.len() > 32768 { return Err("Repository path exceeds 32768 bytes".into()); }
                    let repository = PathBuf::from(std::ffi::OsString::from_vec(repository));
                    if discovery.is_none() {
                        discovery = Some(crate::git::GitWorker::start(repository.clone())?);
                    }
                    if let Err(error) = discovery.as_ref().unwrap().refresh_for(request, repository.clone()) {
                        state.discovery = Some(crate::git::Discovery { request, repository: path_bytes(&repository), result: Err(error) });
                        return Ok(());
                    }
                    pending_discovery = Some((request, repository));
                    state.discovery = None;
                }
                Request::Register(bytes) => {
                    if bytes.len() > 32768 { return Err("Repository path exceeds 32768 bytes".into()); }
                    if adoption.is_some() || registry_operation.is_some() { return Err("Project operation pending; wait or Stop to cancel".into()); }
                    adoption = Some(crate::git::AdoptionJob::start(PathBuf::from(std::ffi::OsString::from_vec(bytes)))?);
                    registering = true;
                    state.adoption_pending = true;
                    state.registry_pending = true;
                }
                Request::Adopt(bytes) => {
                    registering = false;
                    if bytes.len() > 32768 { return Err("Checkout path exceeds 32768 bytes".into()); }
                    if adoption.is_some() || registry_operation.is_some() { return Err("Adoption already pending; wait or Stop to cancel".into()); }
                    adoption = Some(crate::git::AdoptionJob::start(PathBuf::from(std::ffi::OsString::from_vec(bytes)))?);
                    state.adoption_pending = true;
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
                    if !continuing && child.is_none() && stopping.is_none() && (!eof || capture_done.is_some() || flush_pending || storage.progress.lock().unwrap().pending_bytes > 0 || metadata_failed || (matches!(state.lifecycle, Lifecycle::Stopped | Lifecycle::Exited | Lifecycle::Failed) && !final_flush)) {
                        return Err("Previous session history is still pending; repair storage and press y to retry before starting another session".into());
                    }
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
                        capture_started_here = false;
                        cancelled_launch = false;
                        state.capture_loss_possible = false;
                        if !resume { state.native_conversation = None; }
                        state.execution = Some(crate::recovery::Execution::prepared()?);
                        state.reconciled = false;
                        let session = uuid::Uuid::new_v4().to_string();
                        let identity = crate::events::SessionIdentity { task: task_identity.clone(), session, request: request.clone() };
                        state.identity = Some(identity.label());
                        state.session_identity = Some(identity);
                        state.process_exit = None;
                        state.launch_result = None;
                        state.lifecycle = Lifecycle::Interrupted;
                        writes.send(&storage.commands, crate::persistence::Command::Prepare(crate::persistence::Snapshot::new(&state, Some(path), &task_identity)))?;
                        pending_launch = Some(if resume { Request::Resume(request) } else if claude { Request::LaunchClaude(request) } else if codex { Request::LaunchCodex(request) } else { Request::Launch(request) });
                        deferred = true;
                        return Ok(());
                    }
                    let CreatedExecution { pty, child: launched, listener: gate_listener, nonce } = if let Some(result) = created.take() {
                        result.map_err(io::Error::other)?
                    } else {
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
                    #[cfg(debug_assertions)]
                    if !claude && !codex { command.env("RAT_PROOF_STORAGE_DIR", directory); }
                    command.env("TERM", "xterm-256color");
                    command.cwd(path);
                    crate::recovery::wrap(&mut command, &gate_path, &nonce)?;
                    let (finished, result) = mpsc::sync_channel(1);
                    let proof_creation = directory.join("proof-launch-creation");
                    thread::Builder::new().name("session-create".into()).spawn(move || {
                        let result = pty.slave.spawn_command(command).map(|child| {
                            // No fallible setup occurs between successful creation
                            // and installation in this resource owner.
                            let execution = CreatedExecution { pty, child, listener: gate_listener, nonce };
                            #[cfg(debug_assertions)]
                            if proof_creation.exists() {
                                let _ = fs::write(proof_creation.with_extension("ready"), execution.child.process_id().unwrap_or(0).to_string());
                                while proof_creation.exists() { thread::sleep(Duration::from_millis(10)); }
                            }
                            let _ = proof_creation;
                            execution
                        }).map_err(|error| {
                            if claude {
                                format!("Claude Code launch failed: {error}. Install claude on PATH, run claude auth login in your usual terminal, then explicitly retry c; adopted checkout retained")
                            } else if codex {
                                format!("Codex CLI launch failed: {error}. Install codex on PATH, run codex login in your usual terminal, then explicitly retry x; adopted checkout retained")
                            } else {
                                format!("Fixture launch failed: {error}; retry l")
                            }
                        });
                        let _ = finished.send(result);
                    })?;
                    creating = Some(CreatingExecution { request: if resume { Request::Resume(request) } else if claude { Request::LaunchClaude(request) } else if codex { Request::LaunchCodex(request) } else { Request::Launch(request) }, result });
                    deferred = true;
                    return Ok(());
                    };
                    // Transfer the actual child and terminal before any fallible
                    // identity, input or capture installation can return early.
                    child = Some(launched);
                    partial_gate = Some(gate_listener);
                    drop(pty.slave);
                    master = Some(Arc::new(Mutex::new(pty.master)));
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
                    let process_group = child.as_ref().unwrap()
                        .process_id()
                        .ok_or("Missing managed process identity")?
                        .try_into()?;
                    state.execution.as_mut().ok_or("Missing durable intent")?.pid = Some(process_group);
                    launch_setup_fault(directory, "before-scope")?;
                    owned_scope = Some(crate::child_scope::Scope::new(process_group)?);
                    state.execution.as_mut().ok_or("Missing durable intent")?.identify(process_group)?;
                    launch_setup_fault(directory, "after-create")?;
                    let (release, authorized) = mpsc::sync_channel(1);
                    let (receipt, result) = mpsc::sync_channel(1);
                    gate_release = Some(release);
                    gate_receipt = Some(result);
                    let gate_listener = partial_gate.take().unwrap();
                    let proof_receipt_pause = directory.join("proof-launch-receipt");
                    let gate_path = directory.join("launch-gate.sock");
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
                        #[cfg(debug_assertions)]
                        while proof_receipt_pause.exists() { thread::sleep(Duration::from_millis(10)); }
                        let _ = proof_receipt_pause;
                        let _ = receipt.send(result);
                    });
                    launch_setup_fault(directory, "before-input")?;
                    let mut sink = master.as_ref().unwrap().lock().unwrap().take_writer()?;
                    let (input, input_writes) = mpsc::sync_channel::<Vec<u8>>(32);
                    let errors = write_errors.clone();
                    thread::spawn(move || {
                        while let Ok(bytes) = input_writes.recv() {
                            if let Err(error) = sink.write_all(&bytes) {
                                let _ = errors.try_send(error.to_string());
                                break;
                            }
                        }
                    });
                    writer = Some(input);
                    state.lifecycle = Lifecycle::Running;
                    state.stopped = false;
                    state.history_index = 0;
                    history_sealed = false;
                    seal_pending = false;
                    let session = state.session_identity.as_ref().ok_or("Missing prepared identity")?.session.clone();
                    let capture = storage.output.clone();
                    launch_setup_fault(directory, "before-capture")?;
                    let mut source = master.as_ref().unwrap().lock().unwrap().try_clone_reader()?;
                    capture_started_here = true;
                    let (finished, done) = mpsc::sync_channel(1);
                    capture_done = Some(done);
                    eof = false;
                    final_flush = false;
                    let parser = screen.clone();
                    stop_drain.store(false, Ordering::Release);
                    let drain = stop_drain.clone();
                    thread::spawn(move || {
                        // A macOS PTY leader can await terminal output drain even
                        // after SIGKILL. Stop alone reserves a finite 1MiB raw tail;
                        // the single reader preserves byte order and retries it
                        // before reporting EOF. Exhaustion keeps backpressure and
                        // bounded lifecycle verification still reports failure.
                        const STOP_TAIL: usize = 1024 * 1024;
                        const STOP_BLOCKS: usize = 256;
                        let mut tail = VecDeque::new();
                        let mut retained = 0;
                        let mut bytes = [0; 4096];
                        while let Ok(n) = source.read(&mut bytes) {
                            if n == 0 {
                                break;
                            }
                            capture.accept(n);
                            let block = (session.clone(), bytes[..n].to_vec(), std::time::Instant::now());
                            if tail.is_empty() {
                                match capture.send(block, Some(&drain)) {
                                    Ok(Some(block)) => { retained += n; tail.push_back(block); }
                                    Ok(None) => {}
                                    Err(()) => break,
                                }
                            } else {
                                retained += n;
                                tail.push_back(block);
                            }
                            if retained >= STOP_TAIL || tail.len() >= STOP_BLOCKS {
                                let block = tail.pop_front().unwrap();
                                retained -= block.1.len();
                                if capture.send(block, None).is_err() { break; }
                            }
                            let mut screen = parser.lock().unwrap();
                            screen.parser.process(&bytes[..n]);
                            screen.output_bytes = screen.output_bytes.saturating_add(n as u64);
                        }
                        for block in tail {
                            if capture.send(block, None).is_err() { return; }
                        }
                        let _ = finished.send(());
                    });
                    launch_setup_fault(directory, "before-authorization")?;
                    if cancelled_launch {
                        gate_release = None;
                        writer = None;
                        state.lifecycle = Lifecycle::Stopping;
                        stop_drain.store(true, Ordering::Release);
                        stopping = Some(crate::lifecycle::stop(child.take().unwrap(), owned_scope.as_ref().unwrap().clone()));
                        return Err("Launch cancelled by Stop before authorization".into());
                    }
                    writes.send(&storage.commands, crate::persistence::Command::Authorize(crate::persistence::Snapshot::new(&state, Some(path), &task_identity)))?;
                    deferred = true;
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
                Request::HistorySession { session, offset } => {
                    storage.commands.try_send(crate::persistence::Command::HistorySession(session, offset)).map_err(|error| format!("History busy: {error}; retry"))?;
                }
                Request::History { index, offset } => {
                    state.history_index = index;
                    storage.commands.try_send(crate::persistence::Command::History(index, offset)).map_err(|e| format!("History busy: {e}; retry"))?;
                }
                Request::Screen => {}
                Request::Cleanup { page } => {
                    state.cleanup = None;
                    storage.commands.try_send(crate::persistence::Command::Cleanup(page)).map_err(|e| format!("Cleanup busy: {e}; retry"))?;
                }
                Request::DeleteHistory { id } => {
                    if state.session_identity.as_ref().map(|identity| identity.session.as_str()) == Some(&id)
                        && (child.is_some() || stopping.is_some() || !eof || capture_done.is_some() || !final_flush || state.metadata_pending || progress.pending_bytes > 0)
                    { return Err("Selected history still has active, uncertain or unflushed capture".into()); }
                    state.cleanup_message = Some(format!("Deleting explicitly selected history {id}; completion pending"));
                    storage.commands.try_send(crate::persistence::Command::Delete(id)).map_err(|e| format!("Cleanup busy: {e}; retry"))?;
                }
                Request::RetryStorage => {
                    history_sealed = false;
                    retry_requested = true;
                    final_flush = false;
                }
                Request::Stop | Request::Shutdown => {
                    if let Some(job) = &adoption { job.cancel(); }
                    state.adoption_pending = false;
                    if registering { state.registry_pending = false; registering = false; }
                    if let Some(worker) = &discovery { worker.cancel(); }
                    if let Some((request, repository)) = pending_discovery.take() {
                        state.discovery = Some(crate::git::Discovery { request, repository: path_bytes(&repository), result: Err("Git discovery cancelled by Stop".into()) });
                    }
                    if state.storage_wait_millis >= 250 || state.producer_wait_millis > 0 || state.storage_error.is_some() { state.capture_loss_possible = true; }
                    if (pending_launch.is_some() || gate_receipt.is_some()) && creating.is_none() {
                        let result = LaunchResult { request: state.launch_request.clone(), identity: state.identity.clone(), error: Some("Launch cancelled by Stop; metadata may still be pending".into()) };
                        launches.insert(result.request.clone(), result.clone());
                        state.launch_result = Some(result);
                    }
                    let cancelled_preparation = pending_launch.is_some() && creating.is_none() && child.is_none() && gate_receipt.is_none();
                    cancelled_launch = true;
                    pending_launch = None;
                    prepared_ready = false;
                    if cancelled_preparation { state.execution = None; }
                    gate_release = None;
                    if creating.is_none() && child.is_none() && stopping.is_none() && state.execution.is_some() {
                        return Err("Interrupted execution remains uncertain; u reconciles evidence, d detaches. A missing native leader may require verified reboot before replacement or confirmed shutdown.".into());
                    }
                    shutdown |= matches!(request, Request::Shutdown);
                    if stopping.is_none() {
                        if owned_scope.is_none() && let Some(active) = child.as_ref() {
                            let pid = active.process_id().ok_or("Missing managed process identity; stop unconfirmed")?.try_into()?;
                            owned_scope = Some(crate::child_scope::Scope::new(pid).map_err(|error| format!("Stop unconfirmed: {error}; child retained; retry Stop"))?);
                        }
                        if let Some(active) = child.take() {
                            writer = None;
                                    state.lifecycle = Lifecycle::Stopping;
                            stop_drain.store(true, Ordering::Release);
                            stopping = Some(crate::lifecycle::stop(active, owned_scope.as_ref().unwrap().clone()));
                        } else {
                            final_flush = false;
                        }
                    }
                    if creating.is_some() {
                        state.lifecycle = Lifecycle::Stopping;
                        state.error = Some("Launch cancelled by Stop; process creation still outstanding, cleanup pending".into());
                    }
                }
            }
            Ok(())
        })();
        if let Err(error) = result {
            // A failed setup must never release its gated child, including a
            // delayed authorization completion from the storage worker.
            if continuing && launch_request.is_some() && !deferred {
                cancelled_launch = true;
                gate_release = None;
            }
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
            let snapshot = if fresh_launch {
                crate::persistence::Snapshot::result(&state, checkout.as_deref(), &task_identity)
            } else {
                crate::persistence::Snapshot::new(&state, checkout.as_deref(), &task_identity)
            };
            match writes.send(
                &storage.commands,
                crate::persistence::Command::Save(snapshot),
            ) {
                Ok(operation) => {
                    state.metadata_pending = true;
                    if fresh_launch {
                        outcome_operation = Some(operation);
                        outcome_reply = true;
                    }
                }
                Err(error) => {
                    metadata_failed = true;
                    if fresh_launch {
                        result_failed = true;
                        outcome_reply = true;
                    }
                    state.metadata_pending = true;
                    state.error = Some(format!("Metadata not saved: {error}; retry"));
                    metadata_error = state.error.clone();
                    shutdown = false;
                }
            }
        }
        screen.lock().unwrap().snapshot(&mut state);
        if creating.is_none()
            && created.is_none()
            && let Some(monitor) = monitor.as_mut()
        {
            state.status = monitor.poll(
                state.lifecycle == Lifecycle::Running,
                &state.screen,
                state.output_bytes,
            );
        }
        if !continuing && controller.is_some() {
            state.metadata_pending = writes.metadata_pending()
                || metadata_failed
                || pending_launch.is_some()
                || creating.is_some()
                || gate_receipt.is_some()
                || outcome_reply;
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
    let mut paced: Option<std::thread::JoinHandle<io::Result<()>>> = None;
    for byte in io::stdin().lock().bytes() {
        let byte = byte?;
        match byte {
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
                    if let Some(producer) = paced.take() {
                        producer
                            .join()
                            .map_err(|_| io::Error::other("Paced producer panicked"))??;
                    }
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
                } else if line == b"tool-child" {
                    use std::os::unix::process::CommandExt;
                    let mut command = Command::new("python3");
                    command.args(["-c", "import os,signal,time\nsignal.signal(signal.SIGHUP,signal.SIG_IGN)\nsignal.signal(signal.SIGTERM,signal.SIG_IGN)\nopen('tool-child.pid','w').write(str(os.getpid()))\nfor i in range(300):\n with open('tool-child.heartbeat','a') as f: f.write('beat\\n')\n time.sleep(.02)\n"]);
                    command.process_group(0);
                    command
                        .stdin(std::process::Stdio::null())
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null());
                    let mut descendant = command.spawn()?;
                    thread::spawn(move || {
                        let _ = descendant.wait();
                    });
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
                } else if line == b"status-native-active"
                    || line == b"status-native-old-ack"
                    || line == b"status-native-tool-active"
                {
                    native_active = true;
                    if line == b"status-native-tool-active" {
                        crate::native_status::fixture("FixtureNativeBashPre");
                    }
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
                } else if line == b"status-native-done" || line == b"status-native-quoted-done" {
                    if line == b"status-native-done" {
                        crate::native_status::fixture("Stop");
                    }
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
                        b"status-old-stop" => "FixtureOldStop",
                        b"status-prior-turn-stop" => "FixturePreviousTurnStop",
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
                } else if line == b"osc13" {
                    let mut stdout = io::stdout().lock();
                    stdout.write_all(b"\x1b]2;")?;
                    for _ in 0..2048 {
                        stdout.write_all(&[b'q'; 4096])?;
                    }
                    stdout.write_all(b"\x07osc13-complete\r\nfixture> ")?;
                } else if let Some(length) = line.strip_prefix(b"pad14 ") {
                    let length = std::str::from_utf8(length)
                        .ok()
                        .and_then(|value| value.parse::<usize>().ok())
                        .filter(|length| *length <= 65536)
                        .ok_or_else(|| io::Error::other("Invalid proof padding"))?;
                    io::stdout().write_all(&vec![b'R'; length])?;
                    print!("fixture> ");
                } else if let Some(config) = line.strip_prefix(b"pace17 ") {
                    if paced
                        .as_ref()
                        .is_some_and(|producer| !producer.is_finished())
                    {
                        println!("pace17 busy; one producer maximum\r");
                        line.clear();
                        continue;
                    }
                    if let Some(previous) = paced.take() {
                        previous
                            .join()
                            .map_err(|_| io::Error::other("Paced producer panicked"))??;
                    }
                    let values: Vec<u64> = std::str::from_utf8(config)
                        .map_err(io::Error::other)?
                        .split_whitespace()
                        .map(str::parse)
                        .collect::<std::result::Result<_, _>>()
                        .map_err(io::Error::other)?;
                    if values.len() != 3
                        || values[1] > 300_000
                        || !(1024..=4_194_304).contains(&values[2])
                    {
                        return Err(io::Error::other(
                            "pace17 requires run duration_ms(<=300000) bytes_per_second(1024..4194304)",
                        ));
                    }
                    paced = Some(thread::spawn(move || {
                        paced_fixture(values[0], values[1], values[2])
                    }));
                } else if let Some(nonce) = line.strip_prefix(b"ack17 ") {
                    // Input receipt precedes stdout acquisition: full storage can
                    // block its visible ACK without blocking receipt itself.
                    let received = fixture_clock()?;
                    let nonce = String::from_utf8_lossy(nonce);
                    if let Some(mut ledger) = fixture_ledger("proof-input17.jsonl") {
                        writeln!(
                            ledger,
                            "{}",
                            serde_json::json!({"nonce":nonce,"received_ns":received})
                        )?;
                    }
                    let mut stdout = io::stdout().lock();
                    write!(
                        stdout,
                        "\x1b[s\x1b[1;1H\x1b[2KACK17 {nonce} received_ns={received}\x1b[u"
                    )?;
                    stdout.flush()?;
                } else if line == b"flood13" {
                    flood_run += 1;
                    let start_ns = fixture_clock()?;
                    let since = Instant::now();
                    let mut max_write_micros = 0;
                    let payload = "X".repeat(111);
                    let mut stdout = io::stdout().lock();
                    let mut ledger = std::env::var_os("RAT_PROOF_STORAGE_DIR")
                        .map(PathBuf::from)
                        .filter(|path| path.join("proof-storage-ledger").is_file())
                        .and_then(|path| {
                            OpenOptions::new()
                                .create(true)
                                .append(true)
                                .open(path.join("proof-emitted.jsonl"))
                                .ok()
                        });
                    for number in 0..32768 {
                        let record = format!("flood13:{number:06}:{payload}\r\n");
                        let write = Instant::now();
                        stdout.write_all(record.as_bytes())?;
                        let micros = write.elapsed().as_micros();
                        max_write_micros = max_write_micros.max(micros);
                        if let Some(ledger) = ledger.as_mut() {
                            // Format the complete bounded receipt before touching
                            // the file: formatting Value directly into File emits
                            // many writes that Stop can interrupt inside the JSON.
                            let receipt = format!(
                                "{}\n",
                                serde_json::json!({"record":number,"bytes":record.len(),"write_us":micros,"elapsed_us":since.elapsed().as_micros()})
                            );
                            ledger.write_all(receipt.as_bytes())?;
                        }
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
        // Ordinary input collection must not acquire stdout: a producer can
        // legitimately be blocked there by preserved-history backpressure.
        if matches!(byte, 3 | b'\r') {
            io::stdout().flush()?;
        }
    }
    print!("\x1b[?2004l");
    io::stdout().flush()
}

fn fixture_ledger(name: &str) -> Option<std::fs::File> {
    std::env::var_os("RAT_PROOF_STORAGE_DIR")
        .map(PathBuf::from)
        .filter(|path| path.join("proof-storage-ledger").is_file())
        .and_then(|path| {
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(path.join(name))
                .ok()
        })
}

// A native-style producer: one finite worker, one <=1024-byte stdout lock,
// pacing outside that lock, and no catch-up burst after backpressure.
fn paced_fixture(run: u64, duration_ms: u64, rate: u64) -> io::Result<()> {
    let started = Instant::now();
    let start_ns = fixture_clock()?;
    let period = Duration::from_secs_f64(1024.0 / rate as f64);
    let mut ledger = fixture_ledger("proof-emitted17.jsonl");
    let mut sequence = 0u64;
    let mut max_write_us = 0;
    let mut write_us = 0;
    let mut ledger_us = 0;
    let mut max_ledger_us = 0;
    while started.elapsed() < Duration::from_millis(duration_ms) {
        let cycle = Instant::now();
        let mut record = format!(
            "\x1b[2;1H\x1b[2Kpace17 run={run} seq={sequence:08}\x1b[3;1H\x1b[32mλ界\x1b[0m"
        )
        .into_bytes();
        record.resize(1024, b'X');
        let before = Instant::now();
        {
            let mut stdout = io::stdout().lock();
            stdout.write_all(&record)?;
            stdout.flush()?;
        }
        let elapsed = before.elapsed().as_micros() as u64;
        max_write_us = max_write_us.max(elapsed);
        write_us += elapsed;
        if let Some(ledger) = ledger.as_mut() {
            let cost = Instant::now();
            writeln!(
                ledger,
                "{}",
                serde_json::json!({"run":run,"record":sequence,"bytes":1024,"write_us":elapsed,"completed_ns":fixture_clock()?})
            )?;
            let cost = cost.elapsed().as_micros() as u64;
            ledger_us += cost;
            max_ledger_us = max_ledger_us.max(cost);
        }
        sequence += 1;
        if let Some(rest) = period.checked_sub(cycle.elapsed()) {
            thread::sleep(rest);
        }
    }
    let finish_ns = fixture_clock()?;
    let mut stdout = io::stdout().lock();
    write!(
        stdout,
        "\x1b[2;1H\x1b[2Kpace17-complete run={run} records={sequence} bytes={} start_ns={start_ns} finish_ns={finish_ns} write_us={write_us} max_write_us={max_write_us}\r\nfixture> ",
        sequence * 1024
    )?;
    stdout.flush()?;
    if let Some(ledger) = ledger.as_mut() {
        writeln!(
            ledger,
            "{}",
            serde_json::json!({"kind":"complete","run":run,"records":sequence,"bytes":sequence*1024,"start_ns":start_ns,"finish_ns":finish_ns,"write_us":write_us,"max_write_us":max_write_us,"ledger_us":ledger_us,"max_ledger_us":max_ledger_us})
        )?;
    }
    Ok(())
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

fn launch_setup_fault(directory: &Path, boundary: &str) -> io::Result<()> {
    #[cfg(debug_assertions)]
    if fs::read_to_string(directory.join("proof-launch-setup"))
        .is_ok_and(|value| value.trim() == boundary)
    {
        return Err(io::Error::other(format!(
            "Injected launch setup failure: {boundary}"
        )));
    }
    let _ = (directory, boundary);
    Ok(())
}
