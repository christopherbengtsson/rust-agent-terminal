use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
    sync::mpsc,
    time::{Duration, Instant},
};

struct Dashboard {
    child: Box<dyn portable_pty::Child + Send + Sync>,
    writer: Box<dyn Write + Send>,
    output: mpsc::Receiver<Vec<u8>>,
    screen: vt100::Parser,
}
impl Dashboard {
    fn open(repository: &Path, state: &Path) -> Self {
        Self::open_with_path(repository, state, None)
    }
    fn open_with_path(repository: &Path, state: &Path, path: Option<&std::ffi::OsStr>) -> Self {
        Self::open_config(repository, state, path, None)
    }
    fn open_config(
        repository: &Path,
        state: &Path,
        path: Option<&std::ffi::OsStr>,
        proof_boot: Option<&str>,
    ) -> Self {
        let pty = native_pty_system()
            .openpty(PtySize {
                rows: 40,
                cols: 160,
                pixel_width: 0,
                pixel_height: 0,
            })
            .unwrap();
        let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_agent-terminal"));
        if let Some(path) = path {
            command.env("PATH", path);
        }
        if let Some(boot) = proof_boot {
            command.env("RAT_PROOF_BOOT_ID", boot);
        }
        command.arg(repository);
        command.arg("--state-dir");
        command.arg(state);
        let child = pty.slave.spawn_command(command).unwrap();
        drop(pty.slave);
        let mut reader = pty.master.try_clone_reader().unwrap();
        let writer = pty.master.take_writer().unwrap();
        let (tx, output) = mpsc::sync_channel(8);
        std::thread::spawn(move || {
            let _master = pty.master;
            let mut bytes = [0; 4096];
            while let Ok(n) = reader.read(&mut bytes) {
                if n == 0 || tx.send(bytes[..n].to_vec()).is_err() {
                    break;
                }
            }
        });
        Self {
            child,
            writer,
            output,
            screen: vt100::Parser::new(40.try_into().unwrap(), 160.try_into().unwrap(), 0),
        }
    }
    fn send(&mut self, bytes: &[u8]) {
        self.writer.write_all(bytes).unwrap();
    }
    fn wait(&mut self, needle: &str) -> String {
        self.wait_any(&[needle])
    }
    fn wait_any(&mut self, needles: &[&str]) -> String {
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            let screen = self.screen.screen().contents();
            if needles.iter().any(|needle| screen.contains(needle)) {
                return screen;
            }
            assert!(Instant::now() < deadline, "missing {needles:?}: {screen}");
            if let Ok(bytes) = self.output.recv_timeout(Duration::from_millis(40)) {
                self.screen.process(&bytes);
            }
        }
    }
    fn kill(&mut self) {
        // portable-pty's Unix kill sends SIGHUP, which need not terminate a
        // dashboard blocked on a full outer PTY. Tests require actual abrupt
        // dashboard loss; use SIGKILL only for this owned fixture process.
        if let Some(pid) = self.child.process_id() {
            // SAFETY: the handle is the fixture child, not an inferred owner PID.
            unsafe {
                libc::kill(pid as i32, libc::SIGKILL);
            }
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            // Keep the real outer PTY drained during abrupt fixture cleanup.
            // Otherwise macOS can retain an exiting writer behind tty output
            // backpressure while this bounded harness channel is full.
            while self.output.try_recv().is_ok() {}
            if self.child.try_wait().is_ok_and(|status| status.is_some()) {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "owned dashboard did not exit after SIGKILL and PTY drain"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}
impl Drop for Dashboard {
    fn drop(&mut self) {
        self.kill();
    }
}
// Each scenario deliberately controls its own concurrency. Keep unrelated outer
// PTY scenarios from competing for host process/terminal resources.
static SCENARIO: std::sync::Mutex<()> = std::sync::Mutex::new(());
struct Fixture {
    _scenario: std::sync::MutexGuard<'static, ()>,
    temp: tempfile::TempDir,
    repository: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let scenario = SCENARIO.lock().unwrap_or_else(|error| error.into_inner());
        let temp = tempfile::tempdir_in("/private/tmp").unwrap();
        let repository = temp.path().join("checkout with spaces");
        std::fs::create_dir(&repository).unwrap();
        assert!(
            Command::new("git")
                .args(["init", "-b", "main"])
                .arg(&repository)
                .output()
                .unwrap()
                .status
                .success()
        );
        Self {
            _scenario: scenario,
            temp,
            repository,
        }
    }
    fn state(&self) -> PathBuf {
        self.temp.path().join("state")
    }
    fn assert_one_owner_and_child(&self) {
        let output = Command::new("ps")
            .args(["-Ao", "pid,ppid,command"])
            .output()
            .unwrap();
        assert!(output.status.success());
        let processes = String::from_utf8(output.stdout).unwrap();
        let owners: Vec<_> = processes
            .lines()
            .filter(|line| {
                line.contains("--supervisor") && line.contains(self.state().to_str().unwrap())
            })
            .collect();
        assert_eq!(
            owners.len(),
            1,
            "exactly one supervisor owns this test endpoint"
        );
        let owner = owners[0].split_whitespace().next().unwrap();
        let children = processes
            .lines()
            .filter(|line| {
                line.split_whitespace().nth(1) == Some(owner) && line.contains("--scripted-child")
            })
            .count();
        assert_eq!(
            children, 1,
            "exactly one active fixture child belongs to the owner"
        );
    }
    fn reopen_after_release(&self, needle: &str) -> Dashboard {
        self.reopen_with_path_after_release(needle, None)
    }
    fn reopen_with_path_after_release(
        &self,
        needle: &str,
        path: Option<&std::ffi::OsStr>,
    ) -> Dashboard {
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            let mut dashboard = Dashboard::open_with_path(&self.repository, &self.state(), path);
            let screen =
                dashboard.wait_any(&[needle, "Another dashboard controls this supervisor"]);
            if screen.contains(needle) {
                return dashboard;
            }
            dashboard.kill();
            assert!(Instant::now() < deadline, "controller EOF was not released");
        }
    }
    fn open(&self) -> Dashboard {
        Dashboard::open(&self.repository, &self.state())
    }
}
fn fixture_peer_pid(stream: &std::os::unix::net::UnixStream) -> Option<i32> {
    use std::os::fd::AsRawFd;
    #[cfg(target_os = "macos")]
    {
        let mut pid: libc::pid_t = 0;
        let mut length = std::mem::size_of_val(&pid) as libc::socklen_t;
        // SAFETY: valid socket fd and correctly sized output storage.
        let result = unsafe {
            libc::getsockopt(
                stream.as_raw_fd(),
                0,
                libc::LOCAL_PEERPID,
                (&mut pid as *mut libc::pid_t).cast(),
                &mut length,
            )
        };
        (result == 0 && pid > 1).then_some(pid)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let mut credentials: libc::ucred = unsafe { std::mem::zeroed() };
        let mut length = std::mem::size_of_val(&credentials) as libc::socklen_t;
        let result = unsafe {
            libc::getsockopt(
                stream.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_PEERCRED,
                (&mut credentials as *mut libc::ucred).cast(),
                &mut length,
            )
        };
        (result == 0 && credentials.pid > 1).then_some(credentials.pid)
    }
}
fn fixture_frame(
    reader: &mut std::io::BufReader<std::os::unix::net::UnixStream>,
) -> Option<serde_json::Value> {
    use std::io::BufRead;
    let mut bytes = Vec::new();
    reader
        .take(4 * 1024 * 1024 + 1)
        .read_until(b'\n', &mut bytes)
        .ok()?;
    if bytes.len() > 4 * 1024 * 1024 || bytes.last() != Some(&b'\n') {
        return None;
    }
    serde_json::from_slice(&bytes).ok()
}
impl Drop for Fixture {
    fn drop(&mut self) {
        use std::os::unix::{fs::PermissionsExt, net::UnixStream};
        let state = self.state();
        // Repair only this fixture's real private directory; never follow the
        // deliberately invalid symlink used by the unsafe-directory test.
        if !std::fs::symlink_metadata(&state).is_ok_and(|m| m.file_type().is_dir()) {
            return;
        }
        let _ = std::fs::set_permissions(&state, std::fs::Permissions::from_mode(0o700));
        if let Ok(entries) = std::fs::read_dir(&state) {
            for entry in entries.flatten() {
                if entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("proof-storage-")
                    || entry.file_name() == "proof-launch-receipt"
                    || entry.file_name() == "proof-launch-creation"
                    || entry.file_name() == "proof-launch-setup"
                {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
        let socket = state.join("supervisor.sock");
        let deadline = Instant::now() + Duration::from_secs(4);
        let mut owner = None;
        let mut child = None;
        while Instant::now() < deadline && socket.exists() {
            if let Ok(mut stream) = UnixStream::connect(&socket) {
                let peer = fixture_peer_pid(&stream);
                if owner.is_some() && owner != peer {
                    break;
                }
                owner = peer;
                let _ = stream.set_read_timeout(Some(Duration::from_millis(250)));
                let _ = stream.set_write_timeout(Some(Duration::from_millis(250)));
                if let Ok(copy) = stream.try_clone() {
                    let mut reader = std::io::BufReader::new(copy);
                    if let Some(greeting) = fixture_frame(&mut reader) {
                        if greeting["control_denied"] == true {
                            std::thread::sleep(Duration::from_millis(10));
                            continue;
                        }
                        child = greeting["execution"]["pid"]
                            .as_i64()
                            .map(|pid| pid as i32)
                            .or(child);
                        if stream.write_all(b"\"Shutdown\"\n").is_ok() {
                            while Instant::now() < deadline {
                                let Some(response) = fixture_frame(&mut reader) else {
                                    break;
                                };
                                if response["stopped"] == true {
                                    break;
                                }
                                if stream.write_all(b"\"Screen\"\n").is_err() {
                                    break;
                                }
                                std::thread::sleep(Duration::from_millis(10));
                            }
                        }
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        if !socket.exists() {
            // The final ACK/endpoint removal precedes actual process exit by a
            // few instructions. Verify the authenticated peer finishes too.
            if let Some(owner) = owner {
                let deadline = Instant::now() + Duration::from_secs(1);
                loop {
                    let running = Command::new("/bin/ps")
                        .args(["-p", &owner.to_string(), "-o", "stat="])
                        .output()
                        .is_ok_and(|output| {
                            String::from_utf8_lossy(&output.stdout).lines().any(|line| {
                                !line.trim().is_empty() && !line.trim().starts_with('Z')
                            })
                        });
                    if !running {
                        return;
                    }
                    if Instant::now() >= deadline {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
            } else {
                return;
            }
        }
        // Recovery scenarios deliberately retain uncertain historical execution.
        // Fallback targets only the socket's authenticated peer and a directly
        // observed child still parented by it; never scan/kill by a text match.
        if let Some(owner) = owner {
            if let Some(child) = child
                && let Ok(output) = Command::new("/bin/ps")
                    .args(["-p", &child.to_string(), "-o", "ppid="])
                    .output()
                && String::from_utf8_lossy(&output.stdout)
                    .trim()
                    .parse::<i32>()
                    .ok()
                    == Some(owner)
            {
                // SAFETY: the observed fixture child belongs to this exact peer.
                unsafe {
                    libc::kill(-child, libc::SIGKILL);
                }
            }
            // SAFETY: this PID was authenticated by this fixture's Unix socket.
            unsafe {
                libc::kill(owner, libc::SIGKILL);
            }
            let deadline = Instant::now() + Duration::from_secs(2);
            loop {
                let live = Command::new("/bin/ps")
                    .args(["-p", &owner.to_string(), "-o", "stat="])
                    .output()
                    .is_ok_and(|output| {
                        String::from_utf8_lossy(&output.stdout)
                            .lines()
                            .any(|line| !line.trim().is_empty() && !line.trim().starts_with('Z'))
                    });
                if !live {
                    let _ = std::fs::remove_file(&socket);
                    break;
                }
                if Instant::now() >= deadline {
                    eprintln!("fixture cleanup unconfirmed for exact peer {owner}");
                    break;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
}
#[test]
fn second_dashboard_is_refused_and_dead_controller_releases_same_child() {
    let fixture = Fixture::new();
    let mut first = fixture.open();
    first.wait("Discovery complete");
    first.send(b"a");
    first.wait("Adopted");
    first.send(b"l");
    let original = first.wait("fixture>");
    let pid = original
        .split("pid=")
        .nth(1)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_owned();
    let mut second = fixture.open();
    second.wait("Another dashboard controls this supervisor");
    second.wait("Project / worktrees (1)");
    second.send(b"r");
    second.wait("Project / worktrees (1)");
    second.send(b"alq");
    first.send(b"still-owned\r");
    first.wait("received: still-owned");
    fixture.assert_one_owner_and_child();
    first.kill();
    second.kill();
    let mut replacement = fixture.open();
    replacement.wait("fixture>");
    assert!(replacement.wait("pid=").contains(&format!("pid={pid}")));
    replacement.send(b"reconnected\r");
    replacement.wait("received: reconnected");
    replacement.send(&[0x1d]);
    replacement.wait("Focus: dashboard");
    replacement.send(b"q");
    let deadline = Instant::now() + Duration::from_secs(5);
    while replacement.child.try_wait().unwrap().is_none() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn simultaneous_supervisors_recover_stale_socket_and_elect_one_owner() {
    use std::os::unix::{fs::PermissionsExt, net::UnixListener};
    let fixture = Fixture::new();
    std::fs::create_dir(fixture.state()).unwrap();
    std::fs::set_permissions(fixture.state(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let endpoint = fixture.state().join("supervisor.sock");
    drop(UnixListener::bind(&endpoint).unwrap());
    let start = || {
        Command::new(env!("CARGO_BIN_EXE_agent-terminal"))
            .arg("--supervisor")
            .arg(fixture.state())
            .spawn()
            .unwrap()
    };
    let mut contenders = [start(), start()];
    let mut dashboard = fixture.open();
    dashboard.wait("Discovery complete");
    dashboard.send(b"a");
    dashboard.wait("Adopted");
    dashboard.send(b"l");
    dashboard.wait("fixture>");
    assert_eq!(
        contenders
            .iter_mut()
            .map(|process| usize::from(process.try_wait().unwrap().is_none()))
            .sum::<usize>(),
        1,
        "only one racing owner survives"
    );
    assert_eq!(
        std::fs::metadata(fixture.state())
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    assert_eq!(
        std::fs::metadata(endpoint).unwrap().permissions().mode() & 0o777,
        0o600
    );
    fixture.assert_one_owner_and_child();
    dashboard.send(&[0x1d]);
    dashboard.wait("Focus: dashboard");
    dashboard.send(b"q");
    let deadline = Instant::now() + Duration::from_secs(5);
    for process in &mut contenders {
        while process.try_wait().unwrap().is_none() {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

#[test]
fn checkout_aliases_and_repeated_launch_keep_the_existing_child() {
    let fixture = Fixture::new();
    let nested = fixture.repository.join("nested");
    std::fs::create_dir(&nested).unwrap();
    let alias = fixture.temp.path().join("alias");
    std::os::unix::fs::symlink(&nested, &alias).unwrap();
    let mut dashboard = Dashboard::open(&alias, &fixture.state());
    dashboard.wait("Discovery complete");
    dashboard.send(b"a");
    dashboard.wait("Adopted");
    dashboard.send(b"l");
    let original = dashboard.wait("fixture>");
    let pid = original
        .split("pid=")
        .nth(1)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_owned();
    assert!(original.contains(&format!(
        "cwd={}",
        fixture.repository.canonicalize().unwrap().display()
    )));
    dashboard.send(&[0x1d]);
    dashboard.wait("Focus: dashboard");
    dashboard.send(b"al");
    dashboard.wait("A session already exists; attach to it");
    dashboard.send(b"\r");
    assert!(
        dashboard
            .wait("Focus: session")
            .contains(&format!("pid={pid}"))
    );
    dashboard.send(b"one-child\r");
    dashboard.wait("received: one-child");
    fixture.assert_one_owner_and_child();
    dashboard.send(&[0x1d]);
    dashboard.wait("Focus: dashboard");
    dashboard.send(b"q");
}

#[test]
fn unsafe_state_directory_is_explained_before_execution_owned_discovery() {
    let fixture = Fixture::new();
    let target = fixture.temp.path().join("target");
    std::fs::create_dir(&target).unwrap();
    std::os::unix::fs::symlink(&target, fixture.state()).unwrap();
    let mut dashboard = fixture.open();
    dashboard.wait("Project / worktrees (0)");
    dashboard.wait("Supervisor state must be a user-owned private directory");
    dashboard.send(b"r");
    dashboard.wait("Project / worktrees (0)");
    assert!(!target.join("supervisor.sock").exists());
}

#[test]
fn stop_quit_and_native_exit_terminate_quiet_foreground_tool_in_separate_process_group() {
    for action in ["stop", "quit", "exit"] {
        let fixture = Fixture::new();
        let mut sentinel = Command::new("sleep").arg("10").spawn().unwrap();
        let mut dashboard = fixture.open();
        dashboard.wait("Discovery complete");
        dashboard.send(b"a");
        dashboard.wait("Adopted");
        dashboard.send(b"l");
        dashboard.wait("fixture>");
        dashboard.send(b"tool-child\r");
        let heartbeat = fixture.repository.join("tool-child.heartbeat");
        let deadline = Instant::now() + Duration::from_secs(3);
        while std::fs::metadata(&heartbeat).map(|m| m.len()).unwrap_or(0) < 60 {
            assert!(Instant::now() < deadline, "tool child must start");
            std::thread::sleep(Duration::from_millis(20));
        }
        let pid = std::fs::read_to_string(fixture.repository.join("tool-child.pid")).unwrap();
        if action == "exit" {
            dashboard.send(b"exit\r");
        } else {
            dashboard.send(&[0x1d]);
            dashboard.wait("Focus: dashboard");
            dashboard.send(if action == "quit" { b"q" } else { b"s" });
        }
        if action == "quit" {
            let deadline = Instant::now() + Duration::from_secs(3);
            while dashboard.child.try_wait().unwrap().is_none() {
                assert!(
                    Instant::now() < deadline,
                    "ordinary quit must terminate managed activity"
                );
                while let Ok(bytes) = dashboard.output.try_recv() {
                    dashboard.screen.process(&bytes);
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        } else {
            dashboard.wait(if action == "exit" {
                "Process: Exited"
            } else {
                "Process: Stopped"
            });
        }
        let stopped = std::fs::read(&heartbeat).unwrap();
        std::thread::sleep(Duration::from_millis(150));
        assert_eq!(
            std::fs::read(&heartbeat).unwrap(),
            stopped,
            "confirmed Stop/quit must stop tool execution"
        );
        let status = Command::new("/bin/ps")
            .args(["-p", pid.trim(), "-o", "stat="])
            .output()
            .unwrap();
        assert!(
            status.stdout.is_empty()
                || String::from_utf8_lossy(&status.stdout)
                    .trim()
                    .starts_with('Z'),
            "owned foreground tool cannot execute"
        );
        assert!(
            sentinel.try_wait().unwrap().is_none(),
            "unrelated process must remain running"
        );
        sentinel.kill().unwrap();
        sentinel.wait().unwrap();
        if action != "quit" {
            dashboard.send(b"q");
        }
    }
}

#[test]
fn native_exit_retains_context_and_explicit_stop_cleans_refusing_descendant() {
    let fixture = Fixture::new();
    let sentinel = fixture.repository.join("working change");
    std::fs::write(&sentinel, "retain me").unwrap();
    let mut dashboard = fixture.open();
    dashboard.wait("Discovery complete");
    dashboard.send(b"a");
    dashboard.wait("Adopted");
    dashboard.send(b"l");
    dashboard.wait("fixture>");
    dashboard.send(b"exit\r");
    dashboard.wait("Process: Exited");
    let exited = dashboard.wait("task=");
    let task = exited
        .split("task=")
        .nth(1)
        .unwrap()
        .chars()
        .take(36)
        .collect::<String>();
    dashboard.kill();
    let mut readiness = Controller::connect(&fixture);
    cleanup_wait15(&mut readiness, |state| {
        state["metadata_pending"] == false && state["launch_result"].is_object()
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    while cleanup_page15(&mut readiness)["cleanup"]["entries"][0]["eligible"] != true {
        assert!(
            Instant::now() < deadline,
            "native exit must seal its final capture before a fresh launch intent"
        );
    }
    drop(readiness);
    dashboard = fixture.reopen_after_release("Process: Exited");
    dashboard.send(b"l");
    dashboard.wait("Focus: session");
    dashboard.wait(&format!("task={task}"));
    dashboard.send(b"descendant\r");
    let screen = dashboard.wait("descendant pid=");
    let descendant: i32 = screen
        .split("descendant pid=")
        .nth(1)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .parse()
        .unwrap();
    dashboard.send(&[0x1d]);
    dashboard.wait("Focus: dashboard");
    let stopping = Instant::now();
    dashboard.send(b"s");
    dashboard.wait("Process: Stopped");
    let elapsed = stopping.elapsed();
    assert!(
        elapsed >= Duration::from_millis(500),
        "TERM refusal must exercise escalation"
    );
    assert!(elapsed < Duration::from_secs(3));
    eprintln!("TERM-refusing descendant stop confirmed in {elapsed:?}");
    let output = Command::new("ps")
        .args(["-p", &descendant.to_string(), "-o", "stat="])
        .output()
        .unwrap();
    assert!(
        output.stdout.is_empty()
            || String::from_utf8_lossy(&output.stdout)
                .trim()
                .starts_with('Z'),
        "descendant must not execute"
    );
    assert_eq!(std::fs::read_to_string(sentinel).unwrap(), "retain me");
    dashboard.send(b"q");
}

#[test]
fn explicit_detach_restores_dashboard_and_preserves_execution() {
    let fixture = Fixture::new();
    let mut dashboard = fixture.open();
    dashboard.wait("Discovery complete");
    dashboard.send(b"a");
    dashboard.wait("Adopted");
    dashboard.send(b"l");
    dashboard.wait("fixture>");
    dashboard.send(&[0x1d]);
    dashboard.wait("Focus: dashboard");
    dashboard.send(b"d");
    let deadline = Instant::now() + Duration::from_secs(3);
    while dashboard.child.try_wait().unwrap().is_none() {
        assert!(Instant::now() < deadline, "detach should exit promptly");
        std::thread::sleep(Duration::from_millis(20));
    }
    let mut restoration = Vec::new();
    while let Ok(bytes) = dashboard.output.recv_timeout(Duration::from_millis(100)) {
        restoration.extend(bytes);
    }
    assert!(
        restoration.windows(8).any(|bytes| bytes == b"\x1b[?1049l"),
        "detach leaves alternate screen"
    );
    assert!(
        restoration.windows(8).any(|bytes| bytes == b"\x1b[?2004l"),
        "detach disables bracketed paste"
    );
    fixture.assert_one_owner_and_child();
    let mut reopened = fixture.open();
    reopened.wait("fixture>");
    reopened.send(b"identity\r");
    reopened.wait("same-child pid=");
    reopened.send(&[0x1d]);
    reopened.wait("Focus: dashboard");
    reopened.send(b"q");
}

#[test]
fn failed_execution_verification_is_visible_and_quit_retries_after_repair() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new();
    let tools = fixture.temp.path().join("tools");
    std::fs::create_dir(&tools).unwrap();
    let ps = tools.join("ps");
    std::fs::write(&ps, "#!/bin/sh\nexit 1\n").unwrap();
    std::fs::set_permissions(&ps, std::fs::Permissions::from_mode(0o700)).unwrap();
    let path = std::env::join_paths(
        std::iter::once(tools).chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    let mut dashboard =
        Dashboard::open_with_path(&fixture.repository, &fixture.state(), Some(&path));
    dashboard.wait("Discovery complete");
    dashboard.send(b"a");
    dashboard.wait("Adopted");
    dashboard.send(b"l");
    dashboard.wait("fixture>");
    dashboard.send(&[0x1d]);
    dashboard.wait("Focus: dashboard");
    dashboard.send(b"q");
    dashboard.wait("Stop unconfirmed:");
    dashboard.wait("Process: StopFailed");
    assert!(
        dashboard.child.try_wait().unwrap().is_none(),
        "failed quit must stay visible"
    );
    dashboard.wait("task=");
    std::fs::remove_file(&ps).unwrap();
    std::os::unix::fs::symlink("/bin/ps", ps).unwrap();
    dashboard.send(b"q");
    let deadline = Instant::now() + Duration::from_secs(3);
    while dashboard.child.try_wait().unwrap().is_none() {
        assert!(
            Instant::now() < deadline,
            "retry must confirm shutdown after repair"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!fixture.state().join("supervisor.sock").exists());
}

#[test]
fn saturated_native_input_does_not_block_stop_control() {
    use std::io::{BufRead, BufReader};
    use std::os::unix::net::UnixStream;
    let fixture = Fixture::new();
    let mut dashboard = fixture.open();
    dashboard.wait("Discovery complete");
    dashboard.send(b"a");
    dashboard.wait("Adopted");
    dashboard.send(b"l");
    dashboard.wait("fixture>");
    dashboard.send(b"no-read\r");
    dashboard.wait("input paused");
    dashboard.kill();
    std::thread::sleep(Duration::from_millis(100));
    let mut stream = UnixStream::connect(fixture.state().join("supervisor.sock")).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let read = |reader: &mut BufReader<UnixStream>| -> serde_json::Value {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    };
    assert!(!read(&mut reader)["control_denied"].as_bool().unwrap());
    let input = serde_json::to_vec(&serde_json::json!({"Input": vec![120; 4096]})).unwrap();
    let mut saturated = false;
    for _ in 0..100 {
        stream.write_all(&input).unwrap();
        stream.write_all(b"\n").unwrap();
        let observation = read(&mut reader);
        saturated |= observation["error"]
            .as_str()
            .is_some_and(|e| e.contains("Native input queue unavailable"));
    }
    assert!(
        saturated,
        "fixture must actually saturate the bounded writer queue"
    );
    let started = Instant::now();
    stream.write_all(b"\"Shutdown\"\n").unwrap();
    let mut observation = read(&mut reader);
    while !observation["stopped"].as_bool().unwrap() {
        assert!(started.elapsed() < Duration::from_secs(3));
        std::thread::sleep(Duration::from_millis(20));
        stream.write_all(b"\"Screen\"\n").unwrap();
        observation = read(&mut reader);
    }
    assert!(started.elapsed() < Duration::from_secs(3));
    eprintln!(
        "Saturated-input shutdown confirmed in {:?}",
        started.elapsed()
    );
}

#[test]
fn missing_claude_keeps_adopted_checkout_and_allows_fixture_launch() {
    let fixture = Fixture::new();
    let tools = fixture.temp.path().join("tools");
    std::fs::create_dir(&tools).unwrap();
    for (name, target) in [("git", "/usr/bin/git"), ("ps", "/bin/ps")] {
        std::os::unix::fs::symlink(target, tools.join(name)).unwrap();
    }
    let mut dashboard = Dashboard::open_with_path(
        &fixture.repository,
        &fixture.state(),
        Some(tools.as_os_str()),
    );
    dashboard.wait("Discovery complete");
    dashboard.send(b"a");
    dashboard.wait("Adopted");
    dashboard.send(b"c");
    dashboard.wait("Claude Code launch failed:");
    dashboard.wait("adopted checkout retained");
    // Failure can be visible while the cancelled native process/history tail
    // still drains. A subsequent launch is explicit only after that boundary.
    dashboard.kill();
    let mut controller = Controller::connect(&fixture);
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        let state = controller.request(serde_json::json!("Screen"));
        if state["lifecycle"] == "Failed"
            && state["metadata_pending"] == false
            && state["accepted_bytes"] == state["durable_bytes"]
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "failed launch history still pending: {state}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    drop(controller);
    dashboard = fixture.reopen_with_path_after_release("Process: Failed", Some(tools.as_os_str()));
    dashboard.send(b"l");
    dashboard.wait("fixture>");
    fixture.assert_one_owner_and_child();
    dashboard.send(&[0x1d]);
    dashboard.send(b"q");
}

#[test]
fn missing_codex_keeps_adopted_checkout_and_allows_fixture_launch() {
    let fixture = Fixture::new();
    let tools = fixture.temp.path().join("tools");
    std::fs::create_dir(&tools).unwrap();
    for (name, target) in [("git", "/usr/bin/git"), ("ps", "/bin/ps")] {
        std::os::unix::fs::symlink(target, tools.join(name)).unwrap();
    }
    let mut dashboard = Dashboard::open_with_path(
        &fixture.repository,
        &fixture.state(),
        Some(tools.as_os_str()),
    );
    dashboard.wait("Discovery complete");
    dashboard.send(b"a");
    dashboard.wait("Adopted");
    dashboard.send(b"x");
    dashboard.wait("Codex CLI launch failed:");
    dashboard.wait("adopted checkout retained");
    // Failure can be visible while the cancelled native process/history tail
    // still drains. A subsequent launch is explicit only after that boundary.
    dashboard.kill();
    let mut controller = Controller::connect(&fixture);
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        let state = controller.request(serde_json::json!("Screen"));
        if state["lifecycle"] == "Failed"
            && state["metadata_pending"] == false
            && state["accepted_bytes"] == state["durable_bytes"]
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "failed launch history still pending: {state}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    drop(controller);
    dashboard = fixture.reopen_with_path_after_release("Process: Failed", Some(tools.as_os_str()));
    dashboard.send(b"l");
    dashboard.wait("fixture>");
    fixture.assert_one_owner_and_child();
    dashboard.send(&[0x1d]);
    dashboard.send(b"q");
}

struct Controller {
    stream: std::os::unix::net::UnixStream,
    reader: std::io::BufReader<std::os::unix::net::UnixStream>,
    greeting: serde_json::Value,
}
impl Controller {
    fn connect(fixture: &Fixture) -> Self {
        use std::io::BufRead;
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let stream =
                std::os::unix::net::UnixStream::connect(fixture.state().join("supervisor.sock"))
                    .unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let greeting: serde_json::Value = serde_json::from_str(&line).unwrap();
            if greeting["control_denied"] == false {
                return Self {
                    stream,
                    reader,
                    greeting,
                };
            }
            assert!(Instant::now() < deadline, "dead controller not released");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    fn send(&mut self, value: serde_json::Value) {
        serde_json::to_writer(&mut self.stream, &value).unwrap();
        self.stream.write_all(b"\n").unwrap();
    }
    fn request(&mut self, value: serde_json::Value) -> serde_json::Value {
        let launch = value.as_object().is_some_and(|object| {
            object.keys().any(|key| {
                matches!(
                    key.as_str(),
                    "Launch" | "LaunchClaude" | "LaunchCodex" | "Resume" | "QueryLaunch"
                )
            })
        });
        let mut state = self.request_now(value);
        let deadline = Instant::now() + Duration::from_secs(3);
        while launch && state["metadata_pending"] == true && state["launch_result"].is_null() {
            assert!(
                Instant::now() < deadline,
                "launch metadata still pending: {state}"
            );
            std::thread::sleep(Duration::from_millis(10));
            state = self.request_now(serde_json::json!("Screen"));
        }
        state
    }
    fn request_now(&mut self, value: serde_json::Value) -> serde_json::Value {
        use std::io::BufRead;
        self.send(value);
        let mut line = String::new();
        self.reader.read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    }
}

#[test]
fn lost_launch_acknowledgement_recovers_original_request_and_child() {
    let fixture = Fixture::new();
    let mut dashboard = fixture.open();
    dashboard.wait("Discovery complete");
    dashboard.send(b"a");
    dashboard.wait("Adopted");
    dashboard.kill();
    let mut first = Controller::connect(&fixture);
    let request = first.greeting["launch_request"]
        .as_str()
        .expect("supervisor provides stable intent before launch")
        .to_owned();
    first.send(serde_json::json!({"Launch":request}));
    // Discard the acknowledgement; the process can die immediately after sending.
    drop(first);
    let mut recovered = Controller::connect(&fixture);
    let result = recovered.request(serde_json::json!({"QueryLaunch":request}));
    assert_eq!(result["launch_result"]["request"], request);
    assert!(result["launch_result"]["error"].is_null());
    let identity = result["identity"].clone();
    assert!(identity.as_str().unwrap().contains(&request));
    let retried = recovered.request(serde_json::json!({"Launch":request}));
    assert_eq!(retried["identity"], identity);
    assert!(
        retried["error"].is_null(),
        "same request returns success, not duplicate refusal"
    );
    fixture.assert_one_owner_and_child();
    drop(recovered);
    // Client EOF is observed asynchronously; retry only after explicit refusal.
    let mut reopened = fixture.reopen_after_release("fixture>");
    reopened.wait("fixture>");
    reopened.send(b"identity\r");
    reopened.wait("same-child pid=");
    reopened.send(&[0x1d]);
    reopened.wait("Focus: dashboard");
    reopened.send(b"q");
}

#[test]
fn lost_failed_launch_result_is_recovered_without_executing_after_adoption() {
    for launch in ["Launch", "LaunchClaude", "LaunchCodex"] {
        let fixture = Fixture::new();
        let mut dashboard = fixture.open();
        dashboard.wait("Discovery complete");
        dashboard.kill();
        let mut first = Controller::connect(&fixture);
        let request = first.greeting["launch_request"]
            .as_str()
            .unwrap()
            .to_owned();
        first.send(serde_json::json!({(launch):request}));
        drop(first);
        let mut recovered = Controller::connect(&fixture);
        let failed = recovered.request(serde_json::json!({"QueryLaunch":request}));
        assert_eq!(
            failed["launch_result"]["error"],
            "Adopt a checkout before launch"
        );
        let adopted = recovered.request(
            serde_json::json!({"Adopt":fixture.repository.as_os_str().as_encoded_bytes()}),
        );
        assert!(adopted["error"].is_null());
        let retry = recovered.request(serde_json::json!({(launch):request}));
        assert_eq!(retry["launch_result"], failed["launch_result"]);
        assert_eq!(retry["error"], "Adopt a checkout before launch");
        assert!(
            retry["identity"].is_null(),
            "failed retry never creates a child"
        );
        assert_eq!(retry["lifecycle"], "Idle");
        recovered.request(serde_json::json!("Shutdown"));
    }
}

#[test]
fn streaming_child_keeps_screen_sequence_identity_and_resize_after_dashboard_death() {
    let fixture = Fixture::new();
    let mut dashboard = fixture.open();
    dashboard.wait("Discovery complete");
    dashboard.send(b"a");
    dashboard.wait("Adopted");
    dashboard.send(b"l");
    let original = dashboard.wait("fixture>");
    let pid = original
        .split("pid=")
        .nth(1)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_owned();
    dashboard.send(b"stream\r");
    dashboard.wait("stream: 0");
    dashboard.kill();
    let mut recovered = Controller::connect(&fixture);
    let identity = recovered.greeting["identity"].clone();
    let mut previous = recovered.greeting["output_bytes"].as_u64().unwrap();
    let start = previous;
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let snapshot = recovered.request(serde_json::json!("Screen"));
        let sequence = snapshot["output_bytes"].as_u64().unwrap();
        assert!(sequence >= previous, "snapshots cannot go backwards");
        assert_eq!(snapshot["identity"], identity);
        previous = sequence;
        if snapshot["screen"]
            .as_str()
            .unwrap()
            .contains("stream: 0199")
        {
            assert_eq!(
                snapshot["screen"]
                    .as_str()
                    .unwrap()
                    .matches("stream: 0199")
                    .count(),
                1
            );
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        previous > start,
        "child must continue producing after takeover"
    );
    let resized = recovered.request(serde_json::json!({"Resize":{"rows":18,"cols":64}}));
    assert_eq!(
        (resized["rows"].as_u64(), resized["cols"].as_u64()),
        (Some(18), Some(64))
    );
    recovered.request(serde_json::json!({"Input":b"size\r"}));
    loop {
        let snapshot = recovered.request(serde_json::json!("Screen"));
        if snapshot["screen"].as_str().unwrap().contains("size: 64x18") {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(20));
    }
    fixture.assert_one_owner_and_child();
    drop(recovered);
    // Client EOF is observed asynchronously; retry only after explicit refusal.
    let mut reopened = fixture.reopen_after_release("stream: 0199");
    reopened.wait("stream: 0199");
    reopened.send(b"identity\r");
    reopened.wait(&format!("same-child pid={pid}"));
    reopened.send(&[0x1d]);
    reopened.wait("Focus: dashboard");
    reopened.send(b"r");
    reopened.wait("Project / worktrees (1)");
    reopened.send(b"q");
}

#[test]
fn stopped_history_survives_restart_and_missing_checkout_without_launching() {
    let fixture = Fixture::new();
    let mut dashboard = fixture.open();
    dashboard.wait("Discovery complete");
    dashboard.send(b"a");
    dashboard.wait("Adopted");
    dashboard.send(b"l");
    dashboard.wait("fixture>");
    dashboard.send(b"retained-context-08\r");
    dashboard.wait("received: retained-context-08");
    dashboard.send(b"exit\r");
    dashboard.wait("Process: Exited");
    dashboard.send(b"q");
    let deadline = Instant::now() + Duration::from_secs(8);
    while dashboard.child.try_wait().unwrap().is_none() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(20));
    }
    std::fs::rename(
        &fixture.repository,
        fixture.temp.path().join("removed-checkout"),
    )
    .unwrap();
    let mut restored = fixture.open();
    restored.wait("Process: Exited");
    restored.send(b"v");
    restored.wait("Checkout unavailable: cannot continue in place");
    restored.send(b"h");
    restored.wait("Retained history");
    restored.wait("received: retained-context-08");
    restored.wait("\\u{1b}[?2004h");
    assert!(
        !restored
            .screen
            .screen()
            .contents()
            .contains("Focus: session")
    );
    restored.send(b"q");
}

#[test]
fn chronological_history_retains_erased_and_alternate_screen_text_safely() {
    let fixture = Fixture::new();
    let mut dashboard = fixture.open();
    dashboard.wait("Discovery complete");
    dashboard.send(b"a");
    dashboard.wait("Adopted");
    dashboard.send(b"l");
    dashboard.wait("fixture>");
    dashboard.send(b"history-controls\r");
    dashboard.wait("after-history-controls");
    dashboard.send(b"exit\r");
    dashboard.wait("Process: Exited");
    dashboard.send(b"h");
    let history = dashboard.wait("erased-response-08");
    assert!(history.contains("alternate-response-08"));
    assert!(history.contains("\\u{1b}[2K"));
    assert!(
        history.find("erased-response-08").unwrap()
            < history.find("alternate-response-08").unwrap()
    );
    dashboard.send(b"q");
}

#[test]
fn abrupt_supervisor_loss_recovers_acknowledged_prefix_without_automatic_coding() {
    let fixture = Fixture::new();
    let mut dashboard = fixture.open();
    dashboard.wait("Discovery complete");
    dashboard.send(b"a");
    dashboard.wait("Adopted");
    dashboard.send(b"l");
    let initial = dashboard.wait("fixture>");
    let child_pid = initial
        .split("pid=")
        .nth(1)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_string();
    dashboard.send(b"durable-prefix-08\r");
    dashboard.wait("received: durable-prefix-08");
    dashboard.kill();
    let mut control = Controller::connect(&fixture);
    let identity = control.greeting["identity"].clone();
    let deadline = Instant::now() + Duration::from_secs(3);
    let durable = loop {
        let state = control.request(serde_json::json!("Screen"));
        if state["durable_bytes"].as_u64().unwrap_or(0) >= state["output_bytes"].as_u64().unwrap()
            && state["durable_flush_count"].as_u64().unwrap_or(0) > 0
        {
            break state;
        }
        assert!(
            Instant::now() < deadline,
            "pending output never became durable: {state}"
        );
        std::thread::sleep(Duration::from_millis(10));
    };
    eprintln!(
        "durable prefix: {} bytes; accepted-to-durable {} ms; sync+commit {} us",
        durable["durable_bytes"],
        durable["accepted_to_durable_millis"],
        durable["sync_commit_micros"]
    );
    let processes = Command::new("ps")
        .args(["-Ao", "pid,ppid,command"])
        .output()
        .unwrap();
    let owner: String = String::from_utf8(processes.stdout)
        .unwrap()
        .lines()
        .find(|line| {
            line.contains("--supervisor") && line.contains(fixture.state().to_str().unwrap())
        })
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_owned();
    assert!(
        Command::new("kill")
            .args(["-KILL", &owner])
            .status()
            .unwrap()
            .success()
    );
    drop(control);
    // The fixture process identity is known directly from this isolated run;
    // cleanup is test responsibility, not product reconciliation evidence.
    let _ = Command::new("kill").args(["-KILL", &child_pid]).status();
    let mut restored = fixture.open();
    restored.wait("Process: Interrupted");
    restored.wait("No automatic restart");
    restored.send(b"h");
    restored.wait("received: durable-prefix-08");
    restored.kill();
    let mut control = Controller::connect(&fixture);
    assert_eq!(control.greeting["identity"], identity);
    let blocked = control.request(serde_json::json!({"Launch":"replacement-must-not-run"}));
    assert!(blocked["error"].as_str().unwrap().contains("uncertain"));
    control.request(serde_json::json!("Shutdown"));
}

#[test]
fn healthy_output_reports_real_durable_boundaries_within_one_second() {
    let fixture = Fixture::new();
    let mut dashboard = fixture.open();
    dashboard.wait("Discovery complete");
    dashboard.send(b"a");
    dashboard.wait("Adopted");
    dashboard.send(b"l");
    dashboard.wait("fixture>");
    dashboard.kill();
    let mut control = Controller::connect(&fixture);
    control.request(serde_json::json!({"Input":b"stream\rstream\rstream\r".to_vec()}));
    let started = Instant::now();
    let mut samples = Vec::new();
    let mut previous_count = 0;
    let mut last = serde_json::Value::Null;
    while started.elapsed() < Duration::from_secs(4) {
        let state = control.request(serde_json::json!("Screen"));
        let count = state["durable_flush_count"].as_u64().unwrap();
        if count > previous_count {
            samples.push((
                state["durable_completed_millis"].as_u64().unwrap(),
                state["accepted_to_durable_millis"].as_u64().unwrap(),
                state["sync_commit_micros"].as_u64().unwrap(),
            ));
            previous_count = count;
        }
        last = state;
        std::thread::sleep(Duration::from_millis(10));
    }
    eprintln!(
        "durable samples (completion ms, accepted age ms, file sync + SQLite commit us): {samples:?}"
    );
    assert!(
        samples.len() >= 10,
        "must observe streaming boundaries, not idle timer ticks"
    );
    for sample in &samples {
        assert!(
            sample.1 < 1000,
            "healthy accepted output exceeded target: {sample:?}"
        );
    }
    for pair in samples.windows(2) {
        assert!(
            pair[1].0 - pair[0].0 < 1000,
            "healthy durable interval exceeded target: {pair:?}"
        );
    }
    let commits = last["durable_commits"]
        .as_array()
        .expect("bounded per-commit telemetry");
    assert_eq!(
        commits.len() as u64,
        previous_count,
        "short run must retain every commit"
    );
    for (index, commit) in commits.iter().enumerate() {
        assert_eq!(commit["sequence"].as_u64(), Some(index as u64 + 1));
    }
    assert_eq!(last["durable_bytes"], last["output_bytes"]);
    control.request(serde_json::json!("Shutdown"));
}

#[test]
fn version_one_metadata_migrates_through_real_supervisor_without_losing_history() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new();
    std::fs::create_dir(fixture.state()).unwrap();
    std::fs::set_permissions(fixture.state(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let db = rusqlite::Connection::open(fixture.state().join("history.sqlite3")).unwrap();
    db.execute_batch("CREATE TABLE app (id INTEGER PRIMARY KEY CHECK(id=1), state TEXT NOT NULL, checkout BLOB, task TEXT NOT NULL); CREATE TABLE sessions (id TEXT PRIMARY KEY, identity TEXT NOT NULL, state TEXT NOT NULL, durable INTEGER NOT NULL DEFAULT 0); PRAGMA user_version=1;").unwrap();
    drop(db);
    let mut dashboard = fixture.open();
    dashboard.wait("Discovery complete");
    dashboard.send(b"a");
    dashboard.wait("Adopted");
    dashboard.send(b"l");
    dashboard.wait("fixture>");
    dashboard.send(b"migrated-history-08\r");
    dashboard.wait("received: migrated-history-08");
    dashboard.send(b"exit\r");
    dashboard.wait("Process: Exited");
    dashboard.send(b"h");
    dashboard.wait("received: migrated-history-08");
    dashboard.send(b"q");
}

#[test]
fn failed_final_storage_commit_keeps_quit_visible_until_explicit_retry() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new();
    let mut dashboard = fixture.open();
    dashboard.wait("Discovery complete");
    dashboard.send(b"a");
    dashboard.wait("Adopted");
    dashboard.send(b"l");
    dashboard.wait("fixture>");
    std::fs::set_permissions(fixture.state(), std::fs::Permissions::from_mode(0o500)).unwrap();
    dashboard.send(b"pending-final-history-08\r");
    dashboard.wait("received: pending-final-history-08");
    dashboard.send(&[0x1d]);
    dashboard.wait("Focus: dashboard");
    dashboard.send(b"q");
    // SQLite can report readonly-database or disk-I/O errors for this real
    // directory-permission failure. Assert the visible unsaved/retained fault.
    dashboard.wait_any(&[
        "History capture blocked:",
        "History storage failed:",
        "History sync failed:",
    ]);
    assert!(
        dashboard.child.try_wait().unwrap().is_none(),
        "failed storage cannot acknowledge quit"
    );
    std::fs::set_permissions(fixture.state(), std::fs::Permissions::from_mode(0o700)).unwrap();
    dashboard.send(b"q");
    let deadline = Instant::now() + Duration::from_secs(8);
    while dashboard.child.try_wait().unwrap().is_none() {
        assert!(
            Instant::now() < deadline,
            "retry must commit the retained pending output"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let mut restored = fixture.open();
    restored.send(b"h");
    restored.wait("received: pending-final-history-08");
    restored.send(b"q");
}

#[test]
fn bounded_history_pages_recover_every_record_across_multiple_chunks() {
    let fixture = Fixture::new();
    let mut dashboard = fixture.open();
    dashboard.wait("Discovery complete");
    dashboard.send(b"a");
    dashboard.wait("Adopted");
    dashboard.send(b"l");
    dashboard.wait("fixture>");
    dashboard.send(b"history-bulk\r");
    dashboard.wait("history-bulk-complete");
    dashboard.send(b"exit\r");
    dashboard.wait("Process: Exited");
    dashboard.kill();
    let mut control = Controller::connect(&fixture);
    let mut offset = 0;
    let mut text = String::new();
    loop {
        let deadline = Instant::now() + Duration::from_secs(3);
        let page = loop {
            let observation =
                control.request(serde_json::json!({"History":{"index":0,"offset":offset}}));
            if observation["history"]["offset"].as_u64() == Some(offset) {
                break observation["history"].clone();
            }
            assert!(Instant::now() < deadline);
        };
        let part = page["text"].as_str().unwrap();
        assert!(
            part.len() <= 2048 * 10,
            "readable page must have a finite expansion bound"
        );
        text.push_str(part);
        match page["next"].as_u64() {
            Some(next) => {
                assert!(next > offset);
                offset = next;
            }
            None => break,
        }
    }
    let records: Vec<u32> = text
        .lines()
        .filter_map(|line| {
            line.split_once("bulk08:")
                .map(|(_, rest)| rest)
                .map(|line| line[..5].parse().unwrap())
        })
        .collect();
    assert_eq!(records, (0..6000).collect::<Vec<_>>());
    let chunks: Vec<_> = std::fs::read_dir(fixture.state())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "transcript")
        })
        .collect();
    assert!(chunks.len() >= 7);
    for chunk in chunks {
        assert!(std::fs::metadata(chunk).unwrap().len() <= 65536);
    }
    control.request(serde_json::json!("Shutdown"));
}

fn assert_no_owned_coding(fixture: &Fixture) {
    let owner = owner_pid(fixture);
    let output = Command::new("ps")
        .args(["-Ao", "pid,ppid,command"])
        .output()
        .unwrap();
    let processes = String::from_utf8(output.stdout).unwrap();
    assert!(
        !processes.lines().any(
            |line| line.split_whitespace().nth(1) == Some(owner.as_str())
                && (line.contains("--scripted-child") || line.contains("--launch-gate"))
        ),
        "unexpected automatic coding: {processes}"
    );
}
fn owner_pid(fixture: &Fixture) -> String {
    let output = Command::new("ps")
        .args(["-Ao", "pid,command"])
        .output()
        .unwrap();
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .find(|line| {
            line.contains("--supervisor") && line.contains(fixture.state().to_str().unwrap())
        })
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_owned()
}
fn kill_owner(fixture: &Fixture) {
    assert!(
        Command::new("kill")
            .args(["-KILL", &owner_pid(fixture)])
            .status()
            .unwrap()
            .success()
    );
}
fn wait_reconciled(control: &mut Controller) {
    control.request(serde_json::json!("Reconcile"));
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let state = control.request(serde_json::json!("Screen"));
        if state["execution"].is_null() && state["metadata_pending"] == false {
            break;
        }
        assert!(Instant::now() < deadline, "absence not reconciled: {state}");
        std::thread::sleep(Duration::from_millis(20));
    }
}
#[test]
fn launch_crash_windows_retain_request_and_require_explicit_reconciliation() {
    for window in [
        "before-create",
        "after-create",
        "before-result",
        "after-result",
    ] {
        let fixture = Fixture::new();
        let sentinel = fixture.repository.join("working.txt");
        std::fs::write(&sentinel, "uncommitted work must survive").unwrap();
        let mut dashboard = fixture.open();
        dashboard.wait("Discovery complete");
        dashboard.send(b"a");
        dashboard.wait("Adopted");
        dashboard.kill();
        let mut control = Controller::connect(&fixture);
        let request = control.greeting["launch_request"].clone();
        std::fs::write(fixture.state().join("proof-launch-crash"), window).unwrap();
        control.send(serde_json::json!({"Launch":request}));
        drop(control);
        let deadline = Instant::now() + Duration::from_secs(6);
        while fixture.state().join("proof-launch-crash").exists() {
            assert!(
                Instant::now() < deadline,
                "crash barrier was not reached: {window}"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        std::thread::sleep(Duration::from_millis(100));
        let mut dashboard = fixture.open();
        dashboard.wait("Process: Interrupted");
        dashboard.kill();
        let mut control = Controller::connect(&fixture);
        let original =
            control.request(serde_json::json!({"QueryLaunch":request}))["launch_result"].clone();
        assert_eq!(original["request"], request, "{window}");
        assert_eq!(
            original["error"].is_null(),
            window == "after-result",
            "only committed successful exec outcome is known: {window}"
        );
        assert_no_owned_coding(&fixture);
        eprintln!(
            "launch crash {window}: restored Interrupted, owned coding children=0, request={request}"
        );
        let replay = control.request(serde_json::json!({"Launch":request}));
        assert_eq!(
            replay["launch_result"], original,
            "same request never respawns: {window}"
        );
        let blocked =
            control.request(serde_json::json!({"Launch":format!("replacement-{window}")}));
        assert!(blocked["error"].as_str().unwrap().contains("uncertain"));
        if window == "before-create" || window == "after-create" {
            wait_reconciled(&mut control);
            assert_eq!(
                control.request(serde_json::json!({"QueryLaunch":request}))["launch_result"],
                original
            );
            let launched =
                control.request(serde_json::json!({"Launch":"explicit-after-reconciliation"}));
            assert!(launched["launch_result"]["error"].is_null(), "{launched}");
            fixture.assert_one_owner_and_child();
        }
        assert_eq!(
            std::fs::read_to_string(sentinel).unwrap(),
            "uncommitted work must survive"
        );
        control.request(serde_json::json!("Shutdown"));
    }
}

#[test]
fn surviving_native_execution_blocks_replacement_then_different_boot_reconciles() {
    let fixture = Fixture::new();
    let sentinel = fixture.repository.join("working.txt");
    std::fs::write(&sentinel, "original uncommitted work").unwrap();
    let mut dashboard = fixture.open();
    dashboard.wait("Discovery complete");
    dashboard.send(b"a");
    dashboard.wait("Adopted");
    dashboard.send(b"l");
    dashboard.wait("fixture>");
    dashboard.send(b"retained-recovery-context-09\r");
    dashboard.wait("received: retained-recovery-context-09");
    dashboard.send(b"survive\r");
    dashboard.wait("survival enabled");
    dashboard.kill();
    let mut control = Controller::connect(&fixture);
    let pid = control.greeting["execution"]["pid"].as_i64().unwrap();
    let result = control.greeting["launch_result"].clone();
    let request = result["request"].clone();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let state = control.request(serde_json::json!("Screen"));
        if state["durable_bytes"].as_u64().unwrap_or(0) >= state["output_bytes"].as_u64().unwrap() {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(20));
    }
    kill_owner(&fixture);
    drop(control);
    let mut dashboard = fixture.open();
    dashboard.wait("Process: Interrupted");
    dashboard.kill();
    let mut control = Controller::connect(&fixture);
    let reconciled = control.request(serde_json::json!("Reconcile"));
    assert!(
        reconciled["error"]
            .as_str()
            .unwrap()
            .contains("Verified surviving process"),
        "{reconciled}"
    );
    assert_eq!(
        control.request(serde_json::json!({"QueryLaunch":request}))["launch_result"],
        result
    );
    let resume = control.request(serde_json::json!({"Resume":"must-not-resume-survivor"}));
    assert!(resume["error"].as_str().unwrap().contains("uncertain"));
    let blocked = control.request(serde_json::json!({"Launch":"must-not-compete"}));
    assert!(blocked["error"].as_str().unwrap().contains("uncertain"));
    let stop = control.request(serde_json::json!("Stop"));
    assert_eq!(stop["stopped"], false);
    assert_eq!(stop["lifecycle"], "Interrupted");
    assert_eq!(
        unsafe { libc::kill(pid as i32, 0) },
        0,
        "reconciliation never kills a survivor"
    );
    // Harness owns this fixture identity, unlike production recovery. Simulate a
    // reboot's no-live-process outcome before changing the OS boot-evidence seam.
    assert!(
        Command::new("kill")
            .args(["-KILL", &pid.to_string()])
            .status()
            .unwrap()
            .success()
    );
    let absent = control.request(serde_json::json!("Reconcile"));
    assert!(
        !absent["execution"].is_null(),
        "same-boot leader absence does not prove descendant absence"
    );
    kill_owner(&fixture);
    drop(control);
    let mut dashboard = Dashboard::open_config(
        &fixture.repository,
        &fixture.state(),
        None,
        Some("different-boot-09"),
    );
    dashboard.wait("Process: Interrupted");
    dashboard.kill();
    let mut control = Controller::connect(&fixture);
    wait_reconciled(&mut control);
    assert_no_owned_coding(&fixture);
    assert_eq!(
        control.request(serde_json::json!({"QueryLaunch":request}))["launch_result"],
        result
    );
    let deadline = Instant::now() + Duration::from_secs(3);
    let page = loop {
        let state = control.request(serde_json::json!({"History":{"index":0,"offset":0}}));
        if state["history"]["offset"] == 0 {
            break state["history"].clone();
        }
        assert!(Instant::now() < deadline);
    };
    assert!(
        page["text"]
            .as_str()
            .unwrap()
            .contains("received: retained-recovery-context-09")
    );
    let launched = control.request(serde_json::json!({"Launch":"explicit-new-boot-conversation"}));
    assert!(launched["launch_result"]["error"].is_null(), "{launched}");
    fixture.assert_one_owner_and_child();
    assert_eq!(
        std::fs::read_to_string(sentinel).unwrap(),
        "original uncommitted work"
    );
    control.request(serde_json::json!("Shutdown"));
}

#[test]
fn failed_gate_exec_result_survives_supervisor_restart_without_retrying_native_code() {
    let fixture = Fixture::new();
    let tools = fixture.temp.path().join("empty-native-path");
    std::fs::create_dir(&tools).unwrap();
    for (name, target) in [("git", "/usr/bin/git"), ("ps", "/bin/ps")] {
        std::os::unix::fs::symlink(target, tools.join(name)).unwrap();
    }
    let mut dashboard = Dashboard::open_with_path(
        &fixture.repository,
        &fixture.state(),
        Some(tools.as_os_str()),
    );
    dashboard.wait("Discovery complete");
    dashboard.send(b"a");
    dashboard.wait("Adopted");
    dashboard.send(b"c");
    dashboard.wait("Claude Code launch failed:");
    dashboard.wait("Process: Failed");
    dashboard.kill();
    let control = Controller::connect(&fixture);
    assert_eq!(control.greeting["lifecycle"], "Failed");
    assert_ne!(control.greeting["process_exit"]["code"], 0);
    let outcome = control.greeting["process_exit"].clone();
    let failed = control.greeting["launch_result"].clone();
    let request = failed["request"].clone();
    assert!(
        failed["error"]
            .as_str()
            .unwrap()
            .contains("Claude Code launch failed")
    );
    kill_owner(&fixture);
    drop(control);
    let mut dashboard = fixture.open();
    dashboard.wait("Process: Failed");
    dashboard.kill();
    let mut control = Controller::connect(&fixture);
    assert_eq!(control.greeting["process_exit"], outcome);
    assert_eq!(
        control.request(serde_json::json!({"LaunchClaude":request}))["launch_result"],
        failed
    );
    assert_no_owned_coding(&fixture);
    control.request(serde_json::json!("Shutdown"));
}

#[test]
fn legacy_interrupted_record_anchors_observation_boot_then_recovers_after_later_boot() {
    use std::os::unix::{ffi::OsStrExt, fs::PermissionsExt};
    let fixture = Fixture::new();
    std::fs::create_dir(fixture.state()).unwrap();
    std::fs::set_permissions(fixture.state(), std::fs::Permissions::from_mode(0o700)).unwrap();
    // A literal version-one legacy fixture, consumed through the real supervisor.
    let db = rusqlite::Connection::open(fixture.state().join("history.sqlite3")).unwrap();
    db.execute_batch("CREATE TABLE app (id INTEGER PRIMARY KEY CHECK(id=1), state TEXT NOT NULL, checkout BLOB, task TEXT NOT NULL); CREATE TABLE sessions (id TEXT PRIMARY KEY, identity TEXT NOT NULL, state TEXT NOT NULL, durable INTEGER NOT NULL DEFAULT 0); PRAGMA user_version=1;").unwrap();
    let old = serde_json::json!({"adopted":"legacy checkout", "identity":null, "screen":"", "formatted":"", "rows":24, "cols":100, "output_bytes":0, "error":null, "stopped":false, "lifecycle":"Interrupted", "control_denied":false, "launch_request":"legacy-uncertain-09"});
    db.execute(
        "INSERT INTO app VALUES(1,?1,?2,'legacy-task')",
        rusqlite::params![old.to_string(), fixture.repository.as_os_str().as_bytes()],
    )
    .unwrap();
    drop(db);
    let mut dashboard = fixture.open();
    dashboard.wait("Process: Interrupted");
    dashboard.kill();
    let mut control = Controller::connect(&fixture);
    let uncertain = control.request(serde_json::json!("Reconcile"));
    assert!(uncertain["error"].as_str().unwrap().contains("Legacy"));
    assert_eq!(
        uncertain["execution"]["boot"], "",
        "do not invent historical launch boot"
    );
    let anchor = uncertain["execution"]["observed_boot"]
        .as_str()
        .expect("first recovery observation must provide future reboot route")
        .to_owned();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if control.request(serde_json::json!("Screen"))["metadata_pending"] == false {
            break;
        }
        assert!(Instant::now() < deadline);
    }
    kill_owner(&fixture);
    drop(control);
    let mut dashboard = fixture.open();
    dashboard.wait("Process: Interrupted");
    dashboard.kill();
    let mut control = Controller::connect(&fixture);
    assert_eq!(control.greeting["execution"]["observed_boot"], anchor);
    assert!(!control.request(serde_json::json!("Reconcile"))["execution"].is_null());
    kill_owner(&fixture);
    drop(control);
    let mut dashboard = Dashboard::open_config(
        &fixture.repository,
        &fixture.state(),
        None,
        Some("legacy-future-boot-09"),
    );
    dashboard.wait("Process: Interrupted");
    dashboard.kill();
    let mut control = Controller::connect(&fixture);
    wait_reconciled(&mut control);
    assert_no_owned_coding(&fixture);
    let result = control.request(serde_json::json!({"Launch":"explicit-after-legacy-recovery"}));
    assert!(result["launch_result"]["error"].is_null(), "{result}");
    fixture.assert_one_owner_and_child();
    control.request(serde_json::json!("Shutdown"));
}

#[test]
fn launch_gate_preserves_native_arguments_environment_and_checkout() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new();
    let tools = fixture.temp.path().join("native tools with spaces");
    std::fs::create_dir(&tools).unwrap();
    let codex = tools.join("codex");
    std::fs::write(&codex, "#!/bin/sh\nfor arg in \"$@\"; do\n if [ \"$arg\" = --no-daemon ]; then printf 'native-no-daemon-preserved\\r\\n'; fi\ndone\nprintf 'native-term:%s\\r\\n' \"$TERM\"\nprintf 'native-cwd:%s\\r\\n' \"$PWD\"\nexec /bin/cat\n").unwrap();
    std::fs::set_permissions(codex, std::fs::Permissions::from_mode(0o700)).unwrap();
    let path = std::env::join_paths(
        std::iter::once(tools).chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    let mut dashboard =
        Dashboard::open_with_path(&fixture.repository, &fixture.state(), Some(&path));
    dashboard.wait("Discovery complete");
    dashboard.send(b"a");
    dashboard.wait("Adopted");
    dashboard.send(b"x");
    dashboard.wait("native-no-daemon-preserved");
    dashboard.wait("native-term:xterm-256color");
    dashboard.wait(&format!("native-cwd:{}", fixture.repository.display()));
    dashboard.send(&[0x1d]);
    dashboard.send(b"q");
}

#[test]
fn codex_status_requires_current_attributed_evidence_despite_liveness_and_prose() {
    use std::os::unix::{fs::PermissionsExt, net::UnixDatagram};
    let fixture = Fixture::new();
    let tools = fixture.temp.path().join("tools");
    std::fs::create_dir(&tools).unwrap();
    for (name, target) in [("git", "/usr/bin/git"), ("ps", "/bin/ps")] {
        std::os::unix::fs::symlink(target, tools.join(name)).unwrap();
    }
    // Reuse the shared scripted child through the normal Codex launch boundary.
    let executable = env!("CARGO_BIN_EXE_agent-terminal").replace('\'', "'\\''");
    let wrapper = tools.join("codex");
    std::fs::write(
        &wrapper,
        format!("#!/bin/sh\nexec '{executable}' --scripted-child\n"),
    )
    .unwrap();
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut dashboard = Dashboard::open_with_path(
        &fixture.repository,
        &fixture.state(),
        Some(tools.as_os_str()),
    );
    dashboard.wait("Discovery complete");
    dashboard.send(b"a");
    dashboard.wait("Adopted");
    dashboard.send(b"x");
    dashboard.wait("fixture>");
    dashboard.wait("Codex evidence unavailable");
    dashboard.send(b"Conversation interrupted; Worked for 3s; approval needed\r");
    dashboard.wait("received: Conversation interrupted");
    assert!(
        dashboard
            .wait("semantic status: unknown")
            .contains("Process: Running")
    );
    let channel = UnixDatagram::unbound().unwrap();
    channel
        .send_to(b"{", fixture.state().join("codex-status.sock"))
        .unwrap();
    dashboard.wait("invalid or unattributed Codex evidence");
    dashboard.send(b"fixture-failed-test\r");
    dashboard.wait("test result: FAILED; agent continues");
    assert!(
        dashboard
            .wait("semantic status: unknown")
            .contains("Process: Running")
    );
    dashboard.send(&[0x1d]);
    dashboard.send(b"q");
}

#[test]
fn codex_native_identity_survives_exit_and_resume_uses_original_id_without_new_prompt() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new();
    let tools = fixture.temp.path().join("native-tools");
    std::fs::create_dir(&tools).unwrap();
    let python = Command::new("which").arg("python3").output().unwrap();
    let python = String::from_utf8(python.stdout).unwrap();
    let wrapper = tools.join("codex");
    let script = r#"import sys,os,re,json,shlex,subprocess
args=sys.argv[1:]
id='11111111-2222-4333-8444-555555555555'
if 'resume' in args:
    assert args[args.index('resume')+1]==id,args
    assert '--no-daemon' in args,args
    print('resumed-original-id-without-followup',flush=True)
for arg in args:
    if arg.startswith('hooks.SessionStart='):
        helper=json.loads(re.search(r'command=("(?:[^"\\]|\\.)*")',arg).group(1))
        subprocess.run(shlex.split(helper),input=json.dumps({'hook_event_name':'SessionStart','session_id':id}).encode(),check=True)
os.execv(sys.argv[0]+'.child',[sys.argv[0]+'.child'])
"#;
    std::fs::write(&wrapper, format!("#!{}\n{script}", python.trim())).unwrap();
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o700)).unwrap();
    let child = tools.join("codex.child");
    std::fs::write(
        &child,
        format!(
            "#!/bin/sh\nexec '{}' --scripted-child\n",
            env!("CARGO_BIN_EXE_agent-terminal").replace('\'', "'\\''")
        ),
    )
    .unwrap();
    std::fs::set_permissions(child, std::fs::Permissions::from_mode(0o700)).unwrap();
    let path = std::env::join_paths(
        std::iter::once(tools).chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    let mut dashboard =
        Dashboard::open_with_path(&fixture.repository, &fixture.state(), Some(&path));
    dashboard.wait("Discovery complete");
    dashboard.send(b"a");
    dashboard.wait("Adopted");
    dashboard.send(b"x");
    dashboard.wait("fixture>");
    dashboard.send(b"original-readable-context-16\r");
    dashboard.wait("received: original-readable-context-16");
    dashboard.send(b"exit\r");
    dashboard.wait("Process: Exited");
    dashboard.kill();
    let mut control = Controller::connect(&fixture);
    assert_eq!(
        control.greeting["native_conversation"],
        "11111111-2222-4333-8444-555555555555"
    );
    let old_identity = control.greeting["identity"].clone();
    let deadline = Instant::now() + Duration::from_secs(5);
    while control.request(serde_json::json!("Screen"))["metadata_pending"] != false {
        assert!(Instant::now() < deadline);
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    while cleanup_page15(&mut control)["cleanup"]["entries"][0]["eligible"] != true {
        assert!(
            Instant::now() < deadline,
            "original native segment final capture must be sealed before crash"
        );
    }
    kill_owner(&fixture);
    drop(control);
    let mut dashboard = fixture.reopen_with_path_after_release("Process: Exited", Some(&path));
    dashboard.kill();
    let control = Controller::connect(&fixture);
    assert_eq!(
        control.greeting["native_conversation"],
        "11111111-2222-4333-8444-555555555555"
    );
    assert_eq!(control.greeting["identity"], old_identity);
    assert_no_owned_coding(&fixture);
    let mut refused = Dashboard::open_with_path(&fixture.repository, &fixture.state(), Some(&path));
    refused.wait("Another dashboard controls this supervisor");
    refused.kill();
    drop(control);
    let mut dashboard = fixture.reopen_with_path_after_release("Process: Exited", Some(&path));
    dashboard.send(b"v");
    dashboard.wait("resumed-original-id-without-followup");
    dashboard.wait("fixture>");
    dashboard.send(b"exit\r");
    dashboard.wait("Process: Exited");
    dashboard.send(b"h");
    dashboard.send(b"n");
    dashboard.wait("received: original-readable-context-16");
    dashboard.kill();
    let mut control = Controller::connect(&fixture);
    let resumed_id = control.greeting["identity"]
        .as_str()
        .unwrap()
        .split("session=")
        .nth(1)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_owned();
    let old_id = old_identity
        .as_str()
        .unwrap()
        .split("session=")
        .nth(1)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_owned();
    let native = control.greeting["native_conversation"].clone();
    cleanup_wait15(&mut control, |state| state["metadata_pending"] == false);
    let deadline = Instant::now() + Duration::from_secs(5);
    while cleanup_page15(&mut control)["cleanup"]["entries"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["eligible"] != true)
    {
        assert!(Instant::now() < deadline);
    }
    let resumed_files = cleanup_chunks15(&fixture, &resumed_id);
    assert!(!resumed_files.is_empty());
    control.request(serde_json::json!({"DeleteHistory":{"id":old_id}}));
    let deleted = cleanup_wait15(&mut control, |state| {
        state["cleanup_message"]
            .as_str()
            .unwrap_or("")
            .contains("completed")
    });
    assert_eq!(deleted["native_conversation"], native);
    assert_eq!(cleanup_chunks15(&fixture, &resumed_id), resumed_files);
    assert!(cleanup_chunks15(&fixture, &old_id).is_empty());
    control.request(serde_json::json!({"History":{"index":0,"offset":0}}));
    let retained = cleanup_wait15(&mut control, |state| state["history"].is_object());
    assert!(
        retained["history"]["identity"]
            .as_str()
            .unwrap()
            .contains(&resumed_id)
    );
    control.request(serde_json::json!("Shutdown"));
}

#[test]
fn unavailable_native_recovery_retains_history_until_explicit_new_conversation() {
    let fixture = Fixture::new();
    let mut dashboard = fixture.open();
    dashboard.wait("Discovery complete");
    dashboard.send(b"a");
    dashboard.wait("Adopted");
    dashboard.send(b"l");
    dashboard.wait("fixture>");
    dashboard.send(b"independent-history-16\r");
    dashboard.wait("received: independent-history-16");
    dashboard.send(b"exit\r");
    dashboard.wait("Process: Exited");
    dashboard.send(b"v");
    dashboard.wait("Native resume unavailable: no supported saved conversation identity");
    dashboard.send(b"h");
    dashboard.wait("received: independent-history-16");
    dashboard.send(b"h");
    dashboard.send(b"l");
    dashboard.wait("fixture>");
    dashboard.send(&[0x1d]);
    dashboard.send(b"q");
}

#[test]
fn slow_subscriber_disconnects_with_reason_and_reconnects_coherently() {
    let fixture = Fixture::new();
    let mut dashboard = fixture.open();
    dashboard.wait("Discovery complete");
    dashboard.send(b"a");
    dashboard.wait("Adopted");
    dashboard.send(b"l");
    dashboard.wait("fixture>");
    dashboard.kill();
    let mut slow = Controller::connect(&fixture);
    let identity = slow.greeting["identity"].clone();
    slow.request(serde_json::json!({"Resize":{"rows":80,"cols":200}}));
    slow.request(serde_json::json!({"Input":b"mosaic\r".to_vec()}));
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let state = slow.request(serde_json::json!("Screen"));
        if state["screen"]
            .as_str()
            .unwrap()
            .contains("max geometry complete")
        {
            break;
        }
        assert!(Instant::now() < deadline);
    }
    // Stop reading the real supervisor transport. A maximum styled screen is
    // larger than the socket buffer, forcing partial writes and real lag.
    slow.send(serde_json::json!("Screen"));
    std::thread::sleep(Duration::from_millis(50));
    let since = Instant::now();
    let competing =
        std::os::unix::net::UnixStream::connect(fixture.state().join("supervisor.sock")).unwrap();
    competing
        .set_read_timeout(Some(Duration::from_millis(300)))
        .unwrap();
    let mut refusal = String::new();
    std::io::BufRead::read_line(&mut std::io::BufReader::new(competing), &mut refusal).unwrap();
    let refusal: serde_json::Value = serde_json::from_str(&refusal).unwrap();
    assert_eq!(refusal["control_denied"], true);
    assert!(
        since.elapsed() < Duration::from_millis(300),
        "ownership refusal must not wait for slow snapshot"
    );
    std::thread::sleep(Duration::from_millis(750));
    drop(slow);
    let mut resumed = Controller::connect(&fixture);
    assert_eq!(resumed.greeting["identity"], identity);
    assert!(
        resumed.greeting["error"]
            .as_str()
            .unwrap_or("")
            .contains("Slow dashboard"),
        "missing explicit slow-subscriber reason"
    );
    assert!(
        resumed.greeting["screen"]
            .as_str()
            .unwrap()
            .contains("max geometry complete")
    );
    let state = resumed.request(serde_json::json!({"Input":b"identity\r".to_vec()}));
    assert_eq!(state["identity"], identity);
    resumed.request(serde_json::json!("Shutdown"));
}

fn measurement_clock() -> u64 {
    let mut time = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: writable timespec, same kernel monotonic clock as the child.
    assert_eq!(
        unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut time) },
        0
    );
    time.tv_sec as u64 * 1_000_000_000 + time.tv_nsec as u64
}

#[test]
fn sustained_capture_crosses_caps_while_attached_focused_detached_and_lagged() {
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    };
    let fixture = Fixture::new();
    assert!(
        Command::new("git")
            .arg("-C")
            .arg(&fixture.repository)
            .args([
                "-c",
                "user.name=Proof",
                "-c",
                "user.email=proof@example.invalid",
                "commit",
                "--allow-empty",
                "-m",
                "proof13"
            ])
            .output()
            .unwrap()
            .status
            .success()
    );
    assert!(
        Command::new("git")
            .arg("-C")
            .arg(&fixture.repository)
            .args(["worktree", "add", "-b", "other13"])
            .arg(fixture.temp.path().join("other13"))
            .output()
            .unwrap()
            .status
            .success()
    );
    let evidence = std::env::var_os("RAT_RESOURCE_EVIDENCE")
        .map(PathBuf::from)
        .unwrap_or_else(|| fixture.temp.path().join("13-resource.jsonl"));
    let file = Arc::new(Mutex::new(
        std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&evidence)
            .unwrap(),
    ));
    let origin = Instant::now();
    let record = |value: serde_json::Value| {
        writeln!(file.lock().unwrap(), "{value}").unwrap();
    };
    record(
        serde_json::json!({"kind":"manifest", "revision":String::from_utf8(Command::new("git").args(["rev-parse","HEAD"]).output().unwrap().stdout).unwrap().trim(), "tracked_source_dirty":!Command::new("git").args(["diff","HEAD","--quiet"]).status().unwrap().success(), "profile":"debug", "record_bytes":128,"records_per_phase":32768,"phases":["attached","dashboard-focused","fully-detached","slow-subscriber"],"capture_pending_cap":270336,"frame_cap":4194304,"screen":[80,200],"scrollback":0,"rss_sample_period_ms":100}),
    );
    for (command, args) in [
        ("uname", vec!["-mrs"]),
        ("rustc", vec!["--version"]),
        (
            "sysctl",
            vec!["-n", "hw.model", "hw.memsize", "hw.logicalcpu"],
        ),
    ] {
        record(
            serde_json::json!({"kind":"environment","command":command,"output":String::from_utf8_lossy(&Command::new(command).args(args).output().unwrap().stdout)}),
        );
    }
    let mut dashboard = fixture.open();
    dashboard.wait("Discovery complete");
    dashboard.send(b"a");
    dashboard.wait("Adopted");
    dashboard.send(b"l");
    let start = dashboard.wait("fixture>");
    let child: u32 = start
        .split("pid=")
        .nth(1)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .parse()
        .unwrap();
    let processes = String::from_utf8(
        Command::new("ps")
            .args(["-Ao", "pid,ppid,command"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    let owner: u32 = processes
        .lines()
        .find(|line| {
            line.contains("--supervisor") && line.contains(fixture.state().to_str().unwrap())
        })
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .parse()
        .unwrap();
    let pids = Arc::new(Mutex::new(vec![
        owner,
        child,
        dashboard.child.process_id().unwrap(),
    ]));
    record(
        serde_json::json!({"kind":"pids","supervisor":owner,"child":child,"dashboard":dashboard.child.process_id()}),
    );
    let done = Arc::new(AtomicBool::new(false));
    let sampler = {
        let file = file.clone();
        let done = done.clone();
        let pids = pids.clone();
        std::thread::spawn(move || {
            while !done.load(Ordering::Relaxed) {
                let list = pids
                    .lock()
                    .unwrap()
                    .iter()
                    .map(u32::to_string)
                    .collect::<Vec<_>>()
                    .join(",");
                let since = Instant::now();
                let output = Command::new("ps")
                    .args(["-p", &list, "-o", "pid=,ppid=,rss=,etime=,command="])
                    .output()
                    .unwrap();
                writeln!(file.lock().unwrap(),"{}",serde_json::json!({"kind":"rss","elapsed_us":origin.elapsed().as_micros(),"sampler_us":since.elapsed().as_micros(),"ps":String::from_utf8_lossy(&output.stdout)})).unwrap();
                std::thread::sleep(Duration::from_millis(100));
            }
        })
    };
    dashboard.send(b"flood13\r");
    let complete = dashboard.wait("flood13-complete run=1");
    record(
        serde_json::json!({"kind":"phase","mode":"attached","elapsed_us":origin.elapsed().as_micros(),"visible":complete}),
    );
    dashboard.send(b"flood13\r");
    dashboard.send(&[0x1d]);
    dashboard.wait("Focus: dashboard");
    for _ in 0..5 {
        let injected_ns = measurement_clock();
        let since = Instant::now();
        dashboard.send(b"j");
        dashboard.wait("Branch: other13");
        record(
            serde_json::json!({"kind":"navigation","key":"j","visible_us":since.elapsed().as_micros(),"injected_ns":injected_ns,"visible_ns":measurement_clock()}),
        );
        let injected_ns = measurement_clock();
        let since = Instant::now();
        dashboard.send(b"k");
        dashboard.wait("Branch: main");
        record(
            serde_json::json!({"kind":"navigation","key":"k","visible_us":since.elapsed().as_micros(),"injected_ns":injected_ns,"visible_ns":measurement_clock()}),
        );
        dashboard.send(b"r");
    }
    let attach_injected_ns = measurement_clock();
    let since = Instant::now();
    dashboard.send(b"\r");
    dashboard.wait("Focus: session");
    let screen_ready_us = since.elapsed().as_micros();
    dashboard.send(b"attach13-nonce\r");
    dashboard.wait("received: attach13-nonce");
    record(
        serde_json::json!({"kind":"attach","focus_frame_us":screen_ready_us,"screen_and_input_visible_us":since.elapsed().as_micros(),"injected_ns":attach_injected_ns,"visible_ns":measurement_clock(),"boundary":"coherent replacement snapshots and unique fixture input ACK; child may finish its unpaced burst before reading input"}),
    );
    let complete = dashboard.wait("flood13-complete run=2");
    record(
        serde_json::json!({"kind":"phase","mode":"dashboard-focused","elapsed_us":origin.elapsed().as_micros(),"visible":complete}),
    );
    dashboard.kill();
    let mut control = Controller::connect(&fixture);
    let identity = control.greeting["identity"].clone();
    control.request(serde_json::json!({"Input":b"flood13\r".to_vec()}));
    drop(control);
    std::thread::sleep(Duration::from_secs(3));
    let mut control = Controller::connect(&fixture);
    assert!(
        control.greeting["screen"]
            .as_str()
            .unwrap()
            .contains("flood13-complete run=3")
    );
    record(
        serde_json::json!({"kind":"phase","mode":"fully-detached","elapsed_us":origin.elapsed().as_micros(),"observation":control.greeting}),
    );
    control.request(serde_json::json!({"Resize":{"rows":80,"cols":200}}));
    control.request(serde_json::json!({"Input":b"mosaic\r".to_vec()}));
    let deadline = Instant::now() + Duration::from_secs(5);
    while !control.request(serde_json::json!("Screen"))["screen"]
        .as_str()
        .unwrap()
        .contains("max geometry complete")
    {
        assert!(Instant::now() < deadline);
    }
    control.send(serde_json::json!({"Input":b"flood13\r".to_vec()}));
    // The response captures the large styled screen before the child erases it;
    // do not read that actual socket, forcing its one-frame backlog to saturate.
    std::thread::sleep(Duration::from_secs(3));
    drop(control);
    let mut control = Controller::connect(&fixture);
    assert_eq!(control.greeting["identity"], identity);
    assert!(
        control.greeting["screen"]
            .as_str()
            .unwrap()
            .contains("flood13-complete run=4")
    );
    assert!(
        control.greeting["error"]
            .as_str()
            .unwrap_or("")
            .contains("Slow dashboard")
    );
    record(
        serde_json::json!({"kind":"phase","mode":"slow-subscriber","elapsed_us":origin.elapsed().as_micros(),"observation":control.greeting}),
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    let final_state = loop {
        let state = control.request(serde_json::json!("Screen"));
        if state["durable_bytes"] == state["output_bytes"] {
            break state;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(20));
    };
    assert!(final_state["capture_high_water"].as_u64().unwrap() <= 66 * 4096);
    assert!(
        final_state["capture_backpressure_count"].as_u64().unwrap() > 0,
        "workload must actually saturate capture queue"
    );
    assert!(final_state["subscriber_frame_high_water"].as_u64().unwrap() <= 4 * 1024 * 1024);
    record(serde_json::json!({"kind":"bounds","state":final_state}));
    control.request(serde_json::json!({"Resize":{"rows":24,"cols":100}}));
    control.request(serde_json::json!({"Input":b"exit\r".to_vec()}));
    let mut offset = 0;
    let mut text = String::new();
    loop {
        let deadline = Instant::now() + Duration::from_secs(3);
        let page = loop {
            let state = control.request(serde_json::json!({"History":{"index":0,"offset":offset}}));
            if state["history"]["offset"].as_u64() == Some(offset) {
                break state["history"].clone();
            }
            assert!(Instant::now() < deadline);
        };
        assert!(
            text.len() < 32 * 1024 * 1024,
            "finite fixture history bound"
        );
        text.push_str(page["text"].as_str().unwrap());
        if let Some(next) = page["next"].as_u64() {
            offset = next;
        } else {
            break;
        }
    }
    let records: Vec<u32> = text
        .lines()
        .filter_map(|line| {
            line.split_once("flood13:")
                .and_then(|(_, rest)| rest.get(..6))
                .and_then(|n| n.parse().ok())
        })
        .collect();
    assert_eq!(
        records,
        (0..4).flat_map(|_| 0..32768).collect::<Vec<_>>(),
        "history must contain each numbered record exactly once across lag and reconnect"
    );
    for line in text
        .lines()
        .filter(|line| line.contains("flood13-complete"))
    {
        record(serde_json::json!({"kind":"producer","line":line}));
    }
    for line in text
        .lines()
        .filter(|line| line.contains("input13-received_ns="))
    {
        record(
            serde_json::json!({"kind":"fixture_input_received","line":line,"boundary":"fixture parsed nonce after reading CR; monotonic kernel clock, before writing visible ACK"}),
        );
    }
    record(
        serde_json::json!({"kind":"continuity","records":records.len(),"durable_bytes":final_state["durable_bytes"],"elapsed_us":origin.elapsed().as_micros()}),
    );
    control.request(serde_json::json!("Shutdown"));
    done.store(true, Ordering::Relaxed);
    sampler.join().unwrap();
    eprintln!("resource evidence: {}", evidence.display());
}

#[test]
fn disconnected_dashboard_retains_identity_and_detaches_locally_before_reopen() {
    let fixture = Fixture::new();
    let mut dashboard = fixture.open();
    dashboard.wait("Discovery complete");
    dashboard.send(b"a");
    dashboard.wait("Adopted");
    dashboard.send(b"l");
    let original = dashboard.wait("fixture>");
    let pid = original
        .split("pid=")
        .nth(1)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_owned();
    let processes = Command::new("ps")
        .args(["-Ao", "pid,ppid,command"])
        .output()
        .unwrap();
    let processes = String::from_utf8(processes.stdout).unwrap();
    let owner: i32 = processes
        .lines()
        .find(|line| {
            line.contains("--supervisor") && line.contains(fixture.state().to_str().unwrap())
        })
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .parse()
        .unwrap();
    struct Resume(i32);
    impl Drop for Resume {
        fn drop(&mut self) {
            // SAFETY: only the isolated test supervisor's known PID is resumed.
            unsafe {
                libc::kill(self.0, libc::SIGCONT);
            }
        }
    }
    let resume = Resume(owner);
    // SAFETY: pause only the isolated test supervisor to force a real transport timeout.
    assert_eq!(unsafe { libc::kill(owner, libc::SIGSTOP) }, 0);
    dashboard.wait("press d to detach locally");
    let lost = dashboard.wait("Process: unknown (last seen Running)");
    assert!(lost.contains("session="));
    assert!(!lost.contains("Process: Idle"));
    dashboard.send(b"d");
    let deadline = Instant::now() + Duration::from_secs(3);
    while dashboard.child.try_wait().unwrap().is_none() {
        assert!(
            Instant::now() < deadline,
            "d must exit without waiting on dead transport"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    drop(resume);
    let mut reopened = fixture.open();
    assert!(reopened.wait("fixture>").contains(&format!("pid={pid}")));
    reopened.send(b"identity\r");
    reopened.wait(&format!("same-child pid={pid}"));
    reopened.send(&[0x1d]);
    reopened.send(b"q");
}

#[test]
fn prior_boot_native_record_stays_interrupted_until_explicit_reconcile_then_resume() {
    use std::os::unix::{ffi::OsStrExt, fs::PermissionsExt};
    let fixture = Fixture::new();
    std::fs::create_dir(fixture.state()).unwrap();
    std::fs::set_permissions(fixture.state(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let tools = fixture.temp.path().join("tools");
    std::fs::create_dir(&tools).unwrap();
    let wrapper = tools.join("claude");
    std::fs::write(&wrapper,format!("#!/bin/sh\nprintf 'native-recovery-args:'\nprintf '<%s>' \"$@\"\nprintf '\\r\\n'\nexec '{}' --scripted-child\n",env!("CARGO_BIN_EXE_agent-terminal").replace('\'',"'\\''"))).unwrap();
    std::fs::set_permissions(wrapper, std::fs::Permissions::from_mode(0o700)).unwrap();
    let path = std::env::join_paths(
        std::iter::once(tools).chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    let db = rusqlite::Connection::open(fixture.state().join("history.sqlite3")).unwrap();
    db.execute_batch("CREATE TABLE app (id INTEGER PRIMARY KEY CHECK(id=1), state TEXT NOT NULL, checkout BLOB, task TEXT NOT NULL); CREATE TABLE sessions (id TEXT PRIMARY KEY, identity TEXT NOT NULL, state TEXT NOT NULL, durable INTEGER NOT NULL DEFAULT 0); PRAGMA user_version=1;").unwrap();
    let identity =
        "task=task16 session=33333333-4444-4555-8666-777777777777 request=prior native16 λ";
    let old = serde_json::json!({"adopted":fixture.repository.display().to_string(),"identity":identity,"agent":"Claude Code","native_conversation":"11111111-2222-4333-8444-555555555555","screen":"","formatted":"","rows":24,"cols":100,"output_bytes":0,"error":null,"stopped":false,"lifecycle":"Running","control_denied":false,"launch_request":"prior native16 λ","execution":{"boot":"injected-prior-boot-16","pid":null,"start":null,"released":true}});
    db.execute(
        "INSERT INTO app VALUES(1,?1,?2,'task16')",
        rusqlite::params![old.to_string(), fixture.repository.as_os_str().as_bytes()],
    )
    .unwrap();
    db.execute(
        "INSERT INTO sessions VALUES(?1,?2,?3,0)",
        rusqlite::params![
            "33333333-4444-4555-8666-777777777777",
            identity,
            old.to_string()
        ],
    )
    .unwrap();
    drop(db);
    let mut dashboard =
        Dashboard::open_with_path(&fixture.repository, &fixture.state(), Some(&path));
    dashboard.wait("Process: Interrupted");
    dashboard.kill();
    let mut control = Controller::connect(&fixture);
    assert_no_owned_coding(&fixture);
    assert_eq!(control.greeting["identity"], identity);
    assert_eq!(control.greeting["session_identity"]["task"], "task16");
    assert_eq!(
        control.greeting["session_identity"]["session"],
        "33333333-4444-4555-8666-777777777777"
    );
    assert_eq!(
        control.greeting["session_identity"]["request"],
        "prior native16 λ"
    );
    let blocked = control.request(serde_json::json!({"Resume":"before-reconciliation-16"}));
    assert!(blocked["error"].as_str().unwrap().contains("uncertain"));
    wait_reconciled(&mut control);
    assert_no_owned_coding(&fixture);
    drop(control);
    let mut dashboard = fixture.reopen_with_path_after_release("Process: Interrupted", Some(&path));
    dashboard.send(b"v");
    dashboard.wait("native-recovery-args:<--resume><11111111-2222-4333-8444-555555555555>");
    dashboard.wait("fixture>");

    dashboard.send(&[0x1d]);
    dashboard.send(b"q");
}

#[test]
fn oversized_osc_is_captured_without_growing_live_screen_history() {
    let fixture = Fixture::new();
    let mut dashboard = fixture.open();
    dashboard.wait("Discovery complete");
    dashboard.send(b"a");
    dashboard.wait("Adopted");
    dashboard.send(b"l");
    dashboard.wait("fixture>");
    dashboard.kill();
    let owner = owner_pid(&fixture);
    let rss = || -> u64 {
        String::from_utf8(
            Command::new("ps")
                .args(["-p", &owner, "-o", "rss="])
                .output()
                .unwrap()
                .stdout,
        )
        .unwrap()
        .trim()
        .parse()
        .unwrap()
    };
    let mut control = Controller::connect(&fixture);
    let before = control.greeting["output_bytes"].as_u64().unwrap();
    let before_rss = rss();
    control.request(serde_json::json!({"Input":b"osc13\r".to_vec()}));
    let deadline = Instant::now() + Duration::from_secs(10);
    let after = loop {
        let state = control.request(serde_json::json!("Screen"));
        if state["screen"].as_str().unwrap().contains("osc13-complete")
            && state["durable_bytes"] == state["output_bytes"]
        {
            break state;
        }
        assert!(Instant::now() < deadline, "oversized OSC never drained");
        std::thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(
        after["output_bytes"].as_u64().unwrap() - before,
        8_388_638,
        "raw capture includes the entire oversized control and final marker"
    );
    let after_rss = rss();
    eprintln!(
        "OSC13 supervisor RSS before/after: {before_rss}/{after_rss} KiB, captured8MiB control"
    );
    assert!(
        after_rss <= before_rss + 4096,
        "live OSC parser retained payload-sized memory: {before_rss}->{after_rss}KiB"
    );
    let page = loop {
        let state = control.request(serde_json::json!({"History":{"index":0,"offset":before}}));
        if state["history"]["offset"].as_u64() == Some(before) {
            break state["history"].clone();
        }
        assert!(Instant::now() < deadline);
    };
    assert!(
        page["text"]
            .as_str()
            .unwrap()
            .starts_with("\\u{1b}]2;qqqqqq")
    );
    control.request(serde_json::json!("Shutdown"));
}

#[test]
fn storage_stalled_prepare_keeps_stop_control_and_never_launches_after_cancel() {
    let fixture = Fixture::new();
    let mut dashboard = fixture.open();
    dashboard.wait("Discovery complete");
    dashboard.send(b"a");
    dashboard.wait("Adopted");
    dashboard.kill();
    let mut controller = Controller::connect(&fixture);
    std::fs::write(fixture.state().join("proof-storage-prepare"), "stall").unwrap();
    let started = Instant::now();
    let launch = controller.request_now(serde_json::json!({"Launch":"cancel-stalled-14"}));
    assert!(started.elapsed() < Duration::from_secs(1));
    assert_eq!(launch["metadata_pending"], true);
    assert!(launch["launch_result"].is_null());
    let stopped = controller.request(serde_json::json!("Stop"));
    assert_ne!(stopped["lifecycle"], "Running");
    std::fs::remove_file(fixture.state().join("proof-storage-prepare")).unwrap();
    for _ in 0..30 {
        std::thread::sleep(Duration::from_millis(20));
        let state = controller.request(serde_json::json!("Screen"));
        assert_ne!(
            state["lifecycle"], "Running",
            "late save must not resurrect launch"
        );
    }
    controller.request(serde_json::json!("Shutdown"));
}

fn wait_for_completion_injection(fixture: &Fixture) {
    let deadline = Instant::now() + Duration::from_secs(3);
    let ready = fixture.state().join("proof-storage-completion.ready");
    while !ready.exists() {
        assert!(
            Instant::now() < deadline,
            "completion injection did not reach the storage boundary"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn mismatched_durable_completions_cannot_create_a_coding_process() {
    let fixture = Fixture::new();
    let mut dashboard = fixture.open();
    dashboard.wait("Discovery complete");
    dashboard.send(b"a");
    dashboard.wait("Adopted");
    dashboard.kill();
    let mut controller = Controller::connect(&fixture);
    let hold = fixture.state().join("proof-storage-completion");
    std::fs::write(&hold, "prepare").unwrap();
    controller.request_now(serde_json::json!({"Launch":"attributed-prepare"}));
    wait_for_completion_injection(&fixture);
    let pending = controller.request_now(serde_json::json!("Screen"));
    assert_eq!(
        pending["metadata_pending"], true,
        "matching completion is held"
    );
    assert_ne!(
        pending["lifecycle"], "Running",
        "unmatched successes must not create a process"
    );
    assert!(pending["launch_result"].is_null());
    std::fs::remove_file(hold).unwrap();
    let launched = controller.request(serde_json::json!({"QueryLaunch":"attributed-prepare"}));
    assert_eq!(launched["launch_result"]["request"], "attributed-prepare");
    fixture.assert_one_owner_and_child();
    controller.request(serde_json::json!("Shutdown"));
}

#[test]
fn mismatched_final_capture_completion_cannot_acknowledge_shutdown() {
    let fixture = Fixture::new();
    let mut dashboard = fixture.open();
    dashboard.wait("Discovery complete");
    dashboard.send(b"a");
    dashboard.wait("Adopted");
    dashboard.kill();
    let mut controller = Controller::connect(&fixture);
    controller.request(serde_json::json!({"Launch":"attributed-final"}));
    let hold = fixture.state().join("proof-storage-completion");
    std::fs::write(&hold, "flush").unwrap();
    controller.request(serde_json::json!("Stop"));
    wait_for_completion_injection(&fixture);
    controller.request_now(serde_json::json!("Shutdown"));
    std::thread::sleep(Duration::from_millis(200));
    let shutdown = controller.request_now(serde_json::json!("Screen"));
    assert_ne!(
        shutdown["stopped"], true,
        "only exact final capture receipt permits shutdown acknowledgement"
    );
    std::fs::remove_file(hold).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let state = controller.request_now(serde_json::json!("Screen"));
        if state["stopped"] == true {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "final receipt did not settle shutdown: {state}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn mismatched_result_completions_leave_live_execution_and_result_provisional() {
    let fixture = Fixture::new();
    let mut dashboard = fixture.open();
    dashboard.wait("Discovery complete");
    dashboard.send(b"a");
    dashboard.wait("Adopted");
    dashboard.kill();
    let mut controller = Controller::connect(&fixture);
    let hold = fixture.state().join("proof-storage-completion");
    std::fs::write(&hold, "result").unwrap();
    controller.request_now(serde_json::json!({"Launch":"attributed-result"}));
    wait_for_completion_injection(&fixture);
    let pending = controller.request_now(serde_json::json!("Screen"));
    assert_eq!(pending["metadata_pending"], true);
    assert!(pending["launch_result"].is_null());
    assert_eq!(pending["lifecycle"], "Running");
    assert!(
        pending["error"].is_null(),
        "unmatched errors cannot overwrite execution facts: {pending}"
    );
    fixture.assert_one_owner_and_child();
    // Native controls belong to the same live owner while the exact result
    // completion is held. Neither a snapshot nor another metadata save can
    // acknowledge that provisional result.
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let screen = controller.request_now(serde_json::json!("Screen"));
        if screen["screen"].as_str().unwrap().contains("fixture>") {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "native fixture did not become ready: {screen}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    controller.request_now(serde_json::json!({"Resize":{"rows":19,"cols":73}}));
    controller.request_now(serde_json::json!({"Input":b"size\r".to_vec()}));
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let screen = controller.request_now(serde_json::json!("Screen"));
        assert_eq!(screen["metadata_pending"], true);
        assert!(screen["launch_result"].is_null());
        assert_eq!(screen["execution"], pending["execution"]);
        if screen["screen"].as_str().unwrap().contains("size: 73x19") {
            assert_eq!(screen["rows"], 19);
            assert_eq!(screen["cols"], 73);
            break;
        }
        assert!(
            Instant::now() < deadline,
            "provisional controls stalled: {screen}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    controller.request_now(serde_json::json!("Stop"));
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        let stopped = controller.request_now(serde_json::json!("Screen"));
        assert_eq!(stopped["metadata_pending"], true);
        assert!(stopped["launch_result"].is_null());
        assert_eq!(stopped["session_identity"], pending["session_identity"]);
        if stopped["lifecycle"] == "Stopped" {
            assert!(stopped["execution"].is_null());
            break;
        }
        assert!(
            Instant::now() < deadline,
            "provisional Stop stalled: {stopped}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    std::fs::remove_file(hold).unwrap();
    let settled = controller.request(serde_json::json!({"QueryLaunch":"attributed-result"}));
    assert_eq!(settled["launch_result"]["request"], "attributed-result");
    assert!(settled["launch_result"]["error"].is_null());
    assert_eq!(settled["session_identity"], pending["session_identity"]);
    assert_eq!(settled["lifecycle"], "Stopped");
    assert!(settled["execution"].is_null());
    controller.request(serde_json::json!("Shutdown"));
}

#[test]
fn duplicate_and_prior_session_completions_cannot_replace_current_execution_facts() {
    let fixture = Fixture::new();
    let mut dashboard = fixture.open();
    dashboard.wait("Discovery complete");
    dashboard.send(b"a");
    dashboard.wait("Adopted");
    dashboard.kill();
    let mut controller = Controller::connect(&fixture);
    let first = controller.request(serde_json::json!({"Launch":"old-attribution"}));
    controller.request(serde_json::json!("Stop"));
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let stopped = controller.request_now(serde_json::json!("Screen"));
        if stopped["lifecycle"] == "Stopped" && stopped["metadata_pending"] == false {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "old capture did not settle: {stopped}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    std::fs::write(fixture.state().join("proof-storage-replay"), "replay").unwrap();
    let second = controller.request(serde_json::json!({"Launch":"new-attribution"}));
    assert_eq!(second["lifecycle"], "Running");
    assert!(
        second["error"].is_null(),
        "stale failure replaced newer execution: {second}"
    );
    assert_eq!(second["launch_result"]["request"], "new-attribution");
    assert_ne!(second["session_identity"], first["session_identity"]);
    fixture.assert_one_owner_and_child();
    let replay = controller.request(serde_json::json!({"Launch":"new-attribution"}));
    assert_eq!(replay["execution"], second["execution"]);
    assert_eq!(replay["launch_result"], second["launch_result"]);
    controller.request(serde_json::json!("Shutdown"));
}

#[test]
fn lifecycle_metadata_cannot_settle_failed_launch_result_and_retry_keeps_execution_stopped() {
    let fixture = Fixture::new();
    let mut dashboard = fixture.open();
    dashboard.wait("Discovery complete");
    dashboard.send(b"a");
    dashboard.wait("Adopted");
    dashboard.kill();
    let mut controller = Controller::connect(&fixture);
    let fault = fixture.state().join("proof-storage-save");
    std::fs::write(&fault, "error").unwrap();
    controller.request_now(serde_json::json!({"Launch":"failed-attributed-result"}));
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let state = controller.request_now(serde_json::json!("Screen"));
        if state["error"]
            .as_str()
            .is_some_and(|error| error.contains("outcome not durable"))
        {
            break;
        }
        assert!(Instant::now() < deadline, "missing failed result: {state}");
        std::thread::sleep(Duration::from_millis(10));
    }
    std::fs::remove_file(fault).unwrap();
    controller.request(serde_json::json!("Stop"));
    std::thread::sleep(Duration::from_millis(800));
    let stopped = controller.request_now(serde_json::json!("Screen"));
    assert_eq!(stopped["lifecycle"], "Stopped");
    assert_eq!(
        stopped["metadata_pending"], true,
        "lifecycle metadata cannot discharge the result obligation"
    );
    assert!(stopped["launch_result"].is_null());
    controller.request(serde_json::json!("RetryStorage"));
    let durable = controller.request(serde_json::json!({"QueryLaunch":"failed-attributed-result"}));
    assert_eq!(
        durable["launch_result"]["request"],
        "failed-attributed-result"
    );
    assert!(durable["launch_result"]["error"].is_null());
    assert_eq!(durable["lifecycle"], "Stopped");
    assert!(durable["execution"].is_null());
    controller.request(serde_json::json!("Shutdown"));
}

fn transcript_bytes14(fixture: &Fixture) -> Vec<u8> {
    let mut paths: Vec<_> = std::fs::read_dir(fixture.state())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "transcript")
        })
        .collect();
    paths.sort();
    paths
        .into_iter()
        .flat_map(|path| std::fs::read(path).unwrap())
        .collect()
}

#[test]
fn storage_partial_enospc_retains_exact_capture_and_retry_removes_error_after_durability() {
    let fixture = Fixture::new();
    let mut dashboard = fixture.open();
    dashboard.wait("Discovery complete");
    dashboard.send(b"a");
    dashboard.wait("Adopted");
    dashboard.send(b"l");
    dashboard.wait("fixture>");
    dashboard.kill();
    let mut controller = Controller::connect(&fixture);
    let deadline = Instant::now() + Duration::from_secs(3);
    let before = loop {
        let state = controller.request(serde_json::json!("Screen"));
        if state["durable_bytes"] == state["output_bytes"] {
            break state;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    };
    // Land six bytes before chunk rotation; the injected seven-byte write is
    // clamped to that real file boundary and must not advance the durable prefix.
    let padding = 65530 - before["durable_bytes"].as_u64().unwrap() - 9;
    controller.request(serde_json::json!({"Input":format!("pad14 {padding}\r").into_bytes()}));
    let deadline = Instant::now() + Duration::from_secs(4);
    let before = loop {
        let state = controller.request(serde_json::json!("Screen"));
        if state["durable_bytes"] == 65530 {
            break state;
        }
        assert!(Instant::now() < deadline, "padding not durable: {state}");
        std::thread::sleep(Duration::from_millis(10));
    };
    let prefix = transcript_bytes14(&fixture);
    std::fs::write(fixture.state().join("proof-storage-partial"), "7").unwrap();
    controller.request(serde_json::json!({"Input":b"partial14\r".to_vec()}));
    let deadline = Instant::now() + Duration::from_secs(3);
    let failed = loop {
        let state = controller.request(serde_json::json!("Screen"));
        if state["storage_error"]
            .as_str()
            .is_some_and(|error| error.contains("No space"))
        {
            break state;
        }
        assert!(
            Instant::now() < deadline,
            "missing real partial write error: {state}"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(failed["durable_bytes"], before["durable_bytes"]);
    assert!(
        transcript_bytes14(&fixture).len() > prefix.len(),
        "fault must really write a prefix"
    );
    assert!(failed["capture_pending_bytes"].as_u64().unwrap() > 0);
    std::fs::remove_file(fixture.state().join("proof-storage-partial")).unwrap();
    controller.request(serde_json::json!("RetryStorage"));
    let deadline = Instant::now() + Duration::from_secs(4);
    let recovered = loop {
        let state = controller.request(serde_json::json!("Screen"));
        if state["durable_bytes"] == state["accepted_bytes"] && state["storage_error"].is_null() {
            break state;
        }
        assert!(Instant::now() < deadline, "retry not durable: {state}");
        std::thread::sleep(Duration::from_millis(20));
    };
    let mut expected = prefix;
    expected.extend_from_slice(b"received: partial14\r\nbytes: 7061727469616c3134\r\nfixture> ");
    assert_eq!(transcript_bytes14(&fixture), expected);
    assert_eq!(recovered["capture_pending_bytes"], 0);
    controller.request(serde_json::json!("Shutdown"));
}

#[test]
fn storage_authorization_and_result_stalls_allow_stop_without_late_execution() {
    for boundary in ["authorize", "save"] {
        let fixture = Fixture::new();
        let mut dashboard = fixture.open();
        dashboard.wait("Discovery complete");
        dashboard.send(b"a");
        dashboard.wait("Adopted");
        dashboard.kill();
        let mut controller = Controller::connect(&fixture);
        // Drain adoption before targeting the launch outcome Save.
        while controller.request(serde_json::json!("Screen"))["metadata_pending"] == true {
            std::thread::sleep(Duration::from_millis(10));
        }
        let fault = fixture.state().join(format!("proof-storage-{boundary}"));
        std::fs::write(&fault, "stall").unwrap();
        let launch =
            controller.request_now(serde_json::json!({"Launch":format!("stall-{boundary}-14")}));
        assert_eq!(launch["metadata_pending"], true);
        assert!(launch["launch_result"].is_null());
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let state = controller.request(serde_json::json!("Screen"));
            if state["storage_wait_millis"].as_u64().unwrap_or(0) >= 100 {
                break;
            }
            assert!(Instant::now() < deadline, "writer never stalled: {state}");
            std::thread::sleep(Duration::from_millis(10));
        }
        let start = Instant::now();
        controller.request(serde_json::json!("Stop"));
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let state = controller.request(serde_json::json!("Screen"));
            if state["lifecycle"] == "Stopped" {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "Stop blocked by {boundary}: {state}"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(start.elapsed() < Duration::from_secs(2));
        assert_no_owned_coding(&fixture);
        std::fs::remove_file(fault).unwrap();
        std::thread::sleep(Duration::from_millis(300));
        assert_ne!(
            controller.request(serde_json::json!("Screen"))["lifecycle"],
            "Running"
        );
        assert_no_owned_coding(&fixture);
        controller.request(serde_json::json!("Shutdown"));
    }
}

#[test]
fn storage_saturation_keeps_navigation_retry_stop_and_exact_retained_bytes() {
    let fixture = Fixture::new();
    assert!(
        Command::new("git")
            .arg("-C")
            .arg(&fixture.repository)
            .args([
                "-c",
                "user.name=Proof",
                "-c",
                "user.email=proof@example.invalid",
                "commit",
                "--allow-empty",
                "-m",
                "storage14"
            ])
            .output()
            .unwrap()
            .status
            .success()
    );
    assert!(
        Command::new("git")
            .arg("-C")
            .arg(&fixture.repository)
            .args(["worktree", "add", "-b", "other14"])
            .arg(fixture.temp.path().join("other14"))
            .output()
            .unwrap()
            .status
            .success()
    );
    std::fs::create_dir_all(fixture.state()).unwrap();
    std::fs::set_permissions(
        fixture.state(),
        std::os::unix::fs::PermissionsExt::from_mode(0o700),
    )
    .unwrap();
    std::fs::write(fixture.state().join("proof-storage-ledger"), "enabled").unwrap();
    let mut dashboard = fixture.open();
    dashboard.wait("Discovery complete");
    dashboard.send(b"a");
    dashboard.wait("Adopted");
    dashboard.send(b"l");
    dashboard.wait("fixture>");
    // Allow the initial small output prefix to reach the declared durable boundary.
    std::thread::sleep(Duration::from_millis(400));
    let prefix = transcript_bytes14(&fixture);
    let fault = fixture.state().join("proof-storage-write");
    std::fs::write(&fault, "stall").unwrap();
    dashboard.send(b"flood13\r");
    dashboard.send(&[0x1d]);
    dashboard.wait("Focus: dashboard");
    dashboard.wait("History storage pending");
    let navigation = Instant::now();
    dashboard.send(b"j");
    dashboard.wait("Branch: other14");
    let navigation_us = navigation.elapsed().as_micros();
    let dashboard_pid = dashboard.child.process_id().unwrap();
    let dashboard_rss = Command::new("ps")
        .args([
            "-p",
            &dashboard_pid.to_string(),
            "-o",
            "pid=,ppid=,rss=,stat=",
        ])
        .output()
        .unwrap();
    fixture.assert_one_owner_and_child();
    dashboard.kill();
    let mut controller = Controller::connect(&fixture);
    let deadline = Instant::now() + Duration::from_secs(4);
    let saturated = loop {
        let state = controller.request(serde_json::json!("Screen"));
        if state["producer_wait_millis"].as_u64().unwrap_or(0) >= 500 {
            break state;
        }
        assert!(
            Instant::now() < deadline,
            "capture never saturated: {state}"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    assert!(saturated["capture_pending_bytes"].as_u64().unwrap() <= 270336);
    let owner = owner_pid(&fixture);
    let child = saturated["execution"]["pid"].as_u64().unwrap();
    let mut memory = Vec::new();
    for _ in 0..10 {
        let sample = Command::new("ps")
            .args([
                "-p",
                &format!("{owner},{child}"),
                "-o",
                "pid=,ppid=,rss=,stat=",
            ])
            .output()
            .unwrap();
        let state = controller.request(serde_json::json!("Screen"));
        memory.push(serde_json::json!({"rss_rows":String::from_utf8_lossy(&sample.stdout),"pending":state["capture_pending_bytes"],"accepted":state["accepted_bytes"],"producer_wait_ms":state["producer_wait_millis"],"storage_wait_ms":state["storage_wait_millis"]}));
        assert!(state["capture_pending_bytes"].as_u64().unwrap() <= 270336);
        std::thread::sleep(Duration::from_millis(100));
    }

    let retry = Instant::now();
    controller.request(serde_json::json!("RetryStorage"));
    let retry_us = retry.elapsed().as_micros();
    let stop = Instant::now();
    controller.request(serde_json::json!("Stop"));
    let deadline = Instant::now() + Duration::from_secs(3);
    let stopped = loop {
        let state = controller.request(serde_json::json!("Screen"));
        if state["lifecycle"] == "Stopped" {
            break state;
        }
        assert!(
            Instant::now() < deadline,
            "Stop blocked by saturated storage: {state}"
        );
        std::thread::sleep(Duration::from_millis(10));
    };
    let stop_us = stop.elapsed().as_micros();
    assert!(stopped["capture_pending_bytes"].as_u64().unwrap() > 0);
    assert_eq!(stopped["capture_loss_possible"], true);
    assert_eq!(stopped["stopped"], false);
    assert_no_owned_coding(&fixture);
    let blocked_launch =
        controller.request_now(serde_json::json!({"Launch":"replacement-before-drain-14"}));
    assert!(
        blocked_launch["error"]
            .as_str()
            .unwrap()
            .contains("history is still pending")
    );
    assert_eq!(blocked_launch["identity"], stopped["identity"]);
    assert_no_owned_coding(&fixture);
    // Persistent ENOSPC after a stalled syscall must remain visible and reject quit.
    std::fs::write(&fault, "error").unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let state = controller.request(serde_json::json!("Screen"));
        if !state["storage_error"].is_null() {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(20));
    }
    let failed_quit = controller.request(serde_json::json!("Shutdown"));
    assert_eq!(failed_quit["stopped"], false);
    std::thread::sleep(Duration::from_millis(300));
    let persistent = controller.request(serde_json::json!("Screen"));
    assert!(!persistent["storage_error"].is_null());
    std::fs::remove_file(fault).unwrap();
    let mut repaired_response = Some(controller.request(serde_json::json!("RetryStorage")));
    let deadline = Instant::now() + Duration::from_secs(6);
    let final_state = loop {
        let state = repaired_response
            .take()
            .unwrap_or_else(|| controller.request(serde_json::json!("Screen")));
        if state["accepted_bytes"] == state["durable_bytes"]
            && state["capture_pending_bytes"] == 0
            && state["storage_error"].is_null()
        {
            break state;
        }
        assert!(
            Instant::now() < deadline,
            "repair lost retained bytes: {state}"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    let bytes = transcript_bytes14(&fixture);
    assert_eq!(
        bytes.len() as u64,
        final_state["accepted_bytes"].as_u64().unwrap()
    );
    assert_eq!(&bytes[..prefix.len()], &prefix);
    let expected: Vec<u8> = (0..32768)
        .flat_map(|n| format!("flood13:{n:06}:{}\r\n", "X".repeat(111)).into_bytes())
        .collect();
    assert_eq!(
        &bytes[prefix.len()..],
        &expected[..bytes.len() - prefix.len()]
    );
    let emitted_ledger =
        std::fs::read_to_string(fixture.state().join("proof-emitted.jsonl")).unwrap();
    let emitted: Vec<serde_json::Value> = emitted_ledger
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let emitted_bytes: u64 = emitted
        .iter()
        .map(|entry| entry["bytes"].as_u64().unwrap())
        .sum();
    assert!(
        bytes.len() - prefix.len() <= emitted_bytes as usize + 128,
        "capture can exceed post-write ledger receipts by at most one in-flight 128-byte write"
    );
    let evidence = serde_json::json!({"revision":String::from_utf8_lossy(&Command::new("git").args(["rev-parse","HEAD"]).output().unwrap().stdout).trim(),"source_dirty":!Command::new("git").args(["diff","HEAD","--quiet","--","src","tests"]).status().unwrap().success(),"profile":"debug","fault":"stalled real write, then synthetic persistent ENOSPC before write; no full user disk","capture_slots":64,"block_max":4096,"worker_and_producer_blocks":2,"outer_harness_slots":8,"emitted_records":emitted.len(),"emitted_payload_bytes":emitted_bytes,"emitted_not_captured":emitted_bytes as i64-(bytes.len()-prefix.len()) as i64,"inflight_write_cap":128,"producer_completed_write_max_us":emitted.iter().map(|entry| entry["write_us"].as_u64().unwrap()).max(),"dashboard_rss_rows":String::from_utf8_lossy(&dashboard_rss.stdout),"owner_pid":owner,"child_pid":child,"memory":memory,"navigation_us":navigation_us,"retry_us":retry_us,"stop_us":stop_us,"saturated":saturated,"stopped":stopped,"persistent":persistent,"repaired":final_state,"retained_bytes":bytes.len(),"cap":270336,"workload":"4MiB flood; stalled write; ENOSPC; stop; repair"});
    eprintln!("storage14 evidence: {evidence}");
    if let Some(path) = std::env::var_os("RAT_STORAGE_EVIDENCE") {
        let path = PathBuf::from(path);
        std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(path.with_extension("emitted.jsonl"))
            .unwrap()
            .write_all(emitted_ledger.as_bytes())
            .unwrap();
        std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(path)
            .unwrap()
            .write_all(format!("{evidence}\n").as_bytes())
            .unwrap();
    }
    if final_state["stopped"] != true {
        controller.request(serde_json::json!("Shutdown"));
    }
}

#[test]
fn storage_delayed_writes_and_sync_keep_control_and_defer_saved_boundary() {
    for (boundary, millis) in [("write", 250), ("sync", 1500), ("directory-sync", 0)] {
        let fixture = Fixture::new();
        let mut dashboard = fixture.open();
        dashboard.wait("Discovery complete");
        dashboard.send(b"a");
        dashboard.wait("Adopted");
        // Before first transcript creation so failed directory sync is retried
        // even though the empty file exists on the subsequent attempt.
        let fault = fixture.state().join(format!("proof-storage-{boundary}"));
        std::fs::write(
            &fault,
            if millis == 0 {
                "error".into()
            } else {
                millis.to_string()
            },
        )
        .unwrap();
        dashboard.send(b"l");
        dashboard.wait("fixture>");
        dashboard.kill();
        let mut controller = Controller::connect(&fixture);
        let started = Instant::now();
        let state = controller.request(serde_json::json!("Screen"));
        assert!(started.elapsed() < Duration::from_secs(1));
        // Delay is on the real writer/sync boundary; accepted output is not saved.
        if boundary != "write" {
            assert_eq!(state["durable_bytes"], 0);
        }
        if millis == 0 {
            let deadline = Instant::now() + Duration::from_secs(3);
            loop {
                let state = controller.request(serde_json::json!("Screen"));
                if state["storage_error"]
                    .as_str()
                    .is_some_and(|e| e.contains("No space"))
                {
                    break;
                }
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        std::fs::remove_file(fault).unwrap();
        controller.request(serde_json::json!("RetryStorage"));
        let deadline = Instant::now() + Duration::from_secs(4);
        loop {
            let state = controller.request(serde_json::json!("Screen"));
            if state["durable_bytes"] == state["accepted_bytes"]
                && state["durable_bytes"].as_u64().unwrap() > 0
                && state["storage_error"].is_null()
            {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "{boundary} retry never durable: {state}"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        controller.request(serde_json::json!("Shutdown"));
    }
}

#[test]
fn storage_abrupt_loss_preserves_durable_prefix_and_excludes_unsynced_tail() {
    let fixture = Fixture::new();
    let mut dashboard = fixture.open();
    dashboard.wait("Discovery complete");
    dashboard.send(b"a");
    dashboard.wait("Adopted");
    dashboard.send(b"l");
    dashboard.wait("fixture>");
    dashboard.kill();
    let mut controller = Controller::connect(&fixture);
    let deadline = Instant::now() + Duration::from_secs(3);
    let baseline = loop {
        let state = controller.request(serde_json::json!("Screen"));
        if state["durable_bytes"] == state["accepted_bytes"] {
            break state;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    };
    let prefix = transcript_bytes14(&fixture);
    std::fs::write(fixture.state().join("proof-storage-sync"), "stall").unwrap();
    controller.request(serde_json::json!({"Input":b"unsynced-tail14\r".to_vec()}));
    let deadline = Instant::now() + Duration::from_secs(3);
    let unflushed = loop {
        let state = controller.request(serde_json::json!("Screen"));
        if state["written_bytes"].as_u64().unwrap() > baseline["durable_bytes"].as_u64().unwrap()
            && state["storage_wait_millis"].as_u64().unwrap() >= 100
        {
            break state;
        }
        assert!(
            Instant::now() < deadline,
            "tail never reached real unsynced file: {state}"
        );
        std::thread::sleep(Duration::from_millis(10));
    };
    let child = unflushed["execution"]["pid"].as_i64().unwrap();
    let owner = owner_pid(&fixture);
    assert!(
        Command::new("kill")
            .args(["-KILL", &owner.to_string()])
            .status()
            .unwrap()
            .success()
    );
    // This is cleanup of a directly observed fixture child, not reconciliation.
    let _ = Command::new("kill")
        .args(["-KILL", &child.to_string()])
        .status();
    drop(controller);
    std::fs::remove_file(fixture.state().join("proof-storage-sync")).unwrap();
    let mut restored = fixture.open();
    restored.wait("Process: Interrupted");
    restored.send(b"h");
    restored.wait("Scripted PTY");
    restored.kill();
    let mut controller = Controller::connect(&fixture);
    assert_eq!(controller.greeting["identity"], baseline["identity"]);
    assert_eq!(
        controller.greeting["durable_bytes"],
        baseline["durable_bytes"]
    );
    assert_no_owned_coding(&fixture);
    let deadline = Instant::now() + Duration::from_secs(3);
    let page = loop {
        let state = controller.request(serde_json::json!({"History":{"index":0,"offset":0}}));
        if state["history"]["text"].is_string() {
            break state["history"].clone();
        }
        assert!(Instant::now() < deadline);
    };
    assert!(!page["text"].as_str().unwrap().contains("unsynced-tail14"));
    let raw = transcript_bytes14(&fixture);
    assert_eq!(&raw[..prefix.len()], &prefix);
    assert!(
        raw.len() > prefix.len(),
        "preserve the uncertain raw tail without acknowledging it"
    );
    eprintln!(
        "storage14 abrupt: durable={} accepted={} written={} restored_prefix={} surviving_unacknowledged_file_bytes={}",
        baseline["durable_bytes"],
        unflushed["accepted_bytes"],
        unflushed["written_bytes"],
        controller.greeting["durable_bytes"],
        raw.len() - prefix.len()
    );
}

#[test]
fn launch_receipt_wait_remains_provisional_between_authorization_and_result_save() {
    let fixture = Fixture::new();
    let mut dashboard = fixture.open();
    dashboard.wait("Discovery complete");
    dashboard.send(b"a");
    dashboard.wait("Adopted");
    dashboard.kill();
    let mut controller = Controller::connect(&fixture);
    let marker = fixture.state().join("proof-launch-receipt");
    std::fs::write(&marker, "hold receipt on its worker").unwrap();
    let provisional = controller.request_now(serde_json::json!({"Launch":"receipt-gap-14"}));
    assert_eq!(provisional["metadata_pending"], true);
    let deadline = Instant::now() + Duration::from_secs(3);
    let receipt_wait = loop {
        let state = controller.request_now(serde_json::json!("Screen"));
        if state["screen"].as_str().unwrap().contains("fixture>") {
            break state;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    };
    assert_eq!(
        receipt_wait["metadata_pending"], true,
        "exec receipt/result is still unconfirmed"
    );
    assert!(receipt_wait["launch_result"].is_null());
    controller.request_now(serde_json::json!("Stop"));
    std::fs::remove_file(marker).unwrap();
    let mut state = controller.request_now(serde_json::json!("Shutdown"));
    let deadline = Instant::now() + Duration::from_secs(4);
    while state["stopped"] != true {
        assert!(
            Instant::now() < deadline,
            "cancelled receipt did not finish: {state}"
        );
        std::thread::sleep(Duration::from_millis(10));
        state = controller.request_now(serde_json::json!("Screen"));
    }
}

#[test]
fn explicit_cleanup_warns_previews_cancels_and_deletes_only_selected_history() {
    let fixture = Fixture::new();
    let sentinel = fixture.repository.join("working-change");
    std::fs::write(&sentinel, b"keep checkout").unwrap();
    let mut dashboard = fixture.open();
    dashboard.wait("Discovery complete");
    std::fs::write(fixture.state().join("proof-storage-available"), b"0").unwrap();
    dashboard.wait("Low storage");
    dashboard.send(b"a");
    dashboard.wait("Adopted");
    dashboard.send(b"l");
    dashboard.wait("fixture>");
    dashboard.send(b"\x1ds");
    dashboard.wait("Process: Stopped");
    dashboard.send(b"g");
    dashboard.wait("History cleanup");
    dashboard.wait("allocated bytes");
    let retained = transcript_bytes14(&fixture);
    assert!(!retained.is_empty());
    dashboard.send(b"\r");
    dashboard.wait("Confirm selected history deletion");
    dashboard.send(b"\x1b");
    dashboard.wait("History cleanup");
    assert_eq!(
        transcript_bytes14(&fixture),
        retained,
        "cancellation must preserve all transcript bytes"
    );
    dashboard.send(b"\r");
    dashboard.wait("Confirm selected history deletion");
    dashboard.send(b"D");
    dashboard.wait("History deletion completed");
    assert_eq!(std::fs::read(&sentinel).unwrap(), b"keep checkout");
    dashboard.wait("Low storage");
    dashboard.send(b"\x1b");
    dashboard.wait("h history");
    dashboard.send(b"q");
    let deadline = Instant::now() + Duration::from_secs(8);
    while dashboard.child.try_wait().unwrap().is_none() {
        assert!(
            Instant::now() < deadline,
            "cleanup dashboard failed to quit"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn paced_producer_keeps_native_input_live_before_completion() {
    let fixture = Fixture::new();
    let mut dashboard = fixture.open();
    dashboard.wait("Discovery complete");
    dashboard.send(b"a");
    dashboard.wait("Adopted");
    dashboard.send(b"l");
    dashboard.wait("fixture>");
    dashboard.send(b"pace17 1 3000 65536\r");
    dashboard.wait("pace17 run=1");
    dashboard.send(b"ack17 concurrent\r");
    let screen = dashboard.wait("ACK17 concurrent");
    assert!(
        !screen.contains("pace17-complete"),
        "ACK must be visible while output producer runs"
    );
    dashboard.wait("pace17-complete");
    dashboard.send(&[0x1d]);
    dashboard.wait("Focus: dashboard");
    dashboard.send(b"q");
    let deadline = Instant::now() + Duration::from_secs(8);
    while dashboard.child.try_wait().unwrap().is_none() {
        while dashboard.output.try_recv().is_ok() {}
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
}

// Continuous terminal consumption for measurements. Exactly one bounded latest
// parsed screen is retained; the existing eight-block outer queue remains finite.
struct LiveDashboard {
    dashboard: Dashboard,
    latest: std::sync::Arc<std::sync::Mutex<(String, u64)>>,
    collector: Option<std::thread::JoinHandle<()>>,
    parse_cost: std::sync::Arc<std::sync::Mutex<(u64, u64, u64)>>,
}
impl LiveDashboard {
    fn new(mut dashboard: Dashboard) -> Self {
        let (_, empty) = mpsc::sync_channel(1);
        let output = std::mem::replace(&mut dashboard.output, empty);
        let mut parser = std::mem::replace(
            &mut dashboard.screen,
            vt100::Parser::new(40.try_into().unwrap(), 160.try_into().unwrap(), 0),
        );
        let latest = std::sync::Arc::new(std::sync::Mutex::new((
            parser.screen().contents(),
            measurement_clock(),
        )));
        let screen = latest.clone();
        let parse_cost = std::sync::Arc::new(std::sync::Mutex::new((0u64, 0u64, 0u64)));
        let costs = parse_cost.clone();
        let collector = std::thread::spawn(move || {
            while let Ok(bytes) = output.recv() {
                let since = Instant::now();
                parser.process(&bytes);
                *screen.lock().unwrap() = (parser.screen().contents(), measurement_clock());
                let elapsed = since.elapsed().as_micros() as u64;
                let mut cost = costs.lock().unwrap();
                cost.0 += 1;
                cost.1 += elapsed;
                cost.2 = cost.2.max(elapsed);
            }
        });
        Self {
            dashboard,
            latest,
            collector: Some(collector),
            parse_cost,
        }
    }
    fn send(&mut self, bytes: &[u8]) {
        self.dashboard.send(bytes);
    }
    fn observe(&self, needle: &str, timeout: Duration) -> Option<(String, u64)> {
        let until = Instant::now() + timeout;
        loop {
            let screen = self.latest.lock().unwrap().clone();
            if screen.0.contains(needle) {
                return Some(screen);
            }
            if Instant::now() >= until {
                return None;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    fn wait(&self, needle: &str) -> (String, u64) {
        self.wait_for(needle, Duration::from_secs(8))
    }
    fn wait_for(&self, needle: &str, timeout: Duration) -> (String, u64) {
        self.observe(needle, timeout).unwrap_or_else(|| {
            let screen = self.latest.lock().unwrap().0.clone();
            panic!("missing {needle}: {screen}")
        })
    }
}
impl Drop for LiveDashboard {
    fn drop(&mut self) {
        self.dashboard.kill();
        if let Some(collector) = self.collector.take() {
            collector.join().unwrap();
        }
    }
}

struct FailedMeasurementEvidence {
    state: PathBuf,
    evidence: PathBuf,
}
impl Drop for FailedMeasurementEvidence {
    fn drop(&mut self) {
        if self.evidence.join("history.sqlite3").exists() {
            return;
        }
        let partial = self.evidence.join("failure-snapshot");
        if std::fs::create_dir(&partial).is_err() {
            return;
        }
        let _=std::fs::write(partial.join("NOTE.txt"),"Failure-time best-effort snapshot; owner may still be writing. JSONL can have an incomplete final line. Not a quiescent durability snapshot.
");
        if let Ok(entries) = std::fs::read_dir(&self.state) {
            for entry in entries.flatten() {
                if entry.file_type().is_ok_and(|kind| kind.is_file()) {
                    let _ = std::fs::copy(entry.path(), partial.join(entry.file_name()));
                }
            }
        }
    }
}

fn measurement_shutdown(control: &mut Controller) -> serde_json::Value {
    let mut state = control.request(serde_json::json!("Shutdown"));
    let until = Instant::now() + Duration::from_secs(10);
    while state["stopped"] != true {
        assert!(Instant::now() < until, "shutdown unconfirmed: {state}");
        std::thread::sleep(Duration::from_millis(10));
        state = control.request(serde_json::json!("Screen"));
    }
    assert_eq!(state["capture_pending_bytes"], 0);
    state
}
fn measurement_wait_endpoint_exit(fixture: &Fixture) {
    let until = Instant::now() + Duration::from_secs(3);
    while fixture.state().join("supervisor.sock").exists() {
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
#[ignore = "serial multi-minute macOS measurement; RAT_MEASURE17_DIR is required"]
fn paced_session_measurement17() {
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    };
    let fixture = Fixture::new();
    let preflight = std::env::var_os("RAT_MEASURE17_QUICK").is_some();
    let phase_ms = if preflight { 4000 } else { 60000 };
    let attached_ms = if preflight { 4000 } else { 150000 };
    let attach_samples = if preflight { 5 } else { 100 };
    let navigation_samples = if preflight { 10 } else { 200 };
    let evidence = PathBuf::from(
        std::env::var_os("RAT_MEASURE17_DIR").expect("set unique evidence directory"),
    );
    std::fs::create_dir(&evidence).unwrap();
    let _failure_evidence = FailedMeasurementEvidence {
        state: fixture.state(),
        evidence: evidence.clone(),
    };
    let raw = Arc::new(Mutex::new(
        std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(evidence.join("observations.jsonl"))
            .unwrap(),
    ));
    let log = |value: serde_json::Value| writeln!(raw.lock().unwrap(), "{value}").unwrap();
    // These are run parameters, not product capacity promises.
    log(
        serde_json::json!({"kind":"manifest","revision":String::from_utf8(Command::new("git").args(["rev-parse","HEAD"]).output().unwrap().stdout).unwrap().trim(),"dirty":!Command::new("git").args(["diff","HEAD","--quiet"]).status().unwrap().success(),"profile":"debug","outer_terminal":"portable-pty + vt100 0.18.1; no graphical terminal/pixel latency","outer_geometry":[40,160],"healthy_duration_ms":phase_ms,"attached_duration_ms":attached_ms,"preflight_only":preflight,"healthy_rate_Bps":65536,"record_bytes":1024,"style_mix":"each record: cursor position/erase, green SGR, lambda+CJK UTF8, padded X; raw termios","navigation_samples":navigation_samples,"attach_samples":attach_samples,"rss_period_ms":1000,"lag_ms":phase_ms,"slow_write_ms":50,"slow_fault_navigation_samples":if preflight {4} else {20},"full_fault_dwell_ms":if preflight {5000} else {30000},"session_geometry_initial":[32,104],"slow_subscriber_geometry":[80,200],"fault":"debug write ENOSPC marker, not real volume exhaustion","caps":{"capture_bytes":270336,"stop_tail_bytes":1048576,"stop_tail_blocks":256,"conservative_stop_pending_cap":1318912,"storage_commands":16,"storage_events":16,"input_commands":32,"client_requests":32,"client_latest":1,"outer_blocks":8,"outer_block_bytes":4096,"frame_bytes":4194304,"commit_records":1024,"chunk_bytes":65536,"screen_max":[80,200],"scrollback":0},"instrumentation":"continuous latest screen parser/mutex; 1ms observation poll; fixture per-record and input JSONL; bounded commit ring; ps 1s, actual sampler durations recorded"}),
    );
    let binary_hash = Command::new("shasum")
        .args(["-a", "256", env!("CARGO_BIN_EXE_agent-terminal")])
        .output()
        .unwrap();
    log(serde_json::json!({"kind":"binary","sha256":String::from_utf8_lossy(&binary_hash.stdout)}));
    for (command, args) in [
        ("sw_vers", vec![]),
        ("uname", vec!["-mrs"]),
        (
            "sysctl",
            vec!["-n", "hw.model", "hw.memsize", "hw.logicalcpu"],
        ),
        ("rustc", vec!["-Vv"]),
        ("cargo", vec!["-V"]),
        ("df", vec!["-k", "/private/tmp"]),
    ] {
        let output = Command::new(command).args(args).output().unwrap();
        log(
            serde_json::json!({"kind":"environment","command":command,"stdout":String::from_utf8_lossy(&output.stdout),"stderr":String::from_utf8_lossy(&output.stderr)}),
        );
    }
    assert!(
        Command::new("git")
            .arg("-C")
            .arg(&fixture.repository)
            .args([
                "-c",
                "user.name=Proof",
                "-c",
                "user.email=proof@example.invalid",
                "commit",
                "--allow-empty",
                "-m",
                "measurement17"
            ])
            .output()
            .unwrap()
            .status
            .success()
    );
    assert!(
        Command::new("git")
            .arg("-C")
            .arg(&fixture.repository)
            .args(["worktree", "add", "-b", "other17"])
            .arg(fixture.temp.path().join("other17"))
            .output()
            .unwrap()
            .status
            .success()
    );
    std::os::unix::fs::DirBuilderExt::mode(&mut std::fs::DirBuilder::new(), 0o700)
        .create(fixture.state())
        .unwrap();
    std::fs::write(fixture.state().join("proof-storage-ledger"), "enabled").unwrap();
    let mut dashboard = LiveDashboard::new(fixture.open());
    dashboard.wait("Discovery complete");
    dashboard.send(b"a");
    dashboard.wait("Adopted");
    dashboard.send(b"l");
    let launch = dashboard.wait("fixture>").0;
    let child: u32 = launch
        .split("pid=")
        .nth(1)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .parse()
        .unwrap();
    fixture.assert_one_owner_and_child();
    let mut process_list = String::from_utf8(
        Command::new("ps")
            .args(["-Ao", "pid,ppid,command"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    let owner: u32 = process_list
        .lines()
        .find(|line| {
            line.contains("--supervisor") && line.contains(fixture.state().to_str().unwrap())
        })
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .parse()
        .unwrap();
    process_list.clear();
    let pids = Arc::new(Mutex::new(vec![
        std::process::id(),
        owner,
        child,
        dashboard.dashboard.child.process_id().unwrap(),
    ]));
    log(
        serde_json::json!({"kind":"identities","harness":std::process::id(),"supervisor":owner,"child":child,"dashboard":dashboard.dashboard.child.process_id(),"screen":launch}),
    );
    let done = Arc::new(AtomicBool::new(false));
    let sampler = {
        let raw = raw.clone();
        let done = done.clone();
        let pids = pids.clone();
        let state_directory = fixture.state();
        std::thread::spawn(move || {
            while !done.load(Ordering::Relaxed) {
                let list = pids
                    .lock()
                    .unwrap()
                    .iter()
                    .map(u32::to_string)
                    .collect::<Vec<_>>()
                    .join(",");
                let before = measurement_clock();
                let since = Instant::now();
                let output = Command::new("ps")
                    .args(["-p", &list, "-o", "pid=,ppid=,rss=,etime=,command="])
                    .output()
                    .unwrap();
                writeln!(raw.lock().unwrap(),"{}",serde_json::json!({"kind":"rss","sample_ns":before,"sampler_us":since.elapsed().as_micros(),"ps":String::from_utf8_lossy(&output.stdout)})).unwrap();
                let mut transcript_bytes = 0;
                let mut metadata_bytes = 0;
                let mut chunks = 0;
                let mut allocated = 0;
                for entry in std::fs::read_dir(&state_directory).unwrap() {
                    let entry = entry.unwrap();
                    let metadata = entry.metadata().unwrap();
                    let size = metadata.len();
                    if entry
                        .path()
                        .extension()
                        .is_some_and(|ext| ext == "transcript")
                    {
                        transcript_bytes += size;
                        allocated += std::os::unix::fs::MetadataExt::blocks(&metadata) * 512;
                        chunks += 1;
                    } else if entry
                        .file_name()
                        .to_string_lossy()
                        .starts_with("history.sqlite3")
                    {
                        metadata_bytes += size;
                    }
                }
                writeln!(raw.lock().unwrap(),"{}",serde_json::json!({"kind":"disk","sample_ns":measurement_clock(),"transcript_bytes":transcript_bytes,"metadata_bytes":metadata_bytes,"transcript_allocated_bytes":allocated,"chunks":chunks})).unwrap();
                std::thread::sleep(Duration::from_secs(1));
            }
        })
    };
    let mut failures = Vec::new();
    // Baseline quantifies full harness+application scheduling without output.
    dashboard.send(&[0x1d]);
    dashboard.wait("Focus: dashboard");
    for sample in 0..20 {
        let (key, expected) = if sample % 2 == 0 {
            (b"j", "Branch: other17")
        } else {
            (b"k", "Branch: main")
        };
        let before = measurement_clock();
        dashboard.send(key);
        let visible = dashboard.wait(expected).1;
        log(
            serde_json::json!({"kind":"navigation","mode":"baseline","sample":sample,"injected_ns":before,"visible_ns":visible,"us":visible.saturating_sub(before)/1000}),
        );
    }
    dashboard.send(b"\r");
    dashboard.wait("Focus: session");
    let mut last_commit = 0u64;
    let collect = |state: &serde_json::Value, last: &mut u64| {
        for commit in state["durable_commits"].as_array().unwrap() {
            let seq = commit["sequence"].as_u64().unwrap();
            if seq > *last {
                assert_eq!(
                    seq,
                    *last + 1,
                    "telemetry gap invalidates full distribution"
                );
                log(serde_json::json!({"kind":"commit","commit":commit}));
                *last = seq;
            }
        }
    };
    dashboard.send(format!("pace17 1 {attached_ms} 65536\r").as_bytes());
    dashboard.wait("pace17 run=1");
    for sample in 0..attach_samples {
        dashboard.send(&[0x1d]);
        dashboard.wait("Focus: dashboard");
        let before = measurement_clock();
        dashboard.send(b"\r");
        let focus = dashboard.wait("Focus: session").1;
        let nonce = format!("a{sample:03}");
        dashboard.send(format!("ack17 {nonce}\r").as_bytes());
        let visible = dashboard.observe(&format!("ACK17 {nonce} "), Duration::from_secs(2));
        let end = visible.as_ref().map(|s| s.1);
        let us = end.map(|ns| ns.saturating_sub(before) / 1000);
        if us.is_none_or(|value| value >= 1_000_000) {
            failures.push(format!("attach {sample}: {us:?}"));
        }
        log(
            serde_json::json!({"kind":"attach","mode":"attached","run":1,"sample":sample,"nonce":nonce,"injected_ns":before,"focus_ns":focus,"visible_ns":end,"us":us,"screen":visible.map(|s|s.0)}),
        );
        std::thread::sleep(Duration::from_millis(150));
    }
    dashboard.wait_for(
        "pace17-complete run=1",
        Duration::from_millis(attached_ms + 8000),
    );
    log(
        serde_json::json!({"kind":"parser-cost","mode":"attached","batches_totalus_maxus":*dashboard.parse_cost.lock().unwrap()}),
    );
    drop(dashboard);
    pids.lock().unwrap().truncate(3);
    let control = Controller::connect(&fixture);
    let identity = control.greeting["identity"].clone();
    assert_eq!(
        control.greeting["execution"]["pid"].as_u64(),
        Some(u64::from(child))
    );
    collect(&control.greeting, &mut last_commit);
    log(serde_json::json!({"kind":"phase","mode":"attached-complete","state":control.greeting}));
    fixture.assert_one_owner_and_child();
    drop(control);
    dashboard = LiveDashboard::new(fixture.reopen_after_release("Process: Running"));
    dashboard.wait("Focus: session");
    pids.lock()
        .unwrap()
        .push(dashboard.dashboard.child.process_id().unwrap());
    dashboard.send(format!("pace17 2 {phase_ms} 65536\r").as_bytes());
    dashboard.wait("pace17 run=2");
    dashboard.send(&[0x1d]);
    dashboard.wait("Focus: dashboard");
    for sample in 0..navigation_samples {
        let (key, expected) = if sample % 2 == 0 {
            (b"j", "Branch: other17")
        } else {
            (b"k", "Branch: main")
        };
        let before = measurement_clock();
        dashboard.send(key);
        let visible = dashboard.wait(expected).1;
        let us = visible.saturating_sub(before) / 1000;
        if us > 100_000 {
            failures.push(format!("navigation {sample}: {us}"));
        }
        log(
            serde_json::json!({"kind":"navigation","mode":"dashboard-focused","run":2,"sample":sample,"injected_ns":before,"visible_ns":visible,"us":us}),
        );
        if sample % 5 == 0 {
            dashboard.send(b"r");
        }
        std::thread::sleep(Duration::from_millis(150));
    }
    dashboard.send(b"\r");
    dashboard.wait_for(
        "pace17-complete run=2",
        Duration::from_millis(phase_ms + 8000),
    );
    log(
        serde_json::json!({"kind":"parser-cost","mode":"healthy","batches_totalus_maxus":*dashboard.parse_cost.lock().unwrap()}),
    );
    drop(dashboard);
    pids.lock().unwrap().truncate(3);
    let mut control = Controller::connect(&fixture);
    assert_eq!(control.greeting["identity"], identity);
    assert_eq!(
        control.greeting["execution"]["pid"].as_u64(),
        Some(u64::from(child))
    );
    collect(&control.greeting, &mut last_commit);
    log(serde_json::json!({"kind":"phase","mode":"healthy-complete","state":control.greeting}));
    for (run, mode, rate, delay) in [
        (3, "fully-detached", 65536, None),
        (4, "slow-subscriber", 65536, None),
        (5, "slow-storage", 262144, Some("50")),
    ] {
        if let Some(delay) = delay {
            std::fs::write(fixture.state().join("proof-storage-write"), delay).unwrap();
        }
        if run == 4 {
            control.request(serde_json::json!({"Resize":{"rows":80,"cols":200}}));
            control.request(serde_json::json!({"Input":b"mosaic\r".to_vec()}));
            let until = Instant::now() + Duration::from_secs(8);
            while !control.request(serde_json::json!("Screen"))["screen"]
                .as_str()
                .unwrap()
                .contains("max geometry complete")
            {
                assert!(Instant::now() < until);
            }
            control.send(
                serde_json::json!({"Input":format!("pace17 {run} {phase_ms} {rate}\r").as_bytes()}),
            );
            // Real large styled response is now left unread at the socket.
            std::thread::sleep(Duration::from_millis(phase_ms));
            drop(control);
            control = Controller::connect(&fixture);
            assert_eq!(control.greeting["identity"], identity);
            assert!(
                control.greeting["error"]
                    .as_str()
                    .unwrap_or("")
                    .contains("Slow dashboard"),
                "actual transport must saturate: {}",
                control.greeting
            );
            collect(&control.greeting, &mut last_commit);
        } else if run == 5 {
            let started = Instant::now();
            control.request(
                serde_json::json!({"Input":format!("pace17 {run} {phase_ms} {rate}\r").as_bytes()}),
            );
            drop(control);
            let mut slow = LiveDashboard::new(fixture.reopen_after_release("Process: Running"));
            slow.send(&[0x1d]);
            slow.wait("Focus: dashboard");
            pids.lock()
                .unwrap()
                .push(slow.dashboard.child.process_id().unwrap());
            std::thread::sleep(Duration::from_millis(if preflight { 500 } else { 10000 }));
            for sample in 0..if preflight { 4 } else { 20 } {
                let (key, expected) = if sample % 2 == 0 {
                    (b"j", "Branch: other17")
                } else {
                    (b"k", "Branch: main")
                };
                let before = measurement_clock();
                slow.send(key);
                let visible = slow.wait(expected).1;
                log(
                    serde_json::json!({"kind":"navigation","mode":"slow-storage","run":5,"sample":sample,"injected_ns":before,"visible_ns":visible,"us":visible.saturating_sub(before)/1000}),
                );
                std::thread::sleep(Duration::from_millis(150));
            }
            let before = measurement_clock();
            slow.send(b"\r");
            let focus = slow.wait("Focus: session").1;
            slow.send(b"ack17 slow\r");
            let visible = slow.observe("ACK17 slow ", Duration::from_secs(2));
            log(
                serde_json::json!({"kind":"attach","mode":"slow-storage","run":5,"nonce":"slow","injected_ns":before,"focus_ns":focus,"visible_ns":visible.as_ref().map(|s|s.1),"us":visible.as_ref().map(|s|s.1.saturating_sub(before)/1000),"censor_ns":measurement_clock()}),
            );
            slow.send(&[0x1d]);
            slow.wait("Focus: dashboard");
            if let Some(left) = Duration::from_millis(phase_ms).checked_sub(started.elapsed()) {
                std::thread::sleep(left);
            }
            drop(slow);
            pids.lock().unwrap().truncate(3);
            control = Controller::connect(&fixture);
        } else {
            control.request(
                serde_json::json!({"Input":format!("pace17 {run} {phase_ms} {rate}\r").as_bytes()}),
            );
            // There is genuinely no controller/dashboard during detached output.
            drop(control);
            std::thread::sleep(Duration::from_millis(phase_ms));
            control = Controller::connect(&fixture);
        }
        assert_eq!(control.greeting["identity"], identity);
        assert_eq!(
            control.greeting["execution"]["pid"].as_u64(),
            Some(u64::from(child))
        );
        fixture.assert_one_owner_and_child();
        if delay.is_some() {
            std::fs::remove_file(fixture.state().join("proof-storage-write")).unwrap();
            control.request(serde_json::json!("RetryStorage"));
        }
        let deadline = Instant::now() + Duration::from_secs(15);
        let state = loop {
            let state = control.request(serde_json::json!("Screen"));
            collect(&state, &mut last_commit);
            if state["screen"]
                .as_str()
                .unwrap()
                .contains(&format!("pace17-complete run={run}"))
                && state["capture_pending_bytes"] == 0
            {
                break state;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(25));
        };
        assert!(state["capture_high_water"].as_u64().unwrap() <= 270336);
        assert!(state["subscriber_frame_high_water"].as_u64().unwrap() <= 4194304);
        if run == 5 {
            assert!(
                state["capture_backpressure_count"].as_u64().unwrap() > 0,
                "slow storage must actually saturate finite capture"
            );
        }
        log(serde_json::json!({"kind":"phase","mode":mode,"state":state}));
    }
    // ENOSPC fills the finite queue. Receipt can stay live while visible output
    // is right-censored behind backpressure. Stop is measured independently.
    std::fs::write(fixture.state().join("proof-storage-write"), "error").unwrap();
    log(serde_json::json!({"kind":"fault-window-start","run":6,"started_ns":measurement_clock()}));
    control.request(serde_json::json!({"Input":b"pace17 6 30000 262144\r".to_vec()}));
    drop(control);
    let mut dashboard = LiveDashboard::new(fixture.reopen_after_release("Process: Running"));
    pids.lock()
        .unwrap()
        .push(dashboard.dashboard.child.process_id().unwrap());
    dashboard.send(&[0x1d]);
    dashboard.wait("Focus: dashboard");
    std::thread::sleep(Duration::from_secs(if preflight { 5 } else { 30 }));
    for sample in 0..20 {
        let (key, expected) = if sample % 2 == 0 {
            (b"j", "Branch: other17")
        } else {
            (b"k", "Branch: main")
        };
        let before = measurement_clock();
        dashboard.send(key);
        let end = dashboard.wait(expected).1;
        log(
            serde_json::json!({"kind":"navigation","mode":"full-storage","sample":sample,"injected_ns":before,"visible_ns":end,"us":end.saturating_sub(before)/1000}),
        );
    }
    let before = measurement_clock();
    dashboard.send(b"\r");
    let fault_focus = dashboard.wait("Focus: session").1;
    dashboard.send(b"ack17 fault\r");
    let visible = dashboard.observe("ACK17 fault ", Duration::from_secs(2));
    log(
        serde_json::json!({"kind":"attach","mode":"full-storage","nonce":"fault","injected_ns":before,"focus_ns":fault_focus,"visible_ns":visible.as_ref().map(|s|s.1),"censor_ns":measurement_clock(),"screen":dashboard.latest.lock().unwrap().0}),
    );
    dashboard.send(&[0x1d]);
    dashboard.wait("Focus: dashboard");
    let before = measurement_clock();
    dashboard.send(b"s");
    let stop_confirmed = dashboard
        .observe("Process: Stopped", Duration::from_secs(3))
        .is_some();
    if !stop_confirmed {
        failures.push("full-storage Stop unconfirmed before repair".to_owned());
        drop(dashboard);
        let mut direct = Controller::connect(&fixture);
        let actual = direct.request(serde_json::json!("Screen"));
        let ps = Command::new("ps")
            .args(["-Ao", "pid,ppid,pgid,stat,command"])
            .output()
            .unwrap();
        let processes = String::from_utf8_lossy(&ps.stdout)
            .lines()
            .filter(|line| {
                line.contains(env!("CARGO_BIN_EXE_agent-terminal"))
                    || line
                        .split_whitespace()
                        .next()
                        .and_then(|pid| pid.parse::<u32>().ok())
                        == Some(child)
                    || line
                        .split_whitespace()
                        .nth(2)
                        .and_then(|pid| pid.parse::<u32>().ok())
                        == Some(child)
            })
            .collect::<Vec<_>>()
            .join("\n");
        log(serde_json::json!({"kind":"stop-failed-detail","state":actual,"processes":processes}));
        drop(direct);
        dashboard = LiveDashboard::new(fixture.reopen_after_release("Focus: dashboard"));
    }
    log(
        serde_json::json!({"kind":"stop","mode":"full-storage","injected_ns":before,"visible_ns":measurement_clock(),"screen":dashboard.latest.lock().unwrap().0}),
    );
    std::fs::remove_file(fixture.state().join("proof-storage-write")).unwrap();
    let before = measurement_clock();
    dashboard.send(b"y");
    if !stop_confirmed {
        dashboard.send(b"s");
        dashboard.wait("Process: Stopped");
    }
    std::thread::sleep(Duration::from_secs(2));
    drop(dashboard);
    pids.lock().unwrap().truncate(3);
    let mut control = Controller::connect(&fixture);
    let deadline = Instant::now() + Duration::from_secs(10);
    let final_state = loop {
        let state = control.request(serde_json::json!("Screen"));
        collect(&state, &mut last_commit);
        if state["capture_pending_bytes"] == 0 && state["durable_bytes"] == state["accepted_bytes"]
        {
            break state;
        }
        assert!(Instant::now() < deadline, "{state}");
        std::thread::sleep(Duration::from_millis(25));
    };
    assert!(final_state["capture_high_water"].as_u64().unwrap() <= 1318912);
    log(
        serde_json::json!({"kind":"retry","injected_ns":before,"observed_ns":measurement_clock(),"state":final_state}),
    );
    for offset in [
        0,
        65500,
        final_state["durable_bytes"]
            .as_u64()
            .unwrap()
            .saturating_sub(1024),
    ] {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let state = control.request(serde_json::json!({"History":{"index":0,"offset":offset}}));
            if state["history"]["offset"] == offset {
                assert!(!state["history"]["text"].as_str().unwrap().is_empty());
                log(serde_json::json!({"kind":"public-history","history":state["history"]}));
                break;
            }
            assert!(Instant::now() < deadline);
        }
    }
    let shutdown_ack = measurement_shutdown(&mut control);
    log(serde_json::json!({"kind":"clean-shutdown-ack","state":shutdown_ack}));
    drop(control);
    measurement_wait_endpoint_exit(&fixture);
    done.store(true, Ordering::Relaxed);
    sampler.join().unwrap();
    for entry in std::fs::read_dir(fixture.state()).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_file() {
            std::fs::copy(entry.path(), evidence.join(entry.file_name())).unwrap();
        }
    }
    // A new owner/runtime supplies a current-revision paced abrupt-failure run.
    let mut crash_dashboard = LiveDashboard::new(fixture.reopen_after_release("Process: Stopped"));
    crash_dashboard.send(b"l");
    let screen = crash_dashboard.wait("fixture>").0;
    let crash_child: i32 = screen
        .split("pid=")
        .nth(1)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .parse()
        .unwrap();
    crash_dashboard.send(b"pace17 7 10000 65536\r");
    crash_dashboard.wait("pace17 run=7");
    crash_dashboard.send(b"ack17 crash\r");
    crash_dashboard.wait("ACK17 crash ");
    std::thread::sleep(Duration::from_secs(2));
    drop(crash_dashboard);
    let mut crash_control = Controller::connect(&fixture);
    let counter_request_ns = measurement_clock();
    let acknowledged = crash_control.request(serde_json::json!("Screen"));
    let counter_received_ns = measurement_clock();
    let processes = String::from_utf8(
        Command::new("ps")
            .args(["-Ao", "pid,ppid,command"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    let crash_owner: i32 = processes
        .lines()
        .find(|line| {
            line.contains("--supervisor") && line.contains(fixture.state().to_str().unwrap())
        })
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .parse()
        .unwrap();
    fixture.assert_one_owner_and_child();
    let kill_injected_ns = measurement_clock();
    // SAFETY: both PIDs were observed in this isolated runtime, never the default owner.
    unsafe {
        assert_eq!(libc::kill(crash_owner, libc::SIGKILL), 0);
        libc::kill(crash_child, libc::SIGKILL);
    }
    drop(crash_control);
    let crash_evidence = evidence.join("crash-before-restore");
    std::fs::create_dir(&crash_evidence).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    for entry in std::fs::read_dir(fixture.state()).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_file() {
            std::fs::copy(entry.path(), crash_evidence.join(entry.file_name())).unwrap();
        }
    }
    let mut restored = LiveDashboard::new(fixture.reopen_after_release("Process: Interrupted"));
    restored.wait("No automatic restart");
    restored.send(b"h");
    restored.wait("pace17 run=7");
    drop(restored);
    let mut restored = Controller::connect(&fixture);
    assert_no_owned_coding(&fixture);
    assert_eq!(restored.greeting["identity"], acknowledged["identity"]);
    assert!(
        restored.greeting["durable_bytes"].as_u64().unwrap()
            >= acknowledged["durable_bytes"].as_u64().unwrap()
    );
    let refused = restored.request(serde_json::json!({"Launch":"crash17-must-not-restart"}));
    assert!(refused["error"].as_str().unwrap().contains("uncertain"));
    log(
        serde_json::json!({"kind":"crash","counter_request_ns":counter_request_ns,"counter_received_ns":counter_received_ns,"kill_injected_ns":kill_injected_ns,"owner":crash_owner,"child":crash_child,"acknowledged":acknowledged,"restored":restored.greeting,"replacement":refused}),
    );
    assert_no_owned_coding(&fixture);
    let refused_shutdown = restored.request(serde_json::json!("Shutdown"));
    assert_eq!(
        refused_shutdown["stopped"], false,
        "uncertain execution cannot promise a confirmed shutdown"
    );
    log(serde_json::json!({"kind":"crash-shutdown-refused","state":refused_shutdown}));
    // Harness cleanup of its authenticated restored owner is separate from a
    // product Stop/reconciliation proof. No coding process was restarted.
    let restored_owner =
        fixture_peer_pid(&restored.stream).expect("authenticated isolated restored owner");
    drop(restored);
    // SAFETY: PID authenticated by this private fixture socket, no native child.
    unsafe {
        libc::kill(restored_owner, libc::SIGKILL);
    }
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let ps = Command::new("ps")
            .args(["-p", &restored_owner.to_string(), "-o", "stat="])
            .output()
            .unwrap();
        if !String::from_utf8_lossy(&ps.stdout)
            .lines()
            .any(|line| !line.trim().is_empty() && !line.trim().starts_with('Z'))
        {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
    let after = evidence.join("crash-after-restore");
    std::fs::create_dir(&after).unwrap();
    for entry in std::fs::read_dir(fixture.state()).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_file() {
            std::fs::copy(entry.path(), after.join(entry.file_name())).unwrap();
        }
    }
    log(
        serde_json::json!({"kind":"finished","failures":failures,"identity":identity,"final":final_state}),
    );
    assert!(failures.is_empty(), "healthy targets missed: {failures:?}");
    eprintln!("measurement17 evidence {}", evidence.display());
}

#[test]
fn paced_capture_saturation_stop_confirms_without_storage_repair() {
    let fixture = Fixture::new();
    let mut dashboard = fixture.open();
    dashboard.wait("Discovery complete");
    dashboard.send(b"a");
    dashboard.wait("Adopted");
    dashboard.send(b"l");
    dashboard.wait("fixture>");
    dashboard.send(&[0x1d]);
    dashboard.wait("Focus: dashboard");
    dashboard.kill();
    let mut controller = Controller::connect(&fixture);
    controller.request(serde_json::json!({"Resize":{"rows":80,"cols":200}}));
    controller.request(serde_json::json!({"Input":b"mosaic\r".to_vec()}));
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        let state = controller.request(serde_json::json!("Screen"));
        if state["screen"]
            .as_str()
            .unwrap()
            .contains("max geometry complete")
            && state["capture_pending_bytes"] == 0
        {
            break;
        }
        assert!(Instant::now() < until);
    }
    controller.request(serde_json::json!({"Resize":{"rows":24,"cols":104}}));
    std::fs::write(fixture.state().join("proof-storage-write"), "error").unwrap();
    controller.request(serde_json::json!({"Input":b"pace17 9 30000 262144\r".to_vec()}));
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let state = controller.request(serde_json::json!("Screen"));
        if state["producer_wait_millis"].as_u64().unwrap() > 500 {
            break;
        }
        assert!(Instant::now() < deadline, "{state}");
        std::thread::sleep(Duration::from_millis(25));
    }
    controller.request(serde_json::json!("Stop"));
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        let state = controller.request(serde_json::json!("Screen"));
        if state["lifecycle"] == "Stopped" {
            break;
        }
        assert!(Instant::now() < deadline, "{state}");
        std::thread::sleep(Duration::from_millis(25));
    }
    let stopped = controller.request(serde_json::json!("Screen"));
    assert!(stopped["capture_pending_bytes"].as_u64().unwrap() > 0);
    assert!(stopped["storage_error"].is_string());
    let cleanup = cleanup_page15(&mut controller);
    assert_eq!(cleanup["cleanup"]["entries"][0]["eligible"], false);
    std::fs::remove_file(fixture.state().join("proof-storage-write")).unwrap();
    controller.request(serde_json::json!("RetryStorage"));
    let repaired = cleanup_wait15(&mut controller, |state| {
        state["capture_pending_bytes"] == 0 && state["durable_bytes"] == state["accepted_bytes"]
    });
    assert!(
        repaired["accepted_bytes"].as_u64().unwrap() > stopped["durable_bytes"].as_u64().unwrap()
    );
    controller.request(serde_json::json!("Shutdown"));
}

#[test]
fn failed_append_does_not_invent_data_bearing_durable_commits() {
    let fixture = Fixture::new();
    let mut dashboard = fixture.open();
    dashboard.wait("Discovery complete");
    dashboard.send(b"a");
    dashboard.wait("Adopted");
    dashboard.send(b"l");
    dashboard.wait("fixture>");
    std::thread::sleep(Duration::from_millis(400));
    dashboard.kill();
    let mut controller = Controller::connect(&fixture);
    let before = controller.request(serde_json::json!("Screen"));
    std::fs::write(fixture.state().join("proof-storage-write"), "error").unwrap();
    controller.request(serde_json::json!({"Input":b"ack17 blocked\r".to_vec()}));
    std::thread::sleep(Duration::from_millis(800));
    let failed = controller.request(serde_json::json!("Screen"));
    assert_eq!(failed["durable_bytes"], before["durable_bytes"]);
    assert_eq!(
        failed["durable_flush_count"], before["durable_flush_count"],
        "failed append cannot create a data-bearing durable commit"
    );
    std::fs::remove_file(fixture.state().join("proof-storage-write")).unwrap();
    controller.request(serde_json::json!("RetryStorage"));
    let until = Instant::now() + Duration::from_secs(3);
    loop {
        let state = controller.request(serde_json::json!("Screen"));
        if state["durable_bytes"].as_u64().unwrap() > before["durable_bytes"].as_u64().unwrap() {
            break;
        }
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(25));
    }
    controller.request(serde_json::json!("Shutdown"));
}

fn cleanup_wait15(
    control: &mut Controller,
    predicate: impl Fn(&serde_json::Value) -> bool,
) -> serde_json::Value {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        let state = control.request(serde_json::json!("Screen"));
        if predicate(&state) {
            return state;
        }
        assert!(
            Instant::now() < deadline,
            "cleanup condition missing: {state}"
        );
        std::thread::sleep(Duration::from_millis(15));
    }
}
fn cleanup_page15(control: &mut Controller) -> serde_json::Value {
    control.request(serde_json::json!({"Cleanup":{"page":0}}));
    cleanup_wait15(control, |state| state["cleanup"].is_object())
}
fn cleanup_chunks15(fixture: &Fixture, id: &str) -> Vec<(PathBuf, Vec<u8>)> {
    let mut files = std::fs::read_dir(fixture.state())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(&format!("{id}-"))
                && path.extension().is_some_and(|ext| ext == "transcript")
        })
        .map(|path| {
            let bytes = std::fs::read(&path).unwrap();
            (path, bytes)
        })
        .collect::<Vec<_>>();
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
}
#[test]
fn cleanup_intent_failure_partial_crash_and_restart_preserve_other_history_and_launch_outcomes() {
    let fixture = Fixture::new();
    let sentinel = fixture.repository.join("uncommitted-work");
    std::fs::write(&sentinel, "checkout survives cleanup").unwrap();
    let mut dashboard = fixture.open();
    dashboard.wait("Discovery complete");
    dashboard.send(b"a");
    dashboard.wait("Adopted");
    dashboard.send(b"l");
    dashboard.wait("fixture>");
    dashboard.kill();
    let mut control = Controller::connect(&fixture);
    let first_identity = control.greeting["identity"].as_str().unwrap().to_owned();
    let first_request = control.greeting["launch_request"].clone();
    let first_id = first_identity
        .split("session=")
        .nth(1)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_owned();
    let active = cleanup_page15(&mut control);
    assert_eq!(active["cleanup"]["entries"][0]["eligible"], false);
    let refused = control.request(serde_json::json!({"DeleteHistory":{"id":first_id}}));
    assert!(refused["error"].as_str().unwrap().contains("unflushed"));
    control.request(serde_json::json!({"Input":b"history-bulk\r".to_vec()}));
    cleanup_wait15(&mut control, |state| {
        state["screen"]
            .as_str()
            .unwrap_or("")
            .contains("history-bulk-complete")
    });
    control.request(serde_json::json!("Stop"));
    cleanup_wait15(&mut control, |state| {
        state["lifecycle"] == "Stopped"
            && state["metadata_pending"] == false
            && state["capture_pending_bytes"] == 0
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let page = cleanup_page15(&mut control);
        if page["cleanup"]["entries"][0]["eligible"] == true {
            break;
        }
        assert!(Instant::now() < deadline);
    }
    let first = cleanup_chunks15(&fixture, &first_id);
    assert!(
        first.len() > 2,
        "partial crash must cross multiple physical chunks"
    );
    let second_request = uuid::Uuid::new_v4().to_string();
    let second = control.request(serde_json::json!({"Launch":second_request}));
    let second_identity = second["identity"].as_str().unwrap().to_owned();
    let second_id = second_identity
        .split("session=")
        .nth(1)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_owned();
    control.request(serde_json::json!("Stop"));
    cleanup_wait15(&mut control, |state| {
        state["lifecycle"] == "Stopped"
            && state["metadata_pending"] == false
            && state["capture_pending_bytes"] == 0
    });
    let page = cleanup_page15(&mut control);
    assert_eq!(
        page["cleanup"]["entries"][0]["id"], first_id,
        "oldest stopped segment is suggested first"
    );
    let second_files = cleanup_chunks15(&fixture, &second_id);
    assert!(!second_files.is_empty());
    std::fs::write(fixture.state().join("proof-storage-delete-intent"), "error").unwrap();
    control.request(serde_json::json!({"DeleteHistory":{"id":first_id}}));
    cleanup_wait15(&mut control, |state| {
        state["cleanup_message"]
            .as_str()
            .unwrap_or("")
            .contains("failed/incomplete")
    });
    assert_eq!(
        cleanup_chunks15(&fixture, &first_id),
        first,
        "failed intent must unlink nothing"
    );
    std::fs::remove_file(fixture.state().join("proof-storage-delete-intent")).unwrap();
    std::fs::write(
        fixture.state().join("proof-storage-delete-after-unlink"),
        "stall",
    )
    .unwrap();
    control.request(serde_json::json!({"DeleteHistory":{"id":first_id}}));
    let deadline = Instant::now() + Duration::from_secs(5);
    while cleanup_chunks15(&fixture, &first_id).len() == first.len() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(20));
    }
    let partial = cleanup_chunks15(&fixture, &first_id);
    assert_eq!(partial.len(), first.len() - 1);
    let pending = control.request(serde_json::json!({"History":{"index":0,"offset":0}}));
    assert!(
        !pending["cleanup_message"]
            .as_str()
            .unwrap_or("")
            .contains("completed")
    );
    let owner = fixture_peer_pid(&control.stream).unwrap();
    // SAFETY: authenticated peer of this isolated fixture; no active coding child remains.
    unsafe {
        libc::kill(owner, libc::SIGKILL);
    }
    drop(control);
    std::fs::remove_file(fixture.state().join("proof-storage-delete-after-unlink")).unwrap();
    let mut restarted = fixture.open();
    restarted.wait("Process: Stopped");
    restarted.wait("History deletion completed");
    restarted.kill();
    let mut control = Controller::connect(&fixture);
    let page = cleanup_page15(&mut control);
    assert!(
        page["cleanup"]["entries"]
            .as_array()
            .unwrap()
            .iter()
            .all(|entry| entry["id"] != first_id)
    );
    assert_eq!(cleanup_chunks15(&fixture, &first_id).len(), 0);
    assert_eq!(cleanup_chunks15(&fixture, &second_id), second_files);
    let outcome = control.request(serde_json::json!({"QueryLaunch":first_request}));
    assert_eq!(outcome["launch_result"]["identity"], first_identity);
    assert_eq!(
        std::fs::read_to_string(&sentinel).unwrap(),
        "checkout survives cleanup"
    );
    let before = control.request(serde_json::json!("Screen"))["durable_bytes"].clone();
    assert!(
        before.as_u64().unwrap() > 0,
        "other segment accounting must survive old-history deletion"
    );
    control.request(serde_json::json!("Shutdown"));
}

#[test]
fn cleanup_failures_remain_incomplete_until_directory_sync_and_completion_commit() {
    for boundary in ["delete-unlink", "delete-directory-sync", "delete-complete"] {
        let fixture = Fixture::new();
        let mut dashboard = fixture.open();
        dashboard.wait("Discovery complete");
        dashboard.send(b"a");
        dashboard.wait("Adopted");
        dashboard.send(b"l");
        dashboard.wait("fixture>");
        dashboard.kill();
        let mut control = Controller::connect(&fixture);
        let id = control.greeting["identity"]
            .as_str()
            .unwrap()
            .split("session=")
            .nth(1)
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap()
            .to_owned();
        control.request(serde_json::json!("Stop"));
        cleanup_wait15(&mut control, |state| {
            state["lifecycle"] == "Stopped" && state["metadata_pending"] == false
        });
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let page = cleanup_page15(&mut control);
            if page["cleanup"]["entries"][0]["eligible"] == true {
                break;
            }
            assert!(Instant::now() < deadline);
        }
        let initial = cleanup_chunks15(&fixture, &id);
        assert!(!initial.is_empty());
        let marker = fixture.state().join(format!("proof-storage-{boundary}"));
        std::fs::write(&marker, "error").unwrap();
        control.request(serde_json::json!({"DeleteHistory":{"id":id}}));
        let failed = cleanup_wait15(&mut control, |state| {
            state["cleanup_message"]
                .as_str()
                .unwrap_or("")
                .contains("incomplete")
        });
        assert!(
            failed["durable_bytes"].as_u64().unwrap() > 0,
            "no completed deletion accounting on failure"
        );
        let page = cleanup_page15(&mut control);
        assert_eq!(
            page["cleanup"]["entries"][0]["status"],
            "incomplete deletion; explicitly retry"
        );
        assert_eq!(page["cleanup"]["entries"][0]["id"], id);
        control.request(serde_json::json!({"History":{"index":0,"offset":0}}));
        cleanup_wait15(&mut control, |state| {
            state["error"]
                .as_str()
                .unwrap_or("")
                .contains("No retained session")
        });
        if boundary == "delete-unlink" {
            assert_eq!(cleanup_chunks15(&fixture, &id), initial);
        } else {
            assert!(cleanup_chunks15(&fixture, &id).is_empty());
            assert_eq!(page["cleanup"]["entries"][0]["bytes"], 0);
        }
        std::fs::remove_file(marker).unwrap();
        control.request(serde_json::json!({"DeleteHistory":{"id":id}}));
        let done = cleanup_wait15(&mut control, |state| {
            state["cleanup_message"]
                .as_str()
                .unwrap_or("")
                .contains("completed")
        });
        assert_eq!(done["durable_bytes"], 0);
        assert!(
            cleanup_page15(&mut control)["cleanup"]["entries"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        control.request(serde_json::json!("Shutdown"));
    }
}

#[test]
fn cleanup_never_confuses_process_stop_or_freed_space_with_capture_recovery() {
    let fixture = Fixture::new();
    let mut dashboard = fixture.open();
    dashboard.wait("Discovery complete");
    dashboard.send(b"a");
    dashboard.wait("Adopted");
    dashboard.send(b"l");
    dashboard.wait("fixture>");
    dashboard.kill();
    let mut control = Controller::connect(&fixture);
    let first = control.greeting["identity"]
        .as_str()
        .unwrap()
        .split("session=")
        .nth(1)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_owned();
    control.request(serde_json::json!("Stop"));
    cleanup_wait15(&mut control, |state| {
        state["lifecycle"] == "Stopped" && state["metadata_pending"] == false
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    while cleanup_page15(&mut control)["cleanup"]["entries"][0]["eligible"] != true {
        assert!(Instant::now() < deadline);
    }
    control.request(serde_json::json!({"Launch":uuid::Uuid::new_v4().to_string()}));
    cleanup_wait15(&mut control, |state| {
        state["durable_bytes"].as_u64().unwrap_or(0) > 0
    });
    std::fs::write(fixture.state().join("proof-storage-available"), "0").unwrap();
    std::fs::write(fixture.state().join("proof-storage-write"), "error").unwrap();
    control.request(serde_json::json!({"Input":b"retain-despite-cleanup15\r".to_vec()}));
    cleanup_wait15(&mut control, |state| {
        state["storage_error"].is_string()
            && state["capture_pending_bytes"].as_u64().unwrap_or(0) > 0
    });
    control.request(serde_json::json!("Stop"));
    let stopped = cleanup_wait15(&mut control, |state| state["lifecycle"] == "Stopped");
    let current = stopped["identity"]
        .as_str()
        .unwrap()
        .split("session=")
        .nth(1)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_owned();
    let snapshot = cleanup_chunks15(&fixture, &current);
    let refused = control.request(serde_json::json!({"DeleteHistory":{"id":current}}));
    assert!(refused["error"].as_str().unwrap().contains("unflushed"));
    assert_eq!(cleanup_chunks15(&fixture, &current), snapshot);
    control.request(serde_json::json!({"DeleteHistory":{"id":first}}));
    let deleted = cleanup_wait15(&mut control, |state| {
        state["cleanup_message"]
            .as_str()
            .unwrap_or("")
            .contains("completed")
            && state["capacity"]["available"] == 0
    });
    assert!(
        deleted["storage_error"].is_string(),
        "cleanup must not acknowledge capture recovery"
    );
    assert!(deleted["capture_pending_bytes"].as_u64().unwrap() > 0);
    std::fs::remove_file(fixture.state().join("proof-storage-write")).unwrap();
    control.request(serde_json::json!("RetryStorage"));
    let recovered = cleanup_wait15(&mut control, |state| {
        state["capture_pending_bytes"] == 0
            && state["storage_error"].is_null()
            && state["durable_bytes"].as_u64().unwrap_or(0)
                > snapshot.iter().map(|(_, bytes)| bytes.len() as u64).sum()
    });
    assert_eq!(
        recovered["capacity"]["available"], 0,
        "capacity remains low despite successful write recovery"
    );
    control.request(serde_json::json!("Shutdown"));
}

#[test]
fn cleanup_migrated_age_is_unknown_pages_are_bounded_and_arbitrary_paths_are_refused() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new();
    std::fs::create_dir(fixture.state()).unwrap();
    std::fs::set_permissions(fixture.state(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let sentinel = fixture.repository.join("keep.txt");
    std::fs::write(&sentinel, b"untouched").unwrap();
    let db = rusqlite::Connection::open(fixture.state().join("history.sqlite3")).unwrap();
    db.execute_batch("CREATE TABLE app (id INTEGER PRIMARY KEY CHECK(id=1), state TEXT NOT NULL, checkout BLOB, task TEXT NOT NULL); CREATE TABLE sessions (id TEXT PRIMARY KEY, identity TEXT NOT NULL, state TEXT NOT NULL, durable INTEGER NOT NULL DEFAULT 0); PRAGMA user_version=1;").unwrap();
    for index in 1..=10 {
        let id = format!("11111111-2222-4333-8444-{index:012}");
        let identity = format!("task=legacy-task session={id} request=legacy-{index}");
        let state = serde_json::json!({"adopted":"legacy-checkout","identity":identity,"screen":"","formatted":"","rows":24,"cols":100,"output_bytes":4,"error":null,"stopped":false,"lifecycle":"Stopped","control_denied":false,"launch_request":format!("legacy-{index}")});
        db.execute(
            "INSERT INTO sessions VALUES(?1,?2,?3,4)",
            rusqlite::params![id, identity, state.to_string()],
        )
        .unwrap();
        std::fs::write(
            fixture.state().join(format!("{id}-00000000.transcript")),
            b"keep",
        )
        .unwrap();
    }
    drop(db);
    let mut dashboard = fixture.open();
    dashboard.wait("Discovery complete");
    dashboard.kill();
    let mut control = Controller::connect(&fixture);
    let page = cleanup_page15(&mut control);
    assert_eq!(page["cleanup"]["entries"].as_array().unwrap().len(), 8);
    assert_eq!(page["cleanup"]["next"], true);
    for entry in page["cleanup"]["entries"].as_array().unwrap() {
        assert!(
            entry["created"].is_null(),
            "migration must not invent historical timestamps"
        );
        assert_eq!(
            entry["eligible"], false,
            "legacy stopped lifecycle alone cannot prove final capture EOF"
        );
        assert!(entry["bytes"].as_u64().unwrap() >= 4);
    }
    control.request(serde_json::json!({"Cleanup":{"page":1}}));
    let second = cleanup_wait15(&mut control, |state| state["cleanup"]["index"] == 1);
    assert_eq!(second["cleanup"]["entries"].as_array().unwrap().len(), 2);
    assert_eq!(second["cleanup"]["next"], false);
    control.request(serde_json::json!({"DeleteHistory":{"id":sentinel.display().to_string()}}));
    cleanup_wait15(&mut control, |state| {
        state["cleanup_message"]
            .as_str()
            .unwrap_or("")
            .contains("failed/incomplete")
    });
    assert_eq!(std::fs::read(&sentinel).unwrap(), b"untouched");
    control.request(serde_json::json!("Shutdown"));
}

#[test]
fn cleanup_rejects_symlink_chunks_without_following_checkout_targets() {
    let fixture = Fixture::new();
    let sentinel = fixture.repository.join("keep.txt");
    std::fs::write(&sentinel, b"untouched").unwrap();
    let mut dashboard = fixture.open();
    dashboard.wait("Discovery complete");
    dashboard.send(b"a");
    dashboard.wait("Adopted");
    dashboard.send(b"l");
    dashboard.wait("fixture>");
    dashboard.kill();
    let mut control = Controller::connect(&fixture);
    let id = control.greeting["identity"]
        .as_str()
        .unwrap()
        .split("session=")
        .nth(1)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_owned();
    control.request(serde_json::json!("Stop"));
    cleanup_wait15(&mut control, |state| {
        state["lifecycle"] == "Stopped" && state["metadata_pending"] == false
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    while cleanup_page15(&mut control)["cleanup"]["entries"][0]["eligible"] != true {
        assert!(Instant::now() < deadline);
    }
    let chunks = cleanup_chunks15(&fixture, &id);
    let backup = fixture.temp.path().join("original-chunk");
    std::fs::rename(&chunks[0].0, &backup).unwrap();
    std::os::unix::fs::symlink(&sentinel, &chunks[0].0).unwrap();
    let page = cleanup_page15(&mut control);
    assert_eq!(page["cleanup"]["entries"][0]["eligible"], false);
    assert!(page["cleanup"]["entries"][0]["bytes"].is_null());
    control.request(serde_json::json!({"DeleteHistory":{"id":id}}));
    cleanup_wait15(&mut control, |state| {
        state["cleanup_message"]
            .as_str()
            .unwrap_or("")
            .contains("incomplete")
    });
    assert!(
        std::fs::symlink_metadata(&chunks[0].0)
            .unwrap()
            .is_symlink()
    );
    assert_eq!(std::fs::read(&sentinel).unwrap(), b"untouched");
    std::fs::remove_file(&chunks[0].0).unwrap();
    std::fs::rename(&backup, &chunks[0].0).unwrap();
    control.request(serde_json::json!({"DeleteHistory":{"id":id}}));
    cleanup_wait15(&mut control, |state| {
        state["cleanup_message"]
            .as_str()
            .unwrap_or("")
            .contains("completed")
    });
    assert_eq!(std::fs::read(&sentinel).unwrap(), b"untouched");
    control.request(serde_json::json!("Shutdown"));
}

#[test]
fn paced_fixture_receives_complete_input_while_its_output_is_backpressured() {
    let fixture = Fixture::new();
    let mut dashboard = fixture.open();
    dashboard.wait("Discovery complete");
    dashboard.send(b"a");
    dashboard.wait("Adopted");
    dashboard.send(b"l");
    dashboard.wait("fixture>");
    std::fs::write(fixture.state().join("proof-storage-ledger"), "enabled").unwrap();
    std::fs::write(fixture.state().join("proof-storage-write"), "error").unwrap();
    dashboard.send(b"pace17 8 30000 262144\r");
    dashboard.wait("pace17 run=8");
    dashboard.send(&[0x1d]);
    dashboard.wait("Focus: dashboard");
    dashboard.kill();
    let mut control = Controller::connect(&fixture);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let state = control.request(serde_json::json!("Screen"));
        if state["producer_wait_millis"].as_u64().unwrap() > 500 {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(25));
    }
    control.request(serde_json::json!({"Input":b"ack17 queued8\r".to_vec()}));
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        if std::fs::read_to_string(fixture.state().join("proof-input17.jsonl"))
            .is_ok_and(|s| s.contains("queued8"))
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "input receipt must precede stdout acquisition"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    let repaired_ns = measurement_clock();
    std::fs::remove_file(fixture.state().join("proof-storage-write")).unwrap();
    control.request(serde_json::json!("RetryStorage"));
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        let state = control.request(serde_json::json!("Screen"));
        if state["screen"].as_str().unwrap().contains("ACK17 queued8 ") {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
    let receipt: serde_json::Value = serde_json::from_str(
        std::fs::read_to_string(fixture.state().join("proof-input17.jsonl"))
            .unwrap()
            .lines()
            .find(|l| l.contains("queued8"))
            .unwrap(),
    )
    .unwrap();
    assert!(receipt["received_ns"].as_u64().unwrap() < repaired_ns);
    measurement_shutdown(&mut control);
}

#[test]
#[ignore = "serial 30-second full-storage input receipt supplement; RAT_FAULT17_DIR required"]
fn full_storage_input_receipt_measurement17() {
    let fixture = Fixture::new();
    let evidence =
        PathBuf::from(std::env::var_os("RAT_FAULT17_DIR").expect("set unique evidence directory"));
    std::fs::create_dir(&evidence).unwrap();
    let _failure_evidence = FailedMeasurementEvidence {
        state: fixture.state(),
        evidence: evidence.clone(),
    };
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(evidence.join("observations.jsonl"))
        .unwrap();
    let mut log = |event: serde_json::Value| writeln!(file, "{event}").unwrap();
    log(
        serde_json::json!({"kind":"manifest","fault_supplement":true,"revision":String::from_utf8(Command::new("git").args(["rev-parse","HEAD"]).output().unwrap().stdout).unwrap().trim(),"dirty":!Command::new("git").args(["diff","HEAD","--quiet"]).status().unwrap().success(),"profile":"debug","outer_terminal":"portable-pty + vt100 0.18.1; no graphical/pixel timing","outer_geometry":[40,160],"dwell_ms":30000,"rate_Bps":262144,"duration_ms":60000,"record_bytes":1024,"navigation_samples":0,"attach_samples":0,"fault_navigation_samples":20,"ack_censor_ms":2000,"caps":{"normal_capture_bytes":270336,"stop_tail_bytes":1048576,"stop_tail_blocks":256,"commit_records":1024,"outer_blocks":8,"outer_block_bytes":4096},"fault":"debug ENOSPC before real write, not actual disk exhaustion","fixture_change":"ordinary stdin bytes do not acquire stdout; one complete ACK can wait at stdout after receipt","resource_claim":"supplement measures input receipt/recovery only; full resource proof remains final02","instrumentation":"continuous bounded latest terminal parser, 1ms visible polling, producer/input JSONL, bounded per-commit ring"}),
    );
    for (command, args) in [
        ("sw_vers", vec![]),
        ("uname", vec!["-mrs"]),
        ("rustc", vec!["-Vv"]),
        (
            "sysctl",
            vec!["-n", "hw.model", "hw.memsize", "hw.logicalcpu"],
        ),
        (
            "shasum",
            vec!["-a", "256", env!("CARGO_BIN_EXE_agent-terminal")],
        ),
    ] {
        let output = Command::new(command).args(args).output().unwrap();
        log(
            serde_json::json!({"kind":"environment","command":command,"stdout":String::from_utf8_lossy(&output.stdout)}),
        );
    }
    assert!(
        Command::new("git")
            .arg("-C")
            .arg(&fixture.repository)
            .args([
                "-c",
                "user.name=Proof",
                "-c",
                "user.email=proof@example.invalid",
                "commit",
                "--allow-empty",
                "-m",
                "fault-supplement17"
            ])
            .output()
            .unwrap()
            .status
            .success()
    );
    assert!(
        Command::new("git")
            .arg("-C")
            .arg(&fixture.repository)
            .args(["worktree", "add", "-b", "other17"])
            .arg(fixture.temp.path().join("other17"))
            .output()
            .unwrap()
            .status
            .success()
    );
    let mut dashboard = LiveDashboard::new(fixture.open());
    dashboard.wait("Discovery complete");
    dashboard.send(b"a");
    dashboard.wait("Adopted");
    dashboard.send(b"l");
    let launch = dashboard.wait("fixture>").0;
    log(
        serde_json::json!({"kind":"launch","screen":launch,"dashboard":dashboard.dashboard.child.process_id()}),
    );
    std::fs::write(fixture.state().join("proof-storage-ledger"), "enabled").unwrap();
    std::fs::write(fixture.state().join("proof-storage-write"), "error").unwrap();
    dashboard.send(b"pace17 1 60000 262144\r");
    dashboard.wait("pace17 run=1");
    dashboard.send(&[0x1d]);
    dashboard.wait("Focus: dashboard");
    log(serde_json::json!({"kind":"fault-window-start","run":1,"started_ns":measurement_clock()}));
    std::thread::sleep(Duration::from_secs(30));
    for sample in 0..20 {
        let (key, expected) = if sample % 2 == 0 {
            (b"j", "Branch: other17")
        } else {
            (b"k", "Branch: main")
        };
        let injected_ns = measurement_clock();
        dashboard.send(key);
        let visible_ns = dashboard.wait(expected).1;
        log(
            serde_json::json!({"kind":"navigation","mode":"full-storage","sample":sample,"injected_ns":injected_ns,"visible_ns":visible_ns,"us":visible_ns.saturating_sub(injected_ns)/1000}),
        );
    }
    let injected_ns = measurement_clock();
    dashboard.send(b"\r");
    let focus_ns = dashboard.wait("Focus: session").1;
    dashboard.send(b"ack17 fault17-supplement\r");
    let visible = dashboard.observe("ACK17 fault17-supplement ", Duration::from_secs(2));
    let censor_ns = measurement_clock();
    let receipt: serde_json::Value = serde_json::from_str(
        std::fs::read_to_string(fixture.state().join("proof-input17.jsonl"))
            .expect("complete native input receipt under blocked stdout")
            .lines()
            .find(|line| line.contains("fault17-supplement"))
            .unwrap(),
    )
    .unwrap();
    let received_ns = receipt["received_ns"].as_u64().unwrap();
    assert!(injected_ns <= received_ns && received_ns < censor_ns);
    assert!(
        visible.is_none(),
        "persistent ENOSPC must not fabricate a visible output ACK"
    );
    log(
        serde_json::json!({"kind":"attach","mode":"full-storage","nonce":"fault17-supplement","injected_ns":injected_ns,"focus_ns":focus_ns,"received_ns":received_ns,"visible_ns":null,"censor_ns":censor_ns,"screen":dashboard.latest.lock().unwrap().0}),
    );
    dashboard.send(&[0x1d]);
    dashboard.wait("Focus: dashboard");
    let retry_ns = measurement_clock();
    std::fs::remove_file(fixture.state().join("proof-storage-write")).unwrap();
    dashboard.send(b"y");
    let recovered = dashboard.wait("ACK17 fault17-supplement ");
    log(
        serde_json::json!({"kind":"fault-recovered","retry_ns":retry_ns,"visible_ns":recovered.1,"screen":recovered.0}),
    );
    drop(dashboard);
    let mut control = Controller::connect(&fixture);
    let identity = control.greeting["identity"].clone();
    fixture.assert_one_owner_and_child();
    control.request(serde_json::json!("Stop"));
    cleanup_wait15(&mut control, |state| {
        state["lifecycle"] == "Stopped"
            && state["metadata_pending"] == false
            && state["capture_pending_bytes"] == 0
            && state["accepted_bytes"] == state["durable_bytes"]
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    while cleanup_page15(&mut control)["cleanup"]["entries"][0]["eligible"] != true {
        assert!(
            Instant::now() < deadline,
            "final capture must seal before retained History checks"
        );
    }
    let ready = control.request(serde_json::json!("Screen"));
    assert_eq!(ready["accepted_bytes"], ready["durable_bytes"]);
    let mut last = 0;
    for commit in ready["durable_commits"].as_array().unwrap() {
        let seq = commit["sequence"].as_u64().unwrap();
        assert_eq!(seq, last + 1);
        last = seq;
        log(serde_json::json!({"kind":"commit","commit":commit}));
    }
    for offset in [
        0,
        ready["durable_bytes"]
            .as_u64()
            .unwrap()
            .saturating_sub(1024),
    ] {
        let deadline = Instant::now() + Duration::from_secs(4);
        loop {
            let state = control.request(serde_json::json!({"History":{"index":0,"offset":offset}}));
            if state["history"]["offset"] == offset {
                assert!(!state["history"]["text"].as_str().unwrap().is_empty());
                log(serde_json::json!({"kind":"public-history","history":state["history"]}));
                break;
            }
            assert!(Instant::now() < deadline);
        }
    }
    let shutdown = measurement_shutdown(&mut control);
    assert_eq!(shutdown["accepted_bytes"], shutdown["durable_bytes"]);
    drop(control);
    measurement_wait_endpoint_exit(&fixture);
    for entry in std::fs::read_dir(fixture.state()).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_file() {
            std::fs::copy(entry.path(), evidence.join(entry.file_name())).unwrap();
        }
    }
    log(serde_json::json!({"kind":"finished","failures":[],"identity":identity,"final":shutdown}));
    eprintln!("fault supplement evidence {}", evidence.display());
}

#[test]
fn native_process_outcomes_survive_cleanup_reload_and_explicit_resume() {
    use std::os::unix::fs::PermissionsExt;
    for (command, lifecycle, code, signal) in [
        ("fail", "Failed", Some(7), false),
        ("exit", "Exited", Some(0), false),
        ("signal", "Failed", None, true),
    ] {
        let fixture = Fixture::new();
        let tools = fixture.temp.path().join("native-tools");
        std::fs::create_dir(&tools).unwrap();
        let wrapper = tools.join("claude");
        std::fs::write(&wrapper, "#!/bin/sh\nprintf 'native-outcome-ready\\n'\nwhile IFS= read -r command; do\ncase \"$command\" in\nintermediate) false; printf 'intermediate command failed; still alive\\n';;\nexit) printf 'final-native-output\\n'; exit 0;;\nfail) printf 'final-native-output\\n'; exit 7;;\nsignal) printf 'final-native-output\\n'; kill -TERM $$;;\nesac\ndone\n").unwrap();
        std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o700)).unwrap();
        let path = std::env::join_paths(
            std::iter::once(tools).chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
        )
        .unwrap();
        let mut dashboard =
            Dashboard::open_with_path(&fixture.repository, &fixture.state(), Some(&path));
        dashboard.wait("Discovery complete");
        dashboard.send(b"a");
        dashboard.wait("Adopted");
        dashboard.send(b"c");
        dashboard.wait("native-outcome-ready");
        dashboard.send(b"intermediate\r");
        dashboard.wait("intermediate command failed; still alive");
        dashboard.kill();
        let mut control = Controller::connect(&fixture);
        assert_eq!(control.greeting["lifecycle"], "Running");
        assert!(
            control.greeting["process_exit"].is_null(),
            "intermediate command is not a native process failure"
        );
        let identity = control.greeting["identity"].clone();
        let native = control.greeting["native_conversation"].clone();
        if command == "fail" {
            std::fs::write(fixture.state().join("proof-storage-sync"), "error").unwrap();
        }
        control.request(serde_json::json!({"Input":format!("{command}\r").as_bytes()}));
        let finished = cleanup_wait15(&mut control, |state| {
            state["lifecycle"] != "Running"
                && state["lifecycle"] != "Stopping"
                && state["metadata_pending"] == false
        });
        assert_eq!(
            finished["lifecycle"], lifecycle,
            "original native outcome must survive descendant cleanup"
        );
        assert!(finished["execution"].is_null());
        if let Some(code) = code {
            assert_eq!(finished["process_exit"]["code"], code);
        }
        assert_eq!(finished["process_exit"]["signal"].is_string(), signal);
        if command == "fail" {
            let fault = cleanup_wait15(&mut control, |state| state["storage_error"].is_string());
            assert_eq!(fault["process_exit"], finished["process_exit"]);
            let blocked = control.request(serde_json::json!({"Resume":"blocked-until-final-sync"}));
            assert!(
                blocked["launch_result"]["error"]
                    .as_str()
                    .unwrap()
                    .contains("history is still pending"),
                "blocked state: {blocked}"
            );
            assert_eq!(blocked["identity"], identity);
            assert_eq!(blocked["lifecycle"], "Failed");
            assert_eq!(blocked["process_exit"], finished["process_exit"]);
            std::fs::remove_file(fixture.state().join("proof-storage-sync")).unwrap();
            control.request(serde_json::json!("RetryStorage"));
        }
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            if cleanup_page15(&mut control)["cleanup"]["entries"][0]["eligible"] == true {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "finished history must flush and seal"
            );
        }
        control.request(serde_json::json!({"History":{"index":0,"offset":0}}));
        let history = cleanup_wait15(&mut control, |state| state["history"].is_object());
        assert!(
            history["history"]["text"]
                .as_str()
                .unwrap()
                .contains("final-native-output")
        );
        assert_eq!(history["history"]["process_exit"], finished["process_exit"]);
        kill_owner(&fixture);
        drop(control);
        let mut dashboard =
            fixture.reopen_with_path_after_release(&format!("Process: {lifecycle}"), Some(&path));
        if lifecycle == "Failed" {
            dashboard.wait("Native process failed:");
        }
        dashboard.kill();
        let mut control = Controller::connect(&fixture);
        assert_eq!(control.greeting["identity"], identity);
        assert_eq!(control.greeting["native_conversation"], native);
        assert_eq!(control.greeting["process_exit"], finished["process_exit"]);
        assert_no_owned_coding(&fixture);
        let resumed =
            control.request(serde_json::json!({"Resume":format!("explicit-after-{command}")}));
        assert_ne!(resumed["identity"], identity);
        assert_eq!(resumed["native_conversation"], native);
        cleanup_wait15(&mut control, |state| state["lifecycle"] == "Running");
        control.request(serde_json::json!("Stop"));
        let stopped = cleanup_wait15(&mut control, |state| state["lifecycle"] == "Stopped");
        assert!(
            stopped["process_exit"].is_null(),
            "explicit Stop is not spontaneous process failure"
        );
        control.request(serde_json::json!("Shutdown"));
    }
}

#[test]
fn partial_spawn_failure_keeps_gated_process_owned_until_stop() {
    for boundary in [
        "before-scope",
        "after-create",
        "before-input",
        "before-capture",
        "before-authorization",
    ] {
        let fixture = Fixture::new();
        let mut dashboard = fixture.open();
        dashboard.wait("Discovery complete");
        dashboard.send(b"a");
        dashboard.wait("Adopted");
        dashboard.kill();
        let mut control = Controller::connect(&fixture);
        std::fs::write(fixture.state().join("proof-launch-setup"), boundary).unwrap();
        let failed = control.request(serde_json::json!({"Launch":"partial-create"}));
        assert!(
            failed["error"]
                .as_str()
                .unwrap_or("")
                .contains("Injected launch setup failure"),
            "{failed}"
        );
        assert!(!failed["screen"].as_str().unwrap().contains("fixture>"));
        let pid = failed["execution"]["pid"]
            .as_i64()
            .expect("owned gated process");
        control.request(serde_json::json!("Stop"));
        cleanup_wait15(&mut control, |state| {
            matches!(state["lifecycle"].as_str(), Some("Stopped" | "Failed"))
                && state["metadata_pending"] == false
        });
        assert!(
            !Command::new("/bin/ps")
                .args(["-p", &pid.to_string(), "-o", "stat="])
                .output()
                .unwrap()
                .stdout
                .iter()
                .any(|b| !b.is_ascii_whitespace())
        );
        // Shutdown's confirmed receipt also establishes the final capture and
        // metadata barrier, independently of the earlier process-stop receipt.
        control.request(serde_json::json!("Shutdown"));
        cleanup_wait15(&mut control, |state| state["stopped"] == true);
        drop(control);
        std::fs::remove_file(fixture.state().join("proof-launch-setup")).unwrap();
        let mut dashboard = fixture.open();
        dashboard.wait("Adopted:");
        dashboard.kill();
        let mut control = Controller::connect(&fixture);
        let replacement = control.request(serde_json::json!({"Launch":"after-partial"}));
        assert!(replacement["error"].is_null(), "{replacement}");
        cleanup_wait15(&mut control, |state| {
            state["screen"].as_str().unwrap().contains("fixture>")
        });
        control.request(serde_json::json!("Shutdown"));
    }
}

#[test]
fn stop_during_outstanding_creation_never_releases_late_gated_process() {
    let fixture = Fixture::new();
    let mut dashboard = fixture.open();
    dashboard.wait("Discovery complete");
    dashboard.send(b"a");
    dashboard.wait("Adopted");
    dashboard.kill();
    let mut control = Controller::connect(&fixture);
    let pause = fixture.state().join("proof-launch-creation");
    std::fs::write(&pause, "pause").unwrap();
    control.request_now(serde_json::json!({"Launch":"cancel-creation"}));
    let ready = pause.with_extension("ready");
    let deadline = Instant::now() + Duration::from_secs(3);
    while !ready.exists() {
        assert!(
            Instant::now() < deadline,
            "creation worker did not reach gated child"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let provisional = control.request_now(serde_json::json!({"Launch":"cancel-creation"}));
    assert!(
        provisional["launch_result"].is_null(),
        "same request remains provisional: {provisional}"
    );
    let query = control.request_now(serde_json::json!({"QueryLaunch":"cancel-creation"}));
    assert!(
        query["launch_result"].is_null(),
        "query must not manufacture a failure: {query}"
    );
    let pid = std::fs::read_to_string(&ready)
        .unwrap()
        .parse::<i32>()
        .unwrap();
    let started = Instant::now();
    let stopping = control.request_now(serde_json::json!("Stop"));
    assert!(started.elapsed() < Duration::from_secs(1));
    assert_eq!(stopping["lifecycle"], "Stopping");
    assert_eq!(stopping["stopped"], false);
    assert!(
        stopping["error"]
            .as_str()
            .unwrap()
            .contains("cleanup pending")
    );
    let refused = control.request_now(serde_json::json!({"Launch":"replacement-too-soon"}));
    assert!(refused["error"].is_string(), "{refused}");
    std::fs::remove_file(pause).unwrap();
    let stopped = cleanup_wait15(&mut control, |state| {
        state["lifecycle"] == "Stopped" && state["metadata_pending"] == false
    });
    assert!(!stopped["screen"].as_str().unwrap().contains("fixture>"));
    assert!(
        !Command::new("/bin/ps")
            .args(["-p", &pid.to_string(), "-o", "stat="])
            .output()
            .unwrap()
            .stdout
            .iter()
            .any(|b| !b.is_ascii_whitespace())
    );
    let result = control.request(serde_json::json!({"QueryLaunch":"cancel-creation"}));
    assert!(
        result["launch_result"]["error"]
            .as_str()
            .unwrap()
            .contains("cancelled")
    );
    control.request(serde_json::json!("Shutdown"));
}

#[test]
fn partial_spawn_cleanup_failure_retains_reservation_and_retries() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new();
    let tools = fixture.temp.path().join("partial-tools");
    std::fs::create_dir(&tools).unwrap();
    let ps = tools.join("ps");
    std::fs::write(&ps, "#!/bin/sh\nexit 1\n").unwrap();
    std::fs::set_permissions(&ps, std::fs::Permissions::from_mode(0o700)).unwrap();
    let path = std::env::join_paths(
        std::iter::once(tools).chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    let mut dashboard =
        Dashboard::open_with_path(&fixture.repository, &fixture.state(), Some(&path));
    dashboard.wait("Discovery complete");
    dashboard.send(b"a");
    dashboard.wait("Adopted");
    dashboard.kill();
    let mut control = Controller::connect(&fixture);
    std::fs::write(fixture.state().join("proof-launch-setup"), "after-create").unwrap();
    let failed = control.request(serde_json::json!({"Launch":"partial-cleanup"}));
    let pid = failed["execution"]["pid"].as_i64().unwrap();
    control.request(serde_json::json!("Stop"));
    let blocked = cleanup_wait15(&mut control, |state| state["lifecycle"] == "StopFailed");
    assert_eq!(blocked["execution"]["pid"], pid);
    let refused = control.request(serde_json::json!({"Launch":"partial-cleanup-replacement"}));
    assert!(refused["error"].is_string());
    assert_eq!(refused["execution"]["pid"], pid);
    std::fs::remove_file(&ps).unwrap();
    std::os::unix::fs::symlink("/bin/ps", ps).unwrap();
    control.request(serde_json::json!("Stop"));
    let cleaned = cleanup_wait15(&mut control, |state| {
        state["lifecycle"] == "Stopped" && state["metadata_pending"] == false
    });
    assert!(cleaned["execution"].is_null());
    assert!(!cleaned["screen"].as_str().unwrap().contains("fixture>"));
    control.request(serde_json::json!("Shutdown"));
}

#[test]
fn failed_capture_installation_never_borrows_previous_sessions_eof_proof() {
    let fixture = Fixture::new();
    let mut dashboard = fixture.open();
    dashboard.wait("Discovery complete");
    dashboard.send(b"a");
    dashboard.wait("Adopted");
    dashboard.kill();
    let mut control = Controller::connect(&fixture);
    control.request(serde_json::json!({"Launch":"completed-capture"}));
    cleanup_wait15(&mut control, |state| {
        state["screen"].as_str().unwrap().contains("fixture>")
    });
    control.request(serde_json::json!("Stop"));
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let page = cleanup_page15(&mut control);
        if page["cleanup"]["entries"][0]["eligible"] == true {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "previous capture must settle: {page}"
        );
    }
    std::fs::write(fixture.state().join("proof-launch-setup"), "before-capture").unwrap();
    let failed = control.request(serde_json::json!({"Launch":"capture-not-installed"}));
    let id = failed["session_identity"]["session"].clone();
    control.request(serde_json::json!("Stop"));
    cleanup_wait15(&mut control, |state| {
        matches!(state["lifecycle"].as_str(), Some("Stopped" | "Failed"))
            && state["metadata_pending"] == false
    });
    control.request(serde_json::json!("Shutdown"));
    cleanup_wait15(&mut control, |state| state["stopped"] == true);
    drop(control);
    let mut dashboard = fixture.open();
    dashboard.wait("Adopted:");
    dashboard.kill();
    let mut control = Controller::connect(&fixture);
    let page = cleanup_page15(&mut control);
    let entry = page["cleanup"]["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["id"] == id)
        .unwrap();
    assert_eq!(
        entry["eligible"], false,
        "no reader observed this session's EOF: {page}"
    );
    control.request(serde_json::json!("Shutdown"));
}

#[test]
fn slow_adoption_and_saturated_discovery_leave_polling_stop_and_exit_usable() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new();
    let bin = fixture.temp.path().join("bin04");
    std::fs::create_dir(&bin).unwrap();
    let marker = fixture.temp.path().join("slow-git04");
    let git = bin.join("git");
    std::fs::write(
        &git,
        format!(
            "#!/bin/sh\nif [ -f '{}' ]; then /bin/sleep 10; fi\nexec /usr/bin/git \"$@\"\n",
            marker.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&git, std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut paths = vec![bin];
    paths.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap()));
    let mut dashboard = Dashboard::open_with_path(
        &fixture.repository,
        &fixture.state(),
        Some(&std::env::join_paths(paths).unwrap()),
    );
    dashboard.wait("Discovery complete");
    dashboard.send(b"a");
    dashboard.wait("Adopted");
    dashboard.send(b"l");
    dashboard.wait("fixture>");
    dashboard.kill();
    let mut control = Controller::connect(&fixture);
    std::fs::write(&marker, b"hold git only").unwrap();
    let adopted = control.request_now(
        serde_json::json!({"Adopt":fixture.repository.as_os_str().as_encoded_bytes()}),
    );
    assert_eq!(adopted["adoption_pending"], true);
    let previous = adopted["adopted"].clone();
    for request in 1..8 {
        let start = Instant::now();
        let result = control.request_now(serde_json::json!({"Discover":{"request":request,"repository":fixture.repository.as_os_str().as_encoded_bytes()}}));
        assert_eq!(result["lifecycle"], "Running");
        assert!(
            start.elapsed() < Duration::from_millis(500),
            "Git dispatch blocked controls"
        );
    }
    let start = Instant::now();
    control.request_now(serde_json::json!("Stop"));
    let stopped = cleanup_wait15(&mut control, |state| state["lifecycle"] == "Stopped");
    assert!(
        start.elapsed() < Duration::from_secs(2),
        "Stop waited for slow Git"
    );
    assert_eq!(stopped["adoption_pending"], false);
    assert_eq!(
        stopped["adopted"], previous,
        "cancelled adoption cannot change checkout"
    );
    let start = Instant::now();
    control.request_now(serde_json::json!("Shutdown"));
    cleanup_wait15(&mut control, |state| state["stopped"] == true);
    assert!(
        start.elapsed() < Duration::from_secs(2),
        "Shutdown waited for slow Git"
    );
}

#[test]
fn dashboard_refuses_legacy_live_owner_before_sending_new_git_requests() {
    use std::os::unix::{fs::PermissionsExt, net::UnixListener};
    let fixture = Fixture::new();
    std::fs::create_dir(fixture.state()).unwrap();
    std::fs::set_permissions(fixture.state(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let lock_path = fixture.state().join("supervisor.lock");
    let lock = std::fs::File::create(&lock_path).unwrap();
    std::fs::set_permissions(&lock_path, std::fs::Permissions::from_mode(0o600)).unwrap();
    lock.try_lock().unwrap();
    let listener = UnixListener::bind(fixture.state().join("supervisor.sock")).unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let old = serde_json::json!({"adopted":null,"identity":null,"screen":"","formatted":"","rows":24,"cols":100,"output_bytes":0,"error":null,"stopped":false,"lifecycle":"Idle","control_denied":false});
        serde_json::to_writer(&mut stream, &old).unwrap();
        stream.write_all(b"\n").unwrap();
        let mut unexpected = [0; 1];
        assert_eq!(
            stream.read(&mut unexpected).unwrap(),
            0,
            "dashboard sent unsupported worktree request to live legacy owner"
        );
    });
    let mut dashboard = fixture.open();
    dashboard.wait("does not support this dashboard");
    dashboard.send(b"d");
    server.join().unwrap();
}
