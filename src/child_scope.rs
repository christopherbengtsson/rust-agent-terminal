//! Live-owner descendant tracking. Never reconstructs authority from saved PIDs.
use std::{
    collections::HashMap,
    io,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

const MAX_PROCESSES: usize = 8192;
const MAX_OWNED: usize = 1024;
#[derive(Clone, Copy, PartialEq, Eq)]
struct Identity {
    pid: i32,
    uid: u32,
    start: (u64, u64),
}
struct Process {
    identity: Identity,
    parent: i32,
    zombie: bool,
    stopped: bool,
}
struct State {
    owned: HashMap<i32, Identity>,
    error: Option<String>,
}
#[derive(Clone)]
pub struct Scope(Arc<Mutex<State>>, i32);
impl Scope {
    pub fn new(pid: i32) -> io::Result<Self> {
        if pid <= 1 {
            return Err(io::Error::other("Invalid managed process identity"));
        }
        let root = inspect(pid)?.ok_or_else(|| {
            io::Error::other("Managed leader disappeared before ownership capture")
        })?;
        let scope = Self(
            Arc::new(Mutex::new(State {
                owned: HashMap::from([(pid, root.identity)]),
                error: None,
            })),
            pid,
        );
        let weak = Arc::downgrade(&scope.0);
        thread::spawn(move || {
            loop {
                let Some(state) = weak.upgrade() else { break };
                Self(state.clone(), pid).refresh();
                drop(state);
                thread::sleep(Duration::from_millis(100));
            }
        });
        Ok(scope)
    }
    pub fn leader(&self) -> i32 {
        self.1
    }
    fn refresh(&self) {
        let mut state = self.0.lock().unwrap();
        if state.error.is_some()
            && !inspect(self.1)
                .is_ok_and(|p| p.is_some_and(|p| state.owned.get(&self.1) == Some(&p.identity)))
        {
            return;
        }
        state.error = refresh(&mut state).err().map(|error| error.to_string());
    }
    pub fn freeze(&self, deadline: Instant) -> io::Result<()> {
        // Freeze the root before the first stop-time inventory. Then freeze
        // each newly observed generation until two inventories close the tree.
        // Descendants stay frozen until KILL; TERM handlers cannot fork after
        // closure and orphan a new process between verification and leader exit.
        {
            let state = self.0.lock().unwrap();
            if let Some(identity) = state.owned.get(&self.1) {
                send_verified(*identity, libc::SIGSTOP)?;
            }
        }
        loop {
            if Instant::now() >= deadline {
                return Err(io::Error::other("Owned scope closure deadline exceeded"));
            }
            self.refresh();
            let before = {
                let state = self.0.lock().unwrap();
                if let Some(error) = &state.error {
                    return Err(io::Error::other(error.clone()));
                }
                for identity in state.owned.values() {
                    send_verified(*identity, libc::SIGSTOP)?;
                }
                let frozen = state.owned.values().try_fold(
                    true,
                    |frozen, identity| -> io::Result<bool> {
                        Ok(frozen
                            && verified(*identity)?
                                .is_none_or(|process| process.zombie || process.stopped))
                    },
                )?;
                if !frozen {
                    drop(state);
                    thread::sleep(Duration::from_millis(2));
                    continue;
                }
                state.owned.clone()
            };
            self.refresh();
            let state = self.0.lock().unwrap();
            if let Some(error) = &state.error {
                return Err(io::Error::other(error.clone()));
            }
            // A newly verified identity may replace a retained dead PID without
            // changing the count. It must go through SIGSTOP on the next pass.
            if state.owned == before {
                if Instant::now() >= deadline {
                    return Err(io::Error::other("Owned scope closure deadline exceeded"));
                }
                return Ok(());
            }
        }
    }
    pub fn signal_and_running(&self, signal: Option<i32>) -> io::Result<bool> {
        self.refresh();
        let state = self.0.lock().unwrap();
        if let Some(error) = &state.error {
            return Err(io::Error::other(error.clone()));
        }
        let mut running = false;
        // Each target is reidentified immediately before signalling. No group
        // signal can sweep in an unrelated process or a reused group leader.
        for identity in state.owned.values() {
            if let Some(actual) = verified(*identity)?
                && !actual.zombie
            {
                running = true;
                if let Some(signal) = signal {
                    // SAFETY: live-owner ancestry and UID/start identity checked above.
                    if unsafe { libc::kill(identity.pid, signal) } != 0 {
                        let error = io::Error::last_os_error();
                        if error.raw_os_error() != Some(libc::ESRCH) {
                            return Err(error);
                        }
                    }
                }
            }
        }
        Ok(running)
    }
}
fn verified(identity: Identity) -> io::Result<Option<Process>> {
    let actual = inspect(identity.pid)?;
    if let Some(process) = &actual
        && process.identity.start == identity.start
        && process.identity.uid != identity.uid
    {
        return Err(io::Error::other(
            "Owned process changed UID; execution remains uncertain",
        ));
    }
    Ok(actual.filter(|process| process.identity == identity))
}
fn send_verified(identity: Identity, signal: i32) -> io::Result<()> {
    if let Some(actual) = verified(identity)?
        && !actual.zombie
    {
        // SAFETY: only a live-owner descendant with matching UID/start identity.
        if unsafe { libc::kill(identity.pid, signal) } != 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() != Some(libc::ESRCH) {
                return Err(error);
            }
        }
    }
    Ok(())
}
fn refresh(state: &mut State) -> io::Result<()> {
    let began = Instant::now();
    let processes = inventory()?;
    let mut live = HashMap::new();
    for identity in state.owned.values() {
        if verified(*identity)?.is_some() {
            live.insert(identity.pid, *identity);
        }
    }
    loop {
        let mut added = false;
        for (pid, parent) in &processes {
            if began.elapsed() > Duration::from_millis(500) {
                return Err(io::Error::other(
                    "Owned process inventory deadline exceeded",
                ));
            }
            if live.contains_key(pid) {
                continue;
            }
            if let Some(parent) = live.get(parent)
                && let Some(process) = inspect(*pid)?
                && process.identity.uid == parent.uid
                && process.parent == parent.pid
                && verified(*parent)?.is_some()
            {
                if state.owned.len() >= MAX_OWNED {
                    return Err(io::Error::other("Owned process limit exceeded"));
                }
                state.owned.insert(*pid, process.identity);
                live.insert(*pid, process.identity);
                added = true;
            }
        }
        if !added {
            return Ok(());
        }
    }
}
fn inventory() -> io::Result<Vec<(i32, i32)>> {
    use std::{
        io::Read,
        process::{Command, Stdio},
        sync::mpsc,
    };
    let mut child = Command::new("ps")
        .args(["-axo", "pid=,ppid="])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let stdout = child.stdout.take().unwrap();
    let (tx, rx) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stdout
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map(|_| bytes);
        let _ = tx.send(result);
    });
    let began = Instant::now();
    while child.try_wait()?.is_none() {
        if began.elapsed() >= Duration::from_millis(500) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::other("Process inventory timed out"));
        }
        thread::sleep(Duration::from_millis(2));
    }
    if !child.wait()?.success() {
        return Err(io::Error::other("Process inventory failed"));
    }
    let bytes = rx
        .recv_timeout(Duration::from_millis(10))
        .map_err(io::Error::other)??;
    if bytes.len() > 1024 * 1024 {
        return Err(io::Error::other("Process inventory byte limit exceeded"));
    }
    let text = std::str::from_utf8(&bytes).map_err(io::Error::other)?;
    let mut result = Vec::new();
    for line in text.lines() {
        if result.len() >= MAX_PROCESSES {
            return Err(io::Error::other("Process inventory count limit exceeded"));
        }
        let mut fields = line.split_whitespace();
        let pid = fields
            .next()
            .ok_or_else(|| io::Error::other("Missing PID"))?
            .parse()
            .map_err(io::Error::other)?;
        let parent = fields
            .next()
            .ok_or_else(|| io::Error::other("Missing PPID"))?
            .parse()
            .map_err(io::Error::other)?;
        result.push((pid, parent));
    }
    Ok(result)
}
#[cfg(target_os = "macos")]
fn inspect(pid: i32) -> io::Result<Option<Process>> {
    let mut info = unsafe { std::mem::zeroed::<libc::proc_bsdinfo>() };
    let size = std::mem::size_of_val(&info) as i32;
    if unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTBSDINFO,
            0,
            (&mut info as *mut libc::proc_bsdinfo).cast(),
            size,
        )
    } != size
    {
        let error = io::Error::last_os_error();
        return if matches!(error.raw_os_error(), Some(libc::ESRCH) | Some(libc::ENOENT)) {
            Ok(None)
        } else {
            Err(error)
        };
    }
    Ok(Some(Process {
        identity: Identity {
            pid,
            uid: info.pbi_uid,
            start: (info.pbi_start_tvsec, info.pbi_start_tvusec),
        },
        parent: info.pbi_ppid as i32,
        zombie: info.pbi_status == 5,
        stopped: info.pbi_status == 4,
    }))
}
#[cfg(not(target_os = "macos"))]
fn inspect(pid: i32) -> io::Result<Option<Process>> {
    use std::os::unix::fs::MetadataExt;
    let path = format!("/proc/{pid}");
    let text = match std::fs::read_to_string(format!("{path}/stat")) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let uid = std::fs::metadata(path)?.uid();
    let fields: Vec<_> = text
        .rsplit_once(')')
        .ok_or_else(|| io::Error::other("Invalid process identity"))?
        .1
        .split_whitespace()
        .collect();
    Ok(Some(Process {
        identity: Identity {
            pid,
            uid,
            start: (fields[19].parse().map_err(io::Error::other)?, 0),
        },
        parent: fields[1].parse().map_err(io::Error::other)?,
        zombie: fields[0] == "Z",
        stopped: matches!(fields[0], "T" | "t"),
    }))
}
