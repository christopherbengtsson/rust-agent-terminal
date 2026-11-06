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
        let pty = native_pty_system()
            .openpty(PtySize {
                rows: 40,
                cols: 160,
                pixel_width: 0,
                pixel_height: 0,
            })
            .unwrap();
        let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_agent-terminal"));
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
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            let screen = self.screen.screen().contents();
            if screen.contains(needle) {
                return screen;
            }
            assert!(Instant::now() < deadline, "missing {needle}: {screen}");
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
struct Fixture {
    temp: tempfile::TempDir,
    repository: PathBuf,
}
impl Fixture {
    fn new() -> Self {
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
        Self { temp, repository }
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
