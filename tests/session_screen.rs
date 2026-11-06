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
    master: Box<dyn portable_pty::MasterPty + Send>,
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
        command.env("TERM", "xterm-256color");
        command.env("NO_COLOR", "");
        command.arg(repository);
        command.arg("--state-dir");
        command.arg(state);
        let child = pty.slave.spawn_command(command).unwrap();
        drop(pty.slave);
        let mut reader = pty.master.try_clone_reader().unwrap();
        let writer = pty.master.take_writer().unwrap();
        let (tx, output) = mpsc::channel();
        std::thread::spawn(move || {
            let mut bytes = [0; 4096];
            while let Ok(n) = reader.read(&mut bytes) {
                if n == 0 || tx.send(bytes[..n].to_vec()).is_err() {
                    break;
                }
            }
        });
        Self {
            master: pty.master,
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
    fn capture(&self, name: &str) {
        if let Ok(directory) = std::env::var("RAT_SCREEN_EVIDENCE") {
            let (rows, cols) = self.screen.screen().size();
            let mut cells: Vec<_> = (0..rows.get()).flat_map(|row| (0..cols.get()).map(move |col| {
                let cell = self.screen.screen().cell(row,col).unwrap();
                serde_json::json!({"row":row,"col":col,"text":cell.contents(),"fg":format!("{:?}",cell.fgcolor()),"bg":format!("{:?}",cell.bgcolor()),"bold":cell.bold(),"wide":cell.is_wide_continuation()})
            })).collect();
            cells.retain(|cell| !cell["text"].as_str().unwrap().trim().is_empty());
            let cursor = self.screen.screen().cursor_position();
            std::fs::create_dir_all(&directory).unwrap();
            std::fs::write(
                Path::new(&directory).join(format!("{name}.json")),
                serde_json::to_vec_pretty(
                    &serde_json::json!({"rows": rows.get(), "cols": cols.get(), "cells": cells, "cursor": [cursor.0,cursor.1], "hide_cursor": self.screen.screen().hide_cursor()}),
                )
                .unwrap(),
            )
            .unwrap();
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
fn screen_resize_styles_alternate_modes_and_output_survive_focus_and_reconnect() {
    let fixture = Fixture::new();
    let mut dashboard = fixture.open();
    dashboard.wait("Discovery complete");
    dashboard.send(b"a");
    dashboard.wait("Adopted");
    dashboard.send(b"l");
    dashboard.wait("fixture>");
    dashboard.send(b"size\r");
    dashboard.wait("size: 104x32");
    dashboard.send(b"screen\r");
    dashboard.wait("styled λ界");
    let screen = dashboard.screen.screen();
    let (row, col) = (0..40)
        .find_map(|r| {
            (0..160).find_map(|c| {
                (screen.cell(r, c).unwrap().contents() == "s"
                    && screen
                        .cell(r, c + 1)
                        .is_some_and(|cell| cell.contents() == "t")
                    && screen
                        .cell(r, c + 2)
                        .is_some_and(|cell| cell.contents() == "y"))
                .then_some((r, c))
            })
        })
        .unwrap();
    assert!(
        screen.cell(row, col).unwrap().bold(),
        "at {row},{col}: {:?}; {}",
        screen.cell(row, col),
        screen.contents()
    );
    assert_eq!(
        screen.cell(row, col).unwrap().fgcolor(),
        vt100::Color::Idx(1)
    );
    assert!(screen.cell(row, col + 8).unwrap().is_wide());
    assert!(screen.cell(row, col + 9).unwrap().is_wide_continuation());
    assert!(!screen.hide_cursor());
    assert_eq!(screen.cursor_position(), (2, 64));
    dashboard.capture("04-normal");
    dashboard
        .master
        .resize(PtySize {
            rows: 24,
            cols: 100,
            pixel_width: 0,
            pixel_height: 0,
        })
        .unwrap();
    dashboard
        .screen
        .set_size(24.try_into().unwrap(), 100.try_into().unwrap());
    std::thread::sleep(Duration::from_millis(150));
    dashboard.send(b"size\r");
    dashboard.wait("size: 64x16");
    dashboard.send(b"screen\r");
    dashboard.wait("styled λ界");
    dashboard.send(&[0x1d]);
    dashboard.wait("Focus: dashboard");
    dashboard
        .master
        .resize(PtySize {
            rows: 18,
            cols: 60,
            pixel_width: 0,
            pixel_height: 0,
        })
        .unwrap();
    dashboard
        .screen
        .set_size(18.try_into().unwrap(), 60.try_into().unwrap());
    std::thread::sleep(Duration::from_millis(150));
    dashboard.send(b"\rsize\r");
    dashboard.wait("size: 58x11");
    dashboard.capture("04-small");
    // A pane without an interior must preserve the last usable child geometry.
    dashboard
        .master
        .resize(PtySize {
            rows: 4,
            cols: 12,
            pixel_width: 0,
            pixel_height: 0,
        })
        .unwrap();
    dashboard
        .screen
        .set_size(4.try_into().unwrap(), 12.try_into().unwrap());
    std::thread::sleep(Duration::from_millis(200));
    dashboard.send(b"screen\rsize\r");
    std::thread::sleep(Duration::from_millis(200));
    dashboard
        .master
        .resize(PtySize {
            rows: 18,
            cols: 60,
            pixel_width: 0,
            pixel_height: 0,
        })
        .unwrap();
    dashboard
        .screen
        .set_size(18.try_into().unwrap(), 60.try_into().unwrap());
    dashboard.wait("size: 58x11");

    dashboard.send(b"alternate\r");
    dashboard.wait("alternate screen");
    assert!(!dashboard.screen.screen().contents().contains("styled λ界"));
    assert_eq!(
        dashboard.screen.screen().mouse_protocol_mode(),
        vt100::MouseProtocolMode::None
    );
    dashboard.send(b"primary\r");
    dashboard.wait("primary restored");
    dashboard.send(b"stream\r");
    dashboard.wait("stream: 0");
    dashboard.send(&[0x1d]);
    dashboard.wait("Focus: dashboard");
    dashboard.send(b"\r");
    dashboard.wait("Focus: session");
    dashboard.wait("stream: 0199");
    dashboard.kill();
    std::thread::sleep(Duration::from_millis(150));
    // Observe the screen boundary through the same public supervisor protocol,
    // then emit a known byte count with no dashboard subscriber attached.
    use std::{
        io::{BufRead, BufReader},
        os::unix::net::UnixStream,
    };
    let mut connection = UnixStream::connect(fixture.state().join("supervisor.sock")).unwrap();
    connection
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let mut reader = BufReader::new(connection.try_clone().unwrap());
    let mut frame = String::new();
    reader.read_line(&mut frame).unwrap();
    let before: serde_json::Value = serde_json::from_str(&frame).unwrap();
    assert!(!before["control_denied"].as_bool().unwrap());
    connection
        .write_all(b"{\"Input\":[115,116,114,101,97,109,13]}\n")
        .unwrap();
    frame.clear();
    reader.read_line(&mut frame).unwrap();
    drop(reader);
    drop(connection);
    let started = Instant::now();
    // Exercise a real disconnected interval; it does not establish completion.
    std::thread::sleep(Duration::from_millis(1300));
    let mut connection = UnixStream::connect(fixture.state().join("supervisor.sock")).unwrap();
    connection
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let mut reader = BufReader::new(connection.try_clone().unwrap());
    frame.clear();
    reader.read_line(&mut frame).unwrap();
    // A 5ms per-record sleep is a minimum, not a producer completion signal.
    // Wait for the full known stream, including its trailing prompt, while
    // preserving exact byte equality so missing/duplicated bytes still fail.
    let expected_output = before["output_bytes"].as_u64().unwrap() + 200 * 14 + 9;
    let after = loop {
        let state: serde_json::Value = serde_json::from_str(&frame).unwrap();
        let output = state["output_bytes"].as_u64().unwrap();
        assert!(
            output <= expected_output,
            "duplicated fixture output: {state}"
        );
        if output == expected_output {
            eprintln!(
                "detached fixture completed exactly 2809 bytes in {:?}",
                started.elapsed()
            );
            break state;
        }
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "detached fixture did not complete: expected {expected_output} bytes, observed {state}"
        );
        std::thread::sleep(Duration::from_millis(10));
        connection.write_all(b"\"Screen\"\n").unwrap();
        frame.clear();
        reader.read_line(&mut frame).unwrap();
    };
    assert_eq!(after["identity"], before["identity"]);
    assert_eq!(
        after["output_bytes"].as_u64().unwrap() - before["output_bytes"].as_u64().unwrap(),
        200 * 14 + 9,
        "each emitted byte is parsed once across the disconnected snapshot boundary"
    );
    assert!(after["screen"].as_str().unwrap().contains("stream: 0199"));
    connection
        .write_all(b"{\"Resize\":{\"rows\":65535,\"cols\":65535}}\n")
        .unwrap();
    frame.clear();
    reader.read_line(&mut frame).unwrap();
    let geometry: serde_json::Value = serde_json::from_str(&frame).unwrap();
    assert_eq!(geometry["rows"], 80);
    assert_eq!(geometry["cols"], 200);
    connection
        .write_all(b"{\"Input\":[109,111,115,97,105,99,13]}\n")
        .unwrap();
    frame.clear();
    reader.read_line(&mut frame).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        connection.write_all(b"\"Screen\"\n").unwrap();
        frame.clear();
        reader.read_line(&mut frame).unwrap();
        assert!(frame.len() < 4 * 1024 * 1024);
        let snapshot: serde_json::Value = serde_json::from_str(&frame).unwrap();
        if snapshot["screen"]
            .as_str()
            .unwrap()
            .contains("max geometry complete")
        {
            eprintln!(
                "maximum 200x80 styled snapshot: {} encoded bytes",
                frame.len()
            );
            assert!(snapshot["formatted"].as_str().unwrap().contains("38;2;"));
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(20));
    }
    // Restore geometry before reconnecting so the final stream marker remains visible.
    connection
        .write_all(b"{\"Resize\":{\"rows\":11,\"cols\":58}}\n")
        .unwrap();
    frame.clear();
    reader.read_line(&mut frame).unwrap();
    connection
        .write_all(b"{\"Input\":[115,116,114,101,97,109,13]}\n")
        .unwrap();
    frame.clear();
    reader.read_line(&mut frame).unwrap();
    std::thread::sleep(Duration::from_millis(1300));
    drop(reader);
    drop(connection);
    std::thread::sleep(Duration::from_millis(100));
    let mut dashboard = fixture.open();
    dashboard.wait("stream: 0199");
    dashboard.send(b"identity\r");
    dashboard.wait("same-child pid=");
    dashboard.send(&[0x1d]);
    dashboard.wait("Focus: dashboard");
    dashboard.send(b"q");
}
