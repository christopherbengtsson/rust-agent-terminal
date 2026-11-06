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
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            let mut dashboard = self.open();
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
impl Drop for Fixture {
    fn drop(&mut self) {
        // Only the isolated test endpoint can be stopped by this cleanup.
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(self.state(), std::fs::Permissions::from_mode(0o700));
        use std::os::unix::net::UnixStream;
        for _ in 0..20 {
            if let Ok(mut stream) = UnixStream::connect(self.state().join("supervisor.sock")) {
                let _ = stream.set_read_timeout(Some(Duration::from_millis(100)));
                let _ = stream.write_all(b"\"Shutdown\"\n");
                let mut bytes = [0; 4096];
                let _ = stream.read(&mut bytes);
                let _ = stream.read(&mut bytes);
            }
            if !self.state().join("supervisor.sock").exists() {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        if self.state().join("supervisor.sock").exists() {
            let output = Command::new("ps")
                .args(["-Ao", "pid,command"])
                .output()
                .unwrap();
            for line in String::from_utf8_lossy(&output.stdout).lines() {
                if line.contains("--supervisor") && line.contains(self.state().to_str().unwrap()) {
                    let _ = Command::new("kill")
                        .args(["-KILL", line.split_whitespace().next().unwrap()])
                        .status();
                }
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
fn unsafe_state_directory_is_explained_without_changing_discovery() {
    let fixture = Fixture::new();
    let target = fixture.temp.path().join("target");
    std::fs::create_dir(&target).unwrap();
    std::os::unix::fs::symlink(&target, fixture.state()).unwrap();
    let mut dashboard = fixture.open();
    dashboard.wait("Project / worktrees (1)");
    dashboard.wait("Supervisor state must be a user-owned private directory");
    dashboard.send(b"r");
    dashboard.wait("Project / worktrees (1)");
    assert!(!target.join("supervisor.sock").exists());
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
    dashboard.wait("readonly database");
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
    dashboard.wait("Process: Exited");
    dashboard.kill();
    let control = Controller::connect(&fixture);
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
    dashboard.wait("Process: Exited");
    dashboard.kill();
    let mut control = Controller::connect(&fixture);
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
