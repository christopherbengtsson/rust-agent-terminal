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
                break;
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
    wait(&format!(
        "cwd={}",
        repository.canonicalize().unwrap().display()
    ));
    wait("task=");
    writer.write_all(b"hello\r").unwrap();
    wait("received: hello");
    writer.write_all(&[0x1d]).unwrap();
    wait("Focus: dashboard");
    writer.write_all(b"\r").unwrap();
    wait("Focus: session");
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
