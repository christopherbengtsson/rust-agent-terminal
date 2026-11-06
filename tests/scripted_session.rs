use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
    path::PathBuf,
    process::Command,
    time::{Duration, Instant},
};

struct Cleanup {
    child: Box<dyn portable_pty::Child + Send + Sync>,
    socket: PathBuf,
}
impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Ok(mut stream) = UnixStream::connect(&self.socket) {
            let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
            let _ = stream.write_all(b"\"Shutdown\"\n");
            let mut bytes = [0; 1];
            let _ = stream.read(&mut bytes);
        }
    }
}

#[test]
fn dashboard_adopts_launches_and_interacts_without_discovery_mutation() {
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
    let before = Command::new("git")
        .arg("-C")
        .arg(&repository)
        .args(["status", "--porcelain"])
        .output()
        .unwrap()
        .stdout;
    let pty = native_pty_system()
        .openpty(PtySize {
            rows: 30,
            cols: 120,
            pixel_width: 0,
            pixel_height: 0,
        })
        .unwrap();
    let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_agent-terminal"));
    command.arg(&repository);
    command.arg("--state-dir");
    command.arg(temp.path().join("state"));
    let mut cleanup = Cleanup {
        child: pty.slave.spawn_command(command).unwrap(),
        socket: temp.path().join("state/supervisor.sock"),
    };
    drop(pty.slave);
    let mut reader = pty.master.try_clone_reader().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut bytes = [0; 4096];
        while let Ok(n) = reader.read(&mut bytes) {
            if n == 0 {
                break;
            }
            if tx.send(bytes[..n].to_vec()).is_err() {
                break;
            }
        }
    });
    let mut parser = vt100::Parser::new(30.try_into().unwrap(), 120.try_into().unwrap(), 0);
    let mut wait = |needle: &str| {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if parser.screen().contents().contains(needle) {
                break parser.screen().contents();
            }
            assert!(
                Instant::now() < deadline,
                "missing {needle}: {}",
                parser.screen().contents()
            );
            if let Ok(bytes) = rx.recv_timeout(Duration::from_millis(100)) {
                parser.process(&bytes)
            }
        }
    };
    wait("Discovery complete");
    assert_eq!(std::fs::read_dir(&repository).unwrap().count(), 1);
    let mut writer = pty.master.take_writer().unwrap();
    writer.write_all(b"a").unwrap();
    wait("Adopted");
    writer.write_all(b"l").unwrap();
    wait("fixture>");
    writer.write_all(b"status-working\r").unwrap();
    wait("semantic status: working");
    writer.write_all(b"status-wrong-session\r").unwrap();
    wait("invalid or unattributed evidence");
    let screen = wait(&format!(
        "cwd={}",
        repository.canonicalize().unwrap().display()
    ));
    let pid = screen
        .split("pid=")
        .nth(1)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_string();
    wait("task=");
    writer.write_all(b"status-working\r").unwrap();
    wait("semantic status: working");
    writer.write_all(b"fixture-failed-test\r").unwrap();
    wait("test result: FAILED; agent continues");
    wait("semantic status: working");
    writer.write_all(b"status-native-question\r").unwrap();
    wait("semantic status: input needed");
    writer.write_all(b"status-native-post\r").unwrap();
    wait("semantic status: unknown");

    writer.write_all(b"status-native-approval\r").unwrap();
    wait("semantic status: approval needed");
    for (command, label) in [
        ("status-completed\r", "turn completed"),
        ("status-interrupted\r", "interrupted"),
        ("status-input\r", "input needed"),
    ] {
        writer.write_all(command.as_bytes()).unwrap();
        wait(&format!("semantic status: {label}"));
    }
    wait("evidence stale");
    wait("semantic status: unknown");
    writer
        .write_all(b"\x1b[200~Completed. Approval needed. Interrupted. Enter to select.\x1b[201~\r")
        .unwrap();
    wait("Completed. Approval needed.");
    wait("semantic status: unknown");

    writer.write_all(b"hello\r").unwrap();
    wait("received: hello");
    writer.write_all(&[0xce]).unwrap();
    std::thread::sleep(Duration::from_millis(30));
    writer.write_all(&[0xbb, b'\r']).unwrap();
    wait("received: λ");
    writer
        .write_all(b"\x1b[200~alpha\n\xce\xb2\x1b[201~\r")
        .unwrap();
    wait("bytes: 1b5b3230307e616c7068610aceb21b5b3230317e");
    writer.write_all(b"\x1bx\x1b[1;5A\x1bOP\x1b[Z\r").unwrap();
    wait("bytes: 1b781b5b313b35411b4f501b5b5a");
    writer.write_all(b"cursor-on\r").unwrap();
    wait("cursor application");
    writer.write_all(b"\x1b[A\r").unwrap();
    wait("bytes: 1b4f41");
    writer.write_all(b"paste-off\r").unwrap();
    wait("paste plain");
    writer
        .write_all(b"\x1b[200~plain\nline\x1b[201~\r")
        .unwrap();
    wait("received: plain");
    wait("received: line");
    writer.write_all(b"approve\r").unwrap();
    wait("Approve action? [y/n]");
    writer.write_all(b"y\r").unwrap();
    wait("approval: yes");
    writer.write_all(&[3]).unwrap();
    wait("interrupted: 03");
    wait("Focus: session");
    writer.write_all(&[0x1d]).unwrap();
    wait("Focus: dashboard");
    writer.write_all(b"\r").unwrap();
    wait("Focus: session");
    writer.write_all(b"identity\r").unwrap();
    wait(&format!("same-child pid={pid}"));
    writer.write_all(b"again\r").unwrap();
    wait("received: again");
    writer.write_all(&[0x1d]).unwrap();
    wait("Focus: dashboard");
    writer.write_all(b"q").unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if cleanup.child.try_wait().unwrap().is_some() {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(20));
    }
    let after = Command::new("git")
        .arg("-C")
        .arg(&repository)
        .args(["status", "--porcelain"])
        .output()
        .unwrap()
        .stdout;
    assert_eq!(before, after);
    assert!(
        !temp.path().join("state/supervisor.sock").exists(),
        "confirmed shutdown removes the isolated endpoint"
    );
}
