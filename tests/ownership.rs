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
        command.arg(repository);
        command.arg("--state-dir");
        command.arg(state);
        let child = pty.slave.spawn_command(command).unwrap();
        drop(pty.slave);
        let mut reader = pty.master.try_clone_reader().unwrap();
        let writer = pty.master.take_writer().unwrap();
        let (tx, output) = mpsc::channel();
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
        let _ = self.child.kill();
        let _ = self.child.wait();
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
