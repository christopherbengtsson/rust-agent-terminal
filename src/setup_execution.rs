//! Owned bounded setup execution. No storage or native conversation ownership.
use crate::setup::{Phase, Record, Tail};
use std::{
    io::{self, Read, Write},
    os::unix::{fs::DirBuilderExt, io::AsRawFd, net::UnixListener, process::CommandExt},
    path::PathBuf,
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
// The path is generated here, never restored from a receipt. mkdir atomically
// establishes exclusive ownership and permissions before the listener exists.
struct GateDirectory(PathBuf);
impl GateDirectory {
    fn create() -> io::Result<Self> {
        let path = PathBuf::from("/tmp").join(format!("rat-s-{}", uuid::Uuid::new_v4().simple()));
        std::fs::DirBuilder::new().mode(0o700).create(&path)?;
        Ok(Self(path))
    }
}
impl Drop for GateDirectory {
    fn drop(&mut self) {
        // Only the known socket and our empty directory; never recursive removal.
        let _ = std::fs::remove_file(self.0.join("gate"));
        let _ = std::fs::remove_dir(&self.0);
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
    let mut child: Option<std::process::Child> = None;
    let mut released = false;
    let mut receipt_uncertain = false;
    let mut exec_failed = false;
    let mut scope = None;
    let mut stdout = None;
    let mut stderr = None;
    let result = (|| -> Result<std::process::ExitStatus, String> {
        let checkout = record
            .creation
            .checkout
            .as_ref()
            .ok_or("Setup checkout unavailable")?;
        let actual = crate::git::resolve_checkout(&checkout.root, &cancel)?;
        if actual != *checkout {
            return Err("Setup checkout identity changed; execution refused".into());
        }
        let temporary = GateDirectory::create().map_err(|e| e.to_string())?;
        let socket = temporary.0.join("gate");
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
        let created = command.spawn().map_err(|e| e.to_string())?;
        // Install process ownership before fallible pipe/scope/identity setup.
        let pid = created.id() as i32;
        child = Some(created);
        stdout = child.as_mut().unwrap().stdout.take();
        stderr = child.as_mut().unwrap().stderr.take();
        let pipes_ready = nonblocking(stdout.as_ref().ok_or("Setup stdout missing")?.as_raw_fd())
            .and_then(|()| {
                nonblocking(
                    stderr
                        .as_ref()
                        .ok_or_else(|| io::Error::other("Setup stderr missing"))?
                        .as_raw_fd(),
                )
            });
        if let Err(error) = pipes_ready {
            // Cleanup must never read a pipe whose nonblocking setup failed.
            stdout = None;
            stderr = None;
            return Err(format!("Setup pipe initialization failed: {error}"));
        }
        scope = Some(crate::child_scope::Scope::new(pid).map_err(|e| e.to_string())?);
        record.execution.identify(pid).map_err(|e| e.to_string())?;
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
        // Verify the original gated leader while it cannot yet exit through the
        // user command. After exec EOF a fast command may already have exited.
        if crate::recovery::Execution::prepared()
            .map_err(|e| e.to_string())?
            .boot
            != record.execution.boot
        {
            return Err("Setup boot identity changed before release".into());
        }
        record.execution.verify_live().map_err(|e| e.to_string())?;
        gate.set_nonblocking(true).map_err(|e| e.to_string())?;
        released = true; // A partial release write is conservatively uncertain.
        gate.write_all(&[1]).map_err(|e| {
            receipt_uncertain = true;
            format!("Setup release receipt uncertain: {e}")
        })?;
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut failure = Vec::new();
        loop {
            if cancel.load(Ordering::Acquire) {
                return Err("Setup cancelled while awaiting exec receipt".into());
            }
            drain(stdout.as_mut().unwrap(), &tail).map_err(|e| e.to_string())?;
            drain(stderr.as_mut().unwrap(), &tail).map_err(|e| e.to_string())?;
            let mut bytes = [0; 512];
            match gate.read(&mut bytes) {
                Ok(0) => break,
                Ok(n) if failure.len() + n <= 4096 => failure.extend_from_slice(&bytes[..n]),
                Ok(_) => {
                    receipt_uncertain = true;
                    return Err("Setup exec receipt exceeded bound".into());
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                Err(e) => {
                    receipt_uncertain = true;
                    return Err(format!("Setup exec receipt uncertain: {e}"));
                }
            }
            if Instant::now() >= deadline {
                receipt_uncertain = true;
                return Err("Setup exec receipt timed out; outcome uncertain".into());
            }
            thread::sleep(Duration::from_millis(5));
        }
        if !failure.is_empty() {
            exec_failed = true;
            return Err(format!(
                "Setup exec failed: {}",
                String::from_utf8_lossy(&failure)
            ));
        }
        record.phase = Phase::Running;
        record.message =
            "Setup running; bounded combined stdout/stderr tail (not conversation history)".into();
        sender.send(record.clone()).map_err(|e| e.to_string())?;
        // Saving Running may stall. Drain fairly and retain cancellation even
        // when no writer acknowledgement can arrive.
        loop {
            if cancel.load(Ordering::Acquire) {
                return Err("Setup cancelled while Running receipt pending".into());
            }
            drain(stdout.as_mut().unwrap(), &tail).map_err(|e| e.to_string())?;
            drain(stderr.as_mut().unwrap(), &tail).map_err(|e| e.to_string())?;
            match ack.recv_timeout(Duration::from_millis(10)) {
                Ok(()) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(e) => return Err(e.to_string()),
            }
        }
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
                return Ok(status);
            }
            thread::sleep(Duration::from_millis(10));
        }
    })();
    // Retain child/scope until cleanup is proven. Progress events are bounded
    // and nonblocking: a stalled writer must never block ownership or Stop.
    if let Some(owned) = child.take() {
        let mut owned: crate::lifecycle::Child = Box::new(owned);
        if let Some(owned_scope) = scope {
            loop {
                let stop = crate::lifecycle::stop(owned, owned_scope.clone());
                let outcome = loop {
                    if let Some(r) = stdout.as_mut() {
                        let _ = drain(r, &tail);
                    }
                    if let Some(r) = stderr.as_mut() {
                        let _ = drain(r, &tail);
                    }
                    match stop.recv_timeout(Duration::from_millis(10)) {
                        Ok(outcome) => break outcome,
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                        // lifecycle owns the child until returning it. Losing
                        // that worker cannot establish cleanup; retain the job
                        // reservation indefinitely rather than report success.
                        Err(error) => loop {
                            record.phase = Phase::Running;
                            record.message =
                                format!("Setup cleanup worker lost: {error}; ownership uncertain");
                            let _ = sender.try_send(record.clone());
                            thread::sleep(Duration::from_millis(100));
                        },
                    }
                };
                if let Some(retained) = outcome.child {
                    owned = retained;
                    record.phase = Phase::Running;
                    record.message = outcome
                        .error
                        .unwrap_or_else(|| "Setup cleanup unconfirmed; retrying".into());
                    let _ = sender.try_send(record.clone());
                    thread::sleep(Duration::from_millis(100));
                } else {
                    break;
                }
            }
        } else {
            // No gate release is possible before scope capture. Keep the
            // concrete owned child until kill and reap establish its absence.
            loop {
                let _ = owned.kill();
                match owned.try_wait() {
                    Ok(Some(_)) => break,
                    _ => {
                        record.phase = Phase::Running;
                        record.message = "Setup gated-child cleanup unconfirmed; retrying".into();
                        let _ = sender.try_send(record.clone());
                        thread::sleep(Duration::from_millis(20));
                    }
                }
            }
        }
    }
    let deadline = Instant::now() + Duration::from_millis(250);
    let mut output_error = None;
    loop {
        let a = stdout.as_mut().is_none_or(|r| match drain(r, &tail) {
            Ok(eof) => eof,
            Err(e) => {
                output_error = Some(format!("Setup output capture failed: {e}"));
                true
            }
        });
        let b = stderr.as_mut().is_none_or(|r| match drain(r, &tail) {
            Ok(eof) => eof,
            Err(e) => {
                output_error = Some(format!("Setup output capture failed: {e}"));
                true
            }
        });
        if a && b {
            break;
        }
        if Instant::now() > deadline {
            output_error = Some("Setup output pipe did not reach EOF; capture incomplete".into());
            tail.lock()
                .unwrap()
                .append(b"\n[setup output pipe did not reach EOF; tail capture ended]\n");
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }
    record.cleanup_confirmed = true;
    tail.lock().unwrap().apply(&mut record);
    record.phase = if cancel.load(Ordering::Acquire) {
        Phase::Cancelled
    } else if receipt_uncertain
        || output_error.is_some()
        || (released && result.is_err() && !exec_failed)
    {
        Phase::Uncertain
    } else if result.as_ref().is_ok_and(|status| status.success()) {
        Phase::Succeeded
    } else {
        Phase::Failed
    };
    record.message = match result {
        Ok(status) => format!(
            "Setup {status}; owned execution cleaned{}",
            output_error.map(|e| format!("; {e}")).unwrap_or_default()
        ),
        Err(error) => format!(
            "{error}{}",
            if released {
                "; command may have external effects"
            } else {
                "; gate never released"
            }
        ),
    };
    let _ = sender.send(record);
}
