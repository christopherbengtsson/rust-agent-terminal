//! Thin scripted-session proof. All blocking transport and PTY operations live here.
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use serde::{Deserialize, Serialize};
use std::{
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
const MAX_FRAME: usize = 65536;
#[derive(Debug, Serialize, Deserialize)]
pub enum Request {
    Adopt(Vec<u8>),
    Launch(String),
    Input(Vec<u8>),
    Screen,
    Shutdown,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Observation {
    pub adopted: Option<String>,
    pub identity: Option<String>,
    pub screen: String,
    pub error: Option<String>,
    pub stopped: bool,
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
    let greeting: Observation = serde_json::from_slice(&read_frame(&mut reader)?)?;
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
        let shutdown = matches!(request, Request::Shutdown);
        serde_json::to_writer(&mut stream, &request)?;
        stream.write_all(b"\n")?;
        let observation: Observation = serde_json::from_slice(&read_frame(&mut reader)?)?;
        if shutdown {
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
    let mut state = Observation::default();
    let mut checkout = None;
    let mut child: Option<Box<dyn portable_pty::Child + Send + Sync>> = None;
    let mut writer: Option<Box<dyn Write + Send>> = None;
    let screen = Arc::new(Mutex::new(vt100::Parser::new(
        ROWS.try_into().unwrap(),
        COLS.try_into().unwrap(),
        0,
    )));
    let mut controller: Option<UnixStream> = None;
    let mut pending = Vec::new();
    loop {
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
                        state.screen = screen.lock().unwrap().screen().contents();
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
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
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
                    if child.is_some() && checkout.as_ref() != Some(&path) {
                        return Err(
                            "Stop the existing session before adopting another checkout".into()
                        );
                    }
                    state.adopted = Some(path.display().to_string());
                    checkout = Some(path);
                }
                Request::Launch(request) => {
                    if child.is_some() {
                        return Err("A session already exists; attach to it".into());
                    }
                    let path = checkout.as_ref().ok_or("Adopt a checkout before launch")?;
                    let pty = native_pty_system().openpty(PtySize {
                        rows: ROWS,
                        cols: COLS,
                        pixel_width: 0,
                        pixel_height: 0,
                    })?;
                    let mut command = CommandBuilder::new(std::env::current_exe()?);
                    command.arg("--scripted-child");
                    command.cwd(path);
                    let launched = pty.slave.spawn_command(command)?;
                    drop(pty.slave);
                    writer = Some(pty.master.take_writer()?);
                    let mut source = pty.master.try_clone_reader()?;
                    let parser = screen.clone();
                    thread::spawn(move || {
                        let _master = pty.master;
                        let mut bytes = [0; 4096];
                        while let Ok(n) = source.read(&mut bytes) {
                            if n == 0 {
                                break;
                            }
                            parser.lock().unwrap().process(&bytes[..n]);
                        }
                    });
                    state.identity = Some(format!(
                        "task={} session={} request={request}",
                        uuid::Uuid::new_v4(),
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
                        .write_all(&bytes)?;
                }
                Request::Screen => {}
                Request::Shutdown => {
                    if let Some(child) = child.as_mut() {
                        child.kill()?;
                        child.wait()?;
                    }
                    state.stopped = true;
                }
            }
            Ok(())
        })();
        if let Err(error) = result {
            state.error = Some(error.to_string());
        }
        state.screen = screen.lock().unwrap().screen().contents();
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
    println!(
        "Scripted PTY · pid={} · cwd={}\r",
        std::process::id(),
        std::env::current_dir()?.display()
    );
    print!("fixture> ");
    io::stdout().flush()?;
    for line in io::stdin().lock().lines() {
        let line = line?;
        if line == "exit" {
            break;
        }
        println!("received: {line}\r");
        print!("fixture> ");
        io::stdout().flush()?;
    }
    Ok(())
}
