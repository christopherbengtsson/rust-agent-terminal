//! Bounded process-group termination runs away from transport and UI loops.
use std::{
    io,
    process::{Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread,
    time::{Duration, Instant},
};
pub type Child = Box<dyn portable_pty::Child + Send + Sync>;
pub struct Outcome {
    pub child: Option<Child>,
    pub error: Option<String>,
}
pub fn stop(mut child: Child, group: i32) -> Receiver<Outcome> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let result = (|| -> io::Result<()> {
            // macOS can report EPERM transiently while a signalled PTY leader
            // is exiting. Preserve the error, but let actual reaping and group
            // absence establish success within the same bounded deadline.
            let mut signal_error = match signal(group, libc::SIGTERM) {
                Ok(()) => None,
                Err(error) if error.raw_os_error() == Some(libc::EPERM) => Some(error),
                Err(error) => return Err(error),
            };
            let started = Instant::now();
            let mut escalated = false;
            loop {
                let exited = child
                    .try_wait()
                    .map_err(|e| io::Error::other(format!("wait child: {e}")))?
                    .is_some();
                if exited
                    && !group_running(group)
                        .map_err(|e| io::Error::other(format!("verify group: {e}")))?
                {
                    return Ok(());
                }
                if !escalated && started.elapsed() >= Duration::from_millis(500) {
                    match signal(group, libc::SIGKILL) {
                        Ok(()) => {}
                        Err(error) if error.raw_os_error() == Some(libc::EPERM) => {
                            signal_error = Some(error)
                        }
                        Err(error) => return Err(error),
                    }
                    escalated = true;
                }
                if started.elapsed() >= Duration::from_secs(2) {
                    return Err(signal_error.unwrap_or_else(|| {
                        io::Error::other("Execution remains observable after TERM/KILL deadline")
                    }));
                }
                thread::sleep(Duration::from_millis(20));
            }
        })();
        let outcome = match result {
            Ok(()) => Outcome {
                child: None,
                error: None,
            },
            Err(error) => Outcome {
                child: Some(child),
                error: Some(format!(
                    "Stop unconfirmed: {error}; retry s (stop) or q (quit)"
                )),
            },
        };
        let _ = tx.send(outcome);
    });
    rx
}
fn signal(group: i32, signal: i32) -> io::Result<()> {
    if group <= 1 {
        return Err(io::Error::other("Invalid managed process group"));
    }
    // SAFETY: kill only receives integer identifiers; negative targets the managed group.
    if unsafe { libc::kill(-group, signal) } == 0 {
        return Ok(());
    }
    let error = io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ESRCH) {
        Ok(())
    } else {
        Err(error)
    }
}
fn group_running(group: i32) -> io::Result<bool> {
    // Zombies cannot execute. A dead leader alone does not prove descendant cleanup.
    let mut inspection = Command::new("ps")
        .args(["-axo", "pgid=,stat="])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let started = Instant::now();
    while inspection.try_wait()?.is_none() {
        if started.elapsed() >= Duration::from_millis(500) {
            let _ = inspection.kill();
            let _ = inspection.wait();
            return Err(io::Error::other("Process verification timed out"));
        }
        thread::sleep(Duration::from_millis(5));
    }
    let output = inspection.wait_with_output()?;
    if !output.status.success() {
        return Err(io::Error::other("Cannot verify managed process group"));
    }
    Ok(String::from_utf8_lossy(&output.stdout).lines().any(|line| {
        let mut fields = line.split_whitespace();
        fields.next().and_then(|id| id.parse::<i32>().ok()) == Some(group)
            && fields.next().is_some_and(|state| !state.starts_with('Z'))
    }))
}
