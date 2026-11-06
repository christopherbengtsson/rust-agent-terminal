//! Launch gate and conservative process evidence. Never signals restored processes.
use serde::{Deserialize, Serialize};
use std::{
    ffi::OsString,
    io::{self, Read, Write},
    os::unix::{net::UnixStream, process::CommandExt},
    path::Path,
    process::Command,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Execution {
    pub boot: String,
    #[serde(default)]
    pub observed_boot: Option<String>,
    pub pid: Option<i32>,
    pub start: Option<(u64, u64)>,
    /// Native execution may only be released after this record commits.
    pub released: bool,
}
impl Execution {
    pub fn prepared() -> io::Result<Self> {
        Ok(Self {
            boot: boot()?,
            observed_boot: None,
            pid: None,
            start: None,
            released: false,
        })
    }
    pub fn legacy() -> io::Result<Self> {
        Ok(Self {
            boot: String::new(),
            observed_boot: Some(boot()?),
            pid: None,
            start: None,
            released: true,
        })
    }
    pub fn anchor_legacy(&mut self) -> io::Result<()> {
        if self.boot.is_empty() && self.observed_boot.is_none() {
            self.observed_boot = Some(boot()?);
        }
        Ok(())
    }
    pub fn identify(&mut self, pid: i32) -> io::Result<()> {
        self.pid = Some(pid);
        self.start = Some(start(pid)?);
        self.released = true;
        Ok(())
    }
    pub fn verify_live(&self) -> io::Result<()> {
        let pid = self
            .pid
            .ok_or_else(|| io::Error::other("Missing gate identity"))?;
        if Some(start(pid)?) != self.start {
            return Err(io::Error::other(
                "Gate identity changed before exec receipt",
            ));
        }
        Ok(())
    }
    pub fn reconcile(&self) -> Result<(), String> {
        let current = boot().map_err(|e| format!("Cannot identify current boot: {e}"))?;
        if self.boot.is_empty() {
            return match self.observed_boot.as_deref() {
                Some(observed) if observed != current => Ok(()),
                _ => Err("Legacy execution has no historical boot identity. Recovery observation is durably anchored to this boot; verified reboot then u can exclude prior survivors. No replacement yet.".into()),
            };
        }
        if current != self.boot || !self.released {
            return Ok(());
        }
        let pid = self
            .pid
            .ok_or("Released execution has no process identity")?;
        match start(pid) {
            Ok(actual) if Some(actual) == self.start => return Err(format!("Verified surviving process {pid}; no replacement allowed. Finish it through native tools; leader absence may still require verified reboot. u rechecks evidence.")),
            Ok(_) => return Err("PID reused: original descendants remain uncertain. Verified reboot is the conservative recovery path.".into()),
            Err(error) if error.raw_os_error() == Some(libc::ESRCH) => {},
            Err(error) => return Err(format!("Process identity unavailable: {error}; verified reboot is the conservative recovery path.")),
        }
        let output = Command::new("ps")
            .args(["-axo", "pgid="])
            .output()
            .map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err(
                "Process-group inventory unavailable; retry reconciliation or reboot".into(),
            );
        }
        let groups = String::from_utf8(output.stdout).map_err(|e| e.to_string())?;
        for group in groups.split_whitespace() {
            let group: i32 = group
                .parse()
                .map_err(|_| "Invalid process-group inventory")?;
            if group == pid {
                return Err("Managed process group survives; finish it through native tools then retry u. Escaped descendants require verified reboot.".into());
            }
        }
        // Process groups do not establish absence of escaped native descendants.
        // Releasing native execution therefore requires reboot once its leader is lost.
        Err("Native leader absent; escaped descendants cannot be excluded. Verified reboot is the conservative recovery path; u retries evidence.".into())
    }
}
#[cfg(target_os = "macos")]
fn boot() -> io::Result<String> {
    #[cfg(debug_assertions)]
    if let Ok(value) = std::env::var("RAT_PROOF_BOOT_ID") {
        return Ok(format!("proof:{value}"));
    }
    let mut bytes = [0u8; 128];
    let mut size = bytes.len();
    // SAFETY: a NUL-terminated constant and a writable buffer with its actual size.
    if unsafe {
        libc::sysctlbyname(
            c"kern.bootsessionuuid".as_ptr(),
            bytes.as_mut_ptr().cast(),
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    if size == 0 || size > bytes.len() || bytes[size - 1] != 0 {
        return Err(io::Error::other("Invalid boot-session UUID"));
    }
    let value = std::str::from_utf8(&bytes[..size - 1]).map_err(io::Error::other)?;
    if value.len() != 36 {
        return Err(io::Error::other("Missing boot-session UUID"));
    }
    Ok(value.to_owned())
}
#[cfg(target_os = "macos")]
fn start(pid: i32) -> io::Result<(u64, u64)> {
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
        return Err(io::Error::last_os_error());
    }
    Ok((info.pbi_start_tvsec, info.pbi_start_tvusec))
}
#[cfg(not(target_os = "macos"))]
fn boot() -> io::Result<String> {
    #[cfg(debug_assertions)]
    if let Ok(value) = std::env::var("RAT_PROOF_BOOT_ID") {
        return Ok(format!("proof:{value}"));
    }
    std::fs::read_to_string("/proc/sys/kernel/random/boot_id")
}
#[cfg(not(target_os = "macos"))]
fn start(pid: i32) -> io::Result<(u64, u64)> {
    let text = std::fs::read_to_string(format!("/proc/{pid}/stat")).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            io::Error::from_raw_os_error(libc::ESRCH)
        } else {
            error
        }
    })?;
    let suffix = text
        .rsplit_once(')')
        .ok_or_else(|| io::Error::other("Invalid process evidence"))?
        .1;
    let ticks = suffix
        .split_whitespace()
        .nth(19)
        .ok_or_else(|| io::Error::other("Missing process start"))?;
    Ok((ticks.parse().map_err(io::Error::other)?, 0))
}

