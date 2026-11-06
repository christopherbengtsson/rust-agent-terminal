//! Thin scripted-session proof. All blocking transport and PTY operations live here.
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, BufRead, BufReader, Read, Write},
    os::unix::{
        ffi::{OsStrExt, OsStringExt},
        fs::PermissionsExt,
        net::{UnixListener, UnixStream},
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
    Key(crossterm::event::KeyEvent),
    Paste(String),
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
    fs::create_dir_all(&directory)?;
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))?;
    let socket = directory.join("supervisor.sock");
    if !socket.exists() {
        Command::new(std::env::current_exe()?)
            .arg("--supervisor")
            .arg(&directory)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
    }
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
        .ok_or("Cannot connect to prototype supervisor; inspect/remove stale state directory")?;
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;
    let mut reader = BufReader::new(stream.try_clone()?);
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

pub fn supervisor(directory: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let socket = directory.join("supervisor.sock");
    let listener = UnixListener::bind(&socket)?;
    fs::set_permissions(&socket, fs::Permissions::from_mode(0o600))?;
    let mut state = Observation::default();
    let mut checkout = None;
    let mut child: Option<Box<dyn portable_pty::Child + Send + Sync>> = None;
    let mut writer: Option<Box<dyn Write + Send>> = None;
    let screen = Arc::new(Mutex::new(vt100::Parser::new(
        ROWS.try_into().unwrap(),
        COLS.try_into().unwrap(),
        0,
    )));
    // The prototype accepts one controller. Durable ownership/reconnection follows separately.
    for connection in listener.incoming() {
        let mut stream = connection?;
        let mut reader = BufReader::new(stream.try_clone()?);
        while let Ok(bytes) = read_frame(&mut reader) {
            let request: Request = serde_json::from_slice(&bytes)?;
            if !matches!(request, Request::Screen) {
                state.error = None;
            }
            let result = (|| -> Result<(), Box<dyn std::error::Error>> {
                match request {
                    Request::Adopt(bytes) => {
                        if child.is_some() {
                            return Err(
                                "Stop the existing session before adopting another checkout".into(),
                            );
                        }
                        let path =
                            PathBuf::from(std::ffi::OsString::from_vec(bytes)).canonicalize().map_err(|error| format!("Checkout unavailable: {error}; refresh and choose an existing working checkout"))?;
                        let output = Command::new("git")
                            .arg("-C")
                            .arg(&path)
                            .args(["rev-parse", "--is-inside-work-tree"])
                            .output()?;
                        if !output.status.success() || output.stdout != b"true\n" {
                            return Err("Checkout unavailable or bare; choose an existing working checkout and refresh".into());
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
                    Request::Key(key) => {
                        let application_cursor =
                            screen.lock().unwrap().screen().application_cursor();
                        let bytes = crate::native_input::encode(key, application_cursor)?;
                        writer
                            .as_mut()
                            .ok_or("No session; launch first")?
                            .write_all(&bytes)?;
                    }
                    Request::Paste(text) => {
                        if text.len() > 4084 {
                            return Err("Paste exceeds 4084-byte limit".into());
                        }
                        let bracketed = screen.lock().unwrap().screen().bracketed_paste();
                        let bytes = if bracketed {
                            format!("\x1b[200~{text}\x1b[201~").into_bytes()
                        } else {
                            text.replace("\r\n", "\r").replace('\n', "\r").into_bytes()
                        };
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
            serde_json::to_writer(&mut stream, &state)?;
            stream.write_all(b"\n")?;
            if state.stopped {
                fs::remove_file(&socket)?;
                return Ok(());
            }
        }
    }
    Ok(())
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
                if line == b"identity" {
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
