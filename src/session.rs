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
    time::Duration,
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
    QueryLaunch(String),
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
    pub launch_request: String,
    #[serde(default)]
    pub launch_result: Option<LaunchResult>,
    pub adopted: Option<String>,
    pub identity: Option<String>,
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
                        error: Some(error.to_string()),
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
    let mut state = Observation {
        launch_request: uuid::Uuid::new_v4().to_string(),
        ..Observation::default()
    };
    // Retain every result for this owner lifetime. Exhaustion fails closed rather
    // than evicting an outcome and making a retry execute twice.
    let mut launches: HashMap<String, LaunchResult> = HashMap::new();
    let mut checkout = None;
    let mut task_identity = uuid::Uuid::new_v4().to_string();
    let mut child: Option<Box<dyn portable_pty::Child + Send + Sync>> = None;
    let mut writer: Option<SyncSender<Vec<u8>>> = None;
    let (write_errors, input_errors) = mpsc::sync_channel(1);
    let mut process_group = 0;
    let mut stopping: Option<Receiver<crate::lifecycle::Outcome>> = None;
    let mut shutdown = false;
    let mut native_exit = false;
    let screen = Arc::new(Mutex::new(ScreenState {
        parser: vt100::Parser::new(ROWS.try_into().unwrap(), COLS.try_into().unwrap(), 0),
        output_bytes: 0,
    }));
    let mut master: Option<Arc<Mutex<Box<dyn portable_pty::MasterPty + Send>>>> = None;
    let mut controller: Option<UnixStream> = None;
    let mut pending = Vec::new();
    loop {
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
                state.stopped = shutdown;
            }
        }
        if state.stopped && controller.is_none() {
            fs::remove_file(&socket)?;
            return Ok(());
        }
        // Accept/reject independently of the active client's commands or output.
        for _ in 0..32 {
            match listener.accept() {
                Ok((mut incoming, _)) => {
                    incoming.set_write_timeout(Some(Duration::from_millis(100)))?;
                    if controller.is_some() {
                        let refusal = Observation {
                            control_denied: true,
                            error: Some("Another dashboard controls this supervisor; close it before reconnecting".into()),
                            ..Observation::default()
                        };
                        let _ = send_observation(&mut incoming, &refusal);
                    } else {
                        screen.lock().unwrap().snapshot(&mut state);
                        if send_observation(&mut incoming, &state).is_ok() {
                            incoming.set_nonblocking(true)?;
                            controller = Some(incoming);
                            pending.clear();
                        }
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) => return Err(error.into()),
            }
        }
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
        let Some(end) = pending.iter().position(|byte| *byte == b'\n') else {
            thread::sleep(Duration::from_millis(5));
            continue;
        };
        let bytes: Vec<_> = pending.drain(..=end).collect();
        let request: Request = match serde_json::from_slice(&bytes) {
            Ok(request) => request,
            Err(_) => {
                controller = None;
                pending.clear();
                continue;
            }
        };
        if !matches!(request, Request::Screen) {
            state.error = None;
        }
        let launch_request = match &request {
            Request::Launch(id) => Some(id.clone()),
            _ => None,
        };
        let recorded = match &request {
            Request::Launch(id) | Request::QueryLaunch(id) => launches.get(id).cloned(),
            _ => None,
        };
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            if let Some(recorded) = &recorded {
                state.launch_result = Some(recorded.clone());
                state.error = recorded.error.clone();
                return Ok(());
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
                    if checkout.as_ref() != Some(&path) {
                        task_identity = uuid::Uuid::new_v4().to_string();
                    }
                    state.adopted = Some(path.display().to_string());
                    checkout = Some(path);
                }
                Request::QueryLaunch(_) => return Err("Launch request unknown to this supervisor; do not replay uncertain activity after supervisor loss".into()),
                Request::Launch(request) => {
                    if request.is_empty() || request.len() > 128 {
                        return Err("Launch request must contain 1–128 bytes".into());
                    }
                    if launches.len() >= 1024 {
                        return Err("Launch request capacity reached; stop and restart the supervisor explicitly".into());
                    }
                    state.launch_request = request.clone();
                    if child.is_some() || stopping.is_some() {
                        return Err("A session already exists; attach to it".into());
                    }
                    let path = checkout.as_ref().ok_or("Adopt a checkout before launch")?;
                    let (rows, cols) = screen.lock().unwrap().parser.screen().size();
                    let pty = native_pty_system().openpty(PtySize {
                        rows: rows.get(),
                        cols: cols.get(),
                        pixel_width: 0,
                        pixel_height: 0,
                    })?;
                    let mut command = CommandBuilder::new(std::env::current_exe()?);
                    command.arg("--scripted-child");
                    command.cwd(path);
                    let launched = pty.slave.spawn_command(command)?;
                    drop(pty.slave);
                    process_group = launched
                        .process_id()
                        .ok_or("Missing managed process identity")?
                        .try_into()?;
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
                    let mut source = pty.master.try_clone_reader()?;
                    master = Some(Arc::new(Mutex::new(pty.master)));
                    let parser = screen.clone();
                    thread::spawn(move || {
                        let mut bytes = [0; 4096];
                        while let Ok(n) = source.read(&mut bytes) {
                            if n == 0 {
                                break;
                            }
                            let mut screen = parser.lock().unwrap();
                            screen.parser.process(&bytes[..n]);
                            screen.output_bytes = screen.output_bytes.saturating_add(n as u64);
                        }
                    });
                    state.identity = Some(format!(
                        "task={} session={} request={request}",
                        task_identity,
                        uuid::Uuid::new_v4()
                    ));
                    child = Some(launched);
                }
                Request::Input(bytes) => {
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
                Request::Screen => {}
                Request::Stop | Request::Shutdown => {
                    shutdown |= matches!(request, Request::Shutdown);
                    if stopping.is_none() {
                        if let Some(active) = child.take() {
                            writer = None;
                            native_exit = false;
                            state.lifecycle = Lifecycle::Stopping;
                            stopping = Some(crate::lifecycle::stop(active, process_group));
                        } else {
                            state.stopped = shutdown;
                        }
                    }
                }
            }
            Ok(())
        })();
        if let Err(error) = result {
            state.error = Some(error.to_string());
        }
        if let Some(request) = launch_request
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
        screen.lock().unwrap().snapshot(&mut state);
        if send_observation(stream, &state).is_err() {
            controller = None;
            pending.clear();
        }
        if state.stopped {
            fs::remove_file(&socket)?;
            return Ok(());
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
    for byte in io::stdin().lock().bytes() {
        match byte? {
            3 => {
                line.clear();
                println!("interrupted: 03\r");
                print!("fixture> ");
            }
            b'\r' => {
                if line == b"exit" {
                    break;
                }
                if line == b"no-read" {
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