pub enum Receipt {
    Executed,
    ExecFailed(String),
    Unknown(String),
}

/// Prefix argv in place so native adapter arguments, cwd and environment survive.
pub fn wrap(
    command: &mut portable_pty::CommandBuilder,
    socket: &Path,
    nonce: &str,
) -> io::Result<()> {
    let native = std::mem::take(command.get_argv_mut());
    let argv = command.get_argv_mut();
    argv.push(std::env::current_exe()?.into_os_string());
    argv.push("--launch-gate".into());
    argv.push(socket.as_os_str().to_owned());
    argv.push(nonce.into());
    argv.extend(native);
    Ok(())
}
pub fn gate(socket: &Path, nonce: &str, argv: Vec<OsString>) -> io::Result<()> {
    let mut stream = UnixStream::connect(socket)?;
    stream.write_all(nonce.as_bytes())?;
    let mut release = [0];
    stream.read_exact(&mut release)?; // EOF before durable authorization executes nothing.
    if release[0] != 1 {
        return Err(io::Error::other("Launch gate not authorized"));
    }
    let program = argv
        .first()
        .ok_or_else(|| io::Error::other("Missing native executable"))?;
    let error = Command::new(program).args(&argv[1..]).exec();
    // UnixStream closes on exec; explicit exec failure is a distinct receipt.
    stream.write_all(error.to_string().as_bytes())?;
    Err(error)
}

/// Deliberate isolated debug-build crash barriers used by the external proof harness.
pub fn crash_barrier(directory: &Path, window: &str) {
    #[cfg(debug_assertions)]
    if std::fs::read_to_string(directory.join("proof-launch-crash"))
        .ok()
        .as_deref()
        == Some(window)
    {
        let _ = std::fs::remove_file(directory.join("proof-launch-crash"));
        // SAFETY: deliberately simulate supervisor process loss, without Rust destructors.
        unsafe { libc::_exit(86) }
    }
    let _ = (directory, window);
}
