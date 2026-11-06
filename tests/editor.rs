//! Full dashboard/owner/terminal-editor seam; native proofs are explicitly invoked on macOS.
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use std::{
    io::{BufRead, BufReader, Read, Write},
    os::unix::net::UnixStream,
    path::Path,
    process::{Child, Command},
    sync::mpsc,
    time::{Duration, Instant},
};

struct Owner {
    child: Child,
    state: std::path::PathBuf,
}
impl Drop for Owner {
    fn drop(&mut self) {
        let until = Instant::now() + Duration::from_secs(4);
        while Instant::now() < until {
            if let Ok(mut stream) = UnixStream::connect(self.state.join("supervisor.sock")) {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
                let mut greeting = String::new();
                let _ = BufReader::new(&mut stream).read_line(&mut greeting);
                if serde_json::from_str::<serde_json::Value>(&greeting)
                    .is_ok_and(|s| s["control_denied"] != true)
                {
                    let _ = stream.write_all(b"\"Shutdown\"\n");
                    // Keep this accepted control connection until the owner exits.
                    while self.child.try_wait().ok().flatten().is_none()
                        && Instant::now() < until + Duration::from_secs(6)
                    {
                        let mut response = String::new();
                        let _ = BufReader::new(&mut stream).read_line(&mut response);
                    }
                    break;
                }
            } else if self.child.try_wait().ok().flatten().is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let until = Instant::now() + Duration::from_secs(4);
        while self.child.try_wait().ok().flatten().is_none() && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(20));
        }
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}
struct Dashboard {
    child: Box<dyn portable_pty::Child + Send + Sync>,
    master: Box<dyn portable_pty::MasterPty + Send>,
    input: Box<dyn Write + Send>,
    output: mpsc::Receiver<Vec<u8>>,
    screen: vt100::Parser,
}
impl Dashboard {
    fn open(root: &Path, state: &Path, config: &serde_json::Value) -> Self {
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: 40,
                cols: 160,
                pixel_width: 0,
                pixel_height: 0,
            })
            .unwrap();
        let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_agent-terminal"));
        command.arg(root);
        command.args([std::ffi::OsStr::new("--state-dir"), state.as_os_str()]);
        command.env("TERM", "xterm-256color");
        command.env("RAT_EDITOR_JSON", config.to_string());
        let child = pair.slave.spawn_command(command).unwrap();
        drop(pair.slave);
        let input = pair.master.take_writer().unwrap();
        let mut reader = pair.master.try_clone_reader().unwrap();
        let (tx, output) = mpsc::channel();
        std::thread::spawn(move || {
            let mut bytes = [0; 8192];
            while let Ok(n) = reader.read(&mut bytes) {
                if n == 0 || tx.send(bytes[..n].to_vec()).is_err() {
                    break;
                }
            }
        });
        Self {
            child,
            master: pair.master,
            input,
            output,
            screen: vt100::Parser::new(40.try_into().unwrap(), 160.try_into().unwrap(), 0),
        }
    }
    fn send(&mut self, bytes: &[u8]) {
        self.input.write_all(bytes).unwrap();
    }
    fn wait(&mut self, needle: &str) -> String {
        let until = Instant::now() + Duration::from_secs(12);
        loop {
            let screen = self.screen.screen().contents();
            if screen.contains(needle) {
                return screen;
            }
            assert!(Instant::now() < until, "missing {needle}: {screen}");
            if let Ok(bytes) = self.output.recv_timeout(Duration::from_millis(20)) {
                self.screen.process(&bytes);
            }
        }
    }
    fn resize(&mut self, rows: u16, cols: u16) {
        self.master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .unwrap();
        self.screen
            .screen_mut()
            .set_size(rows.try_into().unwrap(), cols.try_into().unwrap());
    }
    fn drain(&mut self) {
        while let Ok(bytes) = self.output.try_recv() {
            self.screen.process(&bytes);
        }
    }
}
impl Drop for Dashboard {
    fn drop(&mut self) {
        // The outer PTY starts an owned session/process group. Do not touch inferred PIDs.
        if self.child.try_wait().ok().flatten().is_none() {
            if let Some(pid) = self.child.process_id() {
                unsafe {
                    libc::kill(-(pid as i32), libc::SIGTERM);
                }
            }
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}
fn git(root: &Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
fn fixture() -> (tempfile::TempDir, Owner) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("checkout");
    std::fs::create_dir(&root).unwrap();
    git(&root, &["init", "-q", "-b", "main"]);
    git(&root, &["config", "user.name", "Test"]);
    git(&root, &["config", "user.email", "test@example.com"]);
    std::fs::write(root.join("file"), "base\n").unwrap();
    git(&root, &["add", "."]);
    git(&root, &["commit", "-qm", "base"]);
    git(&root, &["branch", "base"]);
    std::fs::write(root.join("file"), "committed\n").unwrap();
    git(&root, &["commit", "-qam", "work"]);
    std::fs::write(root.join("file"), "current\n").unwrap();
    let state = dir.path().join("state");
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir(&state).unwrap();
    std::fs::set_permissions(&state, std::fs::Permissions::from_mode(0o700)).unwrap();
    let child = Command::new(env!("CARGO_BIN_EXE_agent-terminal"))
        .arg("--supervisor")
        .arg(&state)
        .spawn()
        .unwrap();
    let owner = Owner { child, state };
    let until = Instant::now() + Duration::from_secs(5);
    while !owner.state.join("supervisor.sock").exists() {
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(20));
    }
    (dir, owner)
}
fn enter_diff(d: &mut Dashboard) {
    d.wait("Discovery complete");
    d.send(b"a");
    d.wait("Adopted:");
    d.send(b"f");
    d.wait("+current");
}

#[test]
fn configured_editor_missing_executable_and_nonzero_return_keep_diff_context() {
    for config in [
        serde_json::json!({"executable":"/missing/rat-editor"}),
        serde_json::json!({"executable":"/usr/bin/false"}),
    ] {
        let (dir, owner) = fixture();
        let mut d = Dashboard::open(&dir.path().join("checkout"), &owner.state, &config);
        enter_diff(&mut d);
        let before = d.master.get_termios().unwrap();
        d.send(b"e");
        d.wait(if config["executable"] == "/usr/bin/false" {
            "Editor exited"
        } else {
            "Editor failed to start/wait"
        });
        assert_eq!(before, d.master.get_termios().unwrap());
        assert!(d.screen.screen().contents().contains("+current"));
        d.send(b"q");
        let exit = d.child.wait().unwrap();
        assert!(exit.success());
    }
}

#[test]
#[ignore = "actual installed macOS Vim and Pico proof; run serially with terminal slot"]
fn macos_vim_and_pico_inspection_source_save_resize_and_supervisor_continuity() {
    assert_eq!(std::env::consts::OS, "macos");
    for editor in ["vim", "pico"] {
        let (dir, owner) = fixture();
        let root = dir.path().join("checkout");
        let config = serde_json::json!({"executable":format!("/usr/bin/{editor}"),"arguments":if editor=="vim" { vec!["-u","NONE"] } else { vec![] },"position":if editor=="vim" {"vim"} else {"plus"}});
        let mut d = Dashboard::open(&root, &owner.state, &config);
        d.wait("Discovery complete");
        d.send(b"a");
        d.wait("Adopted:");
        d.send(b"l");
        d.wait("fixture>");
        d.send(b"\x1d");
        d.wait("Focus: dashboard");
        d.send(b"f");
        d.wait("+current");
        for whole in [false, true] {
            if whole {
                d.send(b"w");
                d.wait("selected local base");
                d.send(b"brefs/heads/base\r");
                d.wait("+working-fresh");
            }
            let before = d.master.get_termios().unwrap();
            d.send(b"e");
            d.wait("comparison.diff");
            let dashboard_group = d.child.process_id().unwrap() as i32;
            let foreground = unsafe { libc::tcgetpgrp(d.master.as_raw_fd().unwrap()) };
            assert!(
                foreground > 0 && foreground != dashboard_group,
                "editor must own a separate foreground group"
            );
            // Actual native Ctrl-C retains the editor's own behavior and cannot kill the dashboard.
            d.send(b"\x03");
            d.resize(30, 100);
            if editor == "vim" {
                d.send(b"gg");
                d.wait("Inspection snapshot");
            } else {
                d.send(b"\x19");
            }
            // Agent writes can change source while the editor owns the foreground; artifact remains snapshot.
            std::fs::write(
                root.join("file"),
                if whole {
                    "whole-fresh\n"
                } else {
                    "working-fresh\n"
                },
            )
            .unwrap();
            if editor == "vim" {
                d.send(b":q\r");
            } else {
                d.send(b"\x18");
            }
            d.wait("Returned from inspection");
            assert_eq!(
                unsafe { libc::tcgetpgrp(d.master.as_raw_fd().unwrap()) },
                dashboard_group
            );
            assert!(d.child.try_wait().unwrap().is_none());
            assert_eq!(before, d.master.get_termios().unwrap());
            assert!(d.screen.screen().contents().contains(if whole {
                "Whole workspace"
            } else {
                "Working"
            }));
            d.resize(40, 160);
        }
        d.screen = vt100::Parser::new(40.try_into().unwrap(), 160.try_into().unwrap(), 0);
        d.send(b"o");
        d.wait("whole-fresh");
        if editor == "vim" {
            d.send(b"ggOeditor-saved\x1b:wq\r");
        } else {
            d.send(b"\x01editor-saved\r\x0f\r\x18");
        }
        d.wait("Returned from LIVE source editing");
        assert!(
            std::fs::read_to_string(root.join("file"))
                .unwrap()
                .contains("editor-saved")
        );
        if editor == "vim" {
            d.send(b"e");
            d.wait("comparison.diff");
            d.send(b":cq\r");
            d.wait("Editor exited exit status: 1");
            assert!(d.child.try_wait().unwrap().is_none());
        }
        std::fs::remove_file(root.join("file")).unwrap();
        d.send(b"o");
        d.wait("Source missing/unavailable");
        assert!(!root.join("file").exists());
        // Supervisor/session survived both editor handoffs and resizing.
        d.send(b"\x1b");
        d.wait("Focus: dashboard");
        d.send(b"\r");
        d.wait("Focus: session");
        d.send(b"editor-continuity\r");
        d.wait("editor-continuity");
        d.send(b"\x1d");
        d.wait("Focus: dashboard");
        d.send(b"q");
        assert!(d.child.wait().unwrap().success());
        d.drain();
    }
}
