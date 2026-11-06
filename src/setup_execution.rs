//! Owned bounded setup execution. No storage or native conversation ownership.
use crate::setup::{Phase, Record, Tail};
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
pub(crate) struct Job {
    pub(crate) cancel: Arc<AtomicBool>,
    pub(crate) events: Receiver<Record>,
    pub(crate) acknowledgement: SyncSender<()>,
    pub(crate) tail: Arc<Mutex<Tail>>,
    pub(crate) _thread: thread::JoinHandle<()>,
}
impl Job {
    pub(crate) fn start(record: Record) -> io::Result<Self> {
        let cancel = Arc::new(AtomicBool::new(false));
        let cancellation = cancel.clone();
        let tail = Arc::new(Mutex::new(Tail::default()));
        let progress = tail.clone();
        let (sender, events) = mpsc::sync_channel(1);
        let (acknowledgement, ack) = mpsc::sync_channel(1);
        let worker = thread::Builder::new()
            .name("workspace-setup".into())
            .spawn(move || execute(record, cancellation, progress, sender, ack))?;
        Ok(Self {
            cancel,
            events,
            acknowledgement,
            tail,
            _thread: worker,
        })
    }
}
fn nonblocking(fd: std::os::fd::RawFd) -> io::Result<()> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
fn drain(reader: &mut impl Read, tail: &Arc<Mutex<Tail>>) -> io::Result<bool> {
    let mut bytes = [0; 4096];
    for _ in 0..2 {
        match reader.read(&mut bytes) {
            Ok(0) => return Ok(true),
            Ok(n) => tail.lock().unwrap().append(&bytes[..n]),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
            Err(e) => return Err(e),
        }
    }
    Ok(false)
}
fn execute(
    mut record: Record,
    cancel: Arc<AtomicBool>,
    tail: Arc<Mutex<Tail>>,
    sender: SyncSender<Record>,
    ack: Receiver<()>,
) {
    let mut child: Option<crate::lifecycle::Child> = None;
    let mut scope = None;
    let mut stdout = None;
    let mut stderr = None;
    let result = (|| -> Result<bool, String> {
        let checkout = record
            .creation
            .checkout
            .as_ref()
            .ok_or("Setup checkout unavailable")?;
        let actual = crate::git::resolve_checkout(&checkout.root, &cancel)?;
        if actual != *checkout {
            return Err("Setup checkout identity changed; execution refused".into());
        }
        let temporary = tempfile::Builder::new()
            .prefix("rat-setup-")
            .tempdir()
            .map_err(|e| e.to_string())?;
        let socket = temporary.path().join("gate");
        let listener = UnixListener::bind(&socket).map_err(|e| e.to_string())?;
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        let nonce = uuid::Uuid::new_v4().to_string();
        let mut command = Process::new(std::env::current_exe().map_err(|e| e.to_string())?);
        command
            .arg("--launch-gate")
            .arg(&socket)
            .arg(&nonce)
            .args(["/bin/sh", "-c", &record.command])
            .current_dir(&checkout.root)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() < 0 {
                    Err(io::Error::last_os_error())
                } else {
                    Ok(())
                }
            });
        }
        if cancel.load(Ordering::Acquire) {
            return Err("Setup cancelled before spawn".into());
        }
        let mut created = command.spawn().map_err(|e| e.to_string())?;
        // Install process ownership before fallible pipe/scope/identity setup.
        let pid = created.id() as i32;
        stdout = created.stdout.take();
        stderr = created.stderr.take();
        child = Some(Box::new(created));
        scope = Some(crate::child_scope::Scope::new(pid).map_err(|e| e.to_string())?);
        record.execution.identify(pid).map_err(|e| e.to_string())?;
        nonblocking(stdout.as_ref().ok_or("Setup stdout missing")?.as_raw_fd())
            .map_err(|e| e.to_string())?;
        nonblocking(stderr.as_ref().ok_or("Setup stderr missing")?.as_raw_fd())
            .map_err(|e| e.to_string())?;
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut gate = loop {
            if cancel.load(Ordering::Acquire) {
                return Err("Setup cancelled before authorization".into());
            }
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                Err(e) => return Err(e.to_string()),
            }
            if Instant::now() > deadline {
                return Err("Setup gate connection timed out".into());
            }
            thread::sleep(Duration::from_millis(5));
        };
        gate.set_read_timeout(Some(Duration::from_millis(500)))
            .map_err(|e| e.to_string())?;
        let mut identity = vec![0; nonce.len()];
        gate.read_exact(&mut identity).map_err(|e| e.to_string())?;
        if identity != nonce.as_bytes() {
            return Err("Setup gate authentication failed".into());
        }
        record.phase = Phase::Authorized;
        record.message = "Setup gate awaiting durable authorization".into();
        sender.send(record.clone()).map_err(|e| e.to_string())?;
        loop {
            if cancel.load(Ordering::Acquire) {
                return Err("Setup cancelled while authorization pending".into());
            }
            match ack.recv_timeout(Duration::from_millis(10)) {
                Ok(()) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(e) => return Err(e.to_string()),
            }
        }
        if cancel.load(Ordering::Acquire) {
            return Err("Setup cancelled before release".into());
        }
        gate.write_all(&[1]).map_err(|e| e.to_string())?;
        let mut failure = Vec::new();
        gate.read_to_end(&mut failure)
            .map_err(|e| format!("Setup exec receipt uncertain: {e}"))?;
        if !failure.is_empty() {
            return Err(format!(
                "Setup exec failed: {}",
                String::from_utf8_lossy(&failure)
            ));
        }
        record
            .execution
            .verify_live()
            .map_err(|e| format!("Setup exec identity uncertain: {e}"))?;
        record.message =
            "Setup running; bounded combined stdout/stderr tail (not conversation history)".into();
        loop {
            if cancel.load(Ordering::Acquire) {
                return Err("Setup cancelled by Stop".into());
            }
            drain(stdout.as_mut().unwrap(), &tail).map_err(|e| e.to_string())?;
            drain(stderr.as_mut().unwrap(), &tail).map_err(|e| e.to_string())?;
            if let Some(status) = child
                .as_mut()
                .unwrap()
                .try_wait()
                .map_err(|e| e.to_string())?
            {
                return Ok(status.success());
            }
            thread::sleep(Duration::from_millis(10));
        }
    })();
    // Scope cleanup and final bounded drain precede terminal receipt/launch.
    let mut cleanup_error = None;
    if let Some(owned) = child.take() {
        if let Some(owned_scope) = scope {
            let mut stop = crate::lifecycle::stop(owned, owned_scope.clone());
            loop {
                match stop.recv() {
                    Ok(outcome) => {
                        if let Some(retained) = outcome.child {
                            cleanup_error = outcome.error; // Retain ownership and keep cancellation responsive until verified cleanup.
                            thread::sleep(Duration::from_millis(100));
                            stop = crate::lifecycle::stop(retained, owned_scope.clone());
                        } else {
                            cleanup_error = None;
                            break;
                        }
                    }
                    Err(e) => {
                        cleanup_error = Some(e.to_string());
                        break;
                    }
                }
            }
        } else {
            // Gate was never released. Retain and reap it; no saved PID grants signaling authority.
            let mut owned = owned;
            let _ = owned.kill();
            if let Err(e) = owned.wait() {
                cleanup_error = Some(e.to_string());
            }
        }
    }
    let deadline = Instant::now() + Duration::from_millis(250);
    loop {
        let a = stdout
            .as_mut()
            .is_none_or(|r| drain(r, &tail).unwrap_or(true));
        let b = stderr
            .as_mut()
            .is_none_or(|r| drain(r, &tail).unwrap_or(true));
        if a && b {
            break;
        }
        if Instant::now() > deadline {
            tail.lock()
                .unwrap()
                .append(b"\n[setup output pipe did not reach EOF; tail capture ended]\n");
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }
    tail.lock().unwrap().apply(&mut record);
    record.phase = if cleanup_error.is_some() {
        Phase::Uncertain
    } else if cancel.load(Ordering::Acquire) {
        Phase::Cancelled
    } else if result.as_ref().is_ok_and(|ok| *ok) {
        Phase::Succeeded
    } else {
        Phase::Failed
    };
    record.message = cleanup_error.or_else(|| result.err()).unwrap_or_else(|| {
        if record.phase == Phase::Succeeded {
            "Setup succeeded; owned execution cleaned".into()
        } else {
            "Setup failed; Retry or Continue without setup".into()
        }
    });
    let _ = sender.send(record);
}
