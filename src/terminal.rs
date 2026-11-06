use std::io;

use crossterm::{
    event::{DisableBracketedPaste, EnableBracketedPaste},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

pub type DashboardTerminal = Terminal<CrosstermBackend<io::Stdout>>;

pub struct TerminalGuard;

impl TerminalGuard {
    /// Deliberate terminal ownership handoff; no dashboard input is read while the editor owns it.
    pub fn release_editor(&mut self, terminal: &mut DashboardTerminal) -> io::Result<()> {
        terminal.show_cursor()?;
        disable_raw_mode()?;
        execute!(io::stdout(), DisableBracketedPaste, LeaveAlternateScreen)
    }

    pub fn reacquire_editor(&mut self, terminal: &mut DashboardTerminal) -> io::Result<()> {
        // The editor foreground group must be relinquished before dashboard input/render.
        crate::editor::foreground(unsafe { libc::getpgrp() })?;
        enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen, EnableBracketedPaste)?;
        // Fullscreen resize resets buffers without requesting a cursor-position report.
        // Terminal::clear preserves the cursor via DSR, which is unnecessary after handoff.
        let area = terminal.size()?.into();
        terminal.resize(area)?;
        Ok(())
    }

    pub fn enter() -> io::Result<(Self, DashboardTerminal)> {
        let previous_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            restore();
            previous_hook(info);
        }));
        enable_raw_mode()?;
        let guard = Self;
        execute!(io::stdout(), EnterAlternateScreen, EnableBracketedPaste)?;
        let terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
        Ok((guard, terminal))
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore();
    }
}

fn restore() {
    let _ = disable_raw_mode();
    let _ = execute!(
        io::stdout(),
        DisableBracketedPaste,
        LeaveAlternateScreen,
        crossterm::cursor::Show
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use portable_pty::{CommandBuilder, PtySize, native_pty_system};
    use std::io::Read;

    #[test]
    #[ignore = "subprocess probe; driven with a real PTY by restoration test"]
    fn terminal_probe() {
        let scenario = std::env::var("RAT_TERMINAL_PROBE").unwrap();
        let outcome = || -> io::Result<()> {
            let (_guard, _terminal) = TerminalGuard::enter()?;
            match scenario.as_str() {
                "panic" => panic!("deliberate terminal restoration probe"),
                "error" => Err(io::Error::other("deliberate restoration probe")),
                _ => Ok(()),
            }
        };
        let _ = outcome();
    }

    #[test]
    fn restores_terminal_modes_after_normal_error_and_panic() {
        for scenario in ["normal", "error", "panic"] {
            let pty = native_pty_system().openpty(PtySize::default()).unwrap();
            let before = pty.master.get_termios().unwrap();
            let mut command = CommandBuilder::new(std::env::current_exe().unwrap());
            command.args([
                "--ignored",
                "--exact",
                "terminal::tests::terminal_probe",
                "--nocapture",
            ]);
            command.env("RAT_TERMINAL_PROBE", scenario);
            let mut reader = pty.master.try_clone_reader().unwrap();
            let output = std::thread::spawn(move || {
                let mut output = Vec::new();
                let mut buffer = [0; 4096];
                while let Ok(n) = reader.read(&mut buffer) {
                    if n == 0 {
                        break;
                    }
                    output.extend_from_slice(&buffer[..n]);
                }
                output
            });
            let mut child = pty.slave.spawn_command(command).unwrap();
            drop(pty.slave);
            child.wait().unwrap();
            let after = pty.master.get_termios().unwrap();
            assert_eq!(before, after, "raw-mode restoration: {scenario}");
            drop(pty.master);
            let bytes = output.join().unwrap();
            for sequence in [
                b"\x1b[?1049h".as_slice(),
                b"\x1b[?1049l",
                b"\x1b[?2004h",
                b"\x1b[?2004l",
            ] {
                assert!(
                    bytes
                        .windows(sequence.len())
                        .any(|window| window == sequence),
                    "missing mode transition {sequence:?}: {scenario}"
                );
            }
        }
    }
}
