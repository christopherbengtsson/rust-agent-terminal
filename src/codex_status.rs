//! Passive invocation-local Codex hooks; status never follows terminal prose alone.
use crate::native_status::{State, Status};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Read},
    os::unix::{fs::PermissionsExt, net::UnixDatagram},
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Serialize, Deserialize)]
struct Receipt {
    token: String,
    session: String,
    turn: String,
    event: String,
    tool: String,
    tool_id: String,
    at: u64,
    main: bool,
}
struct Pending {
    tool: String,
    id: String,
    permission: bool,
    received: Instant,
}
pub struct Monitor {
    socket: UnixDatagram,
    path: PathBuf,
    token: String,
    session: Option<String>,
    turn: Option<String>,
    status: Status,
    pending: Vec<Pending>,
    supported: bool,
    barrier: u64,
    boundary: u64,
    received: Option<Instant>,
    model_pending: bool,
    baseline_marker: Option<String>,
    stopped_boundary: Option<u64>,
}
fn millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}
impl Monitor {
    pub fn start(directory: &Path) -> io::Result<Self> {
        let path = directory.join("codex-status.sock");
        if path.exists() {
            fs::remove_file(&path)?;
        }
        let socket = UnixDatagram::bind(&path)?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
        socket.set_nonblocking(true)?;
        Ok(Self {
            socket,
            path,
            token: uuid::Uuid::new_v4().to_string(),
            session: None,
            turn: None,
            status: Status {
                reason: "Codex evidence unavailable".into(),
                ..Status::default()
            },
            pending: Vec::new(),
            supported: false,
            barrier: millis(),
            boundary: 0,
            received: None,
            model_pending: false,
            baseline_marker: None,
            stopped_boundary: None,
        })
    }
    pub fn native_session(&self) -> Option<&str> {
        self.session.as_deref()
    }
    pub fn configure(&self, command: &mut portable_pty::CommandBuilder) -> io::Result<()> {
        let quote = |value: &str| format!("'{}'", value.replace('\'', "'\\''"));
        let executable = std::env::current_exe()?;
        let helper =
            format!(
                "{} --codex-status-hook {} {}",
                quote(executable.to_str().ok_or_else(|| io::Error::other(
                    "Codex monitor requires UTF-8 executable path"
                ))?),
                quote(
                    self.path.to_str().ok_or_else(|| io::Error::other(
                        "Codex monitor requires UTF-8 state path"
                    ))?
                ),
                quote(&self.token)
            );
        for event in [
            "SessionStart",
            "PreModelRequest",
            "PostModelRequest",
            "UserPromptSubmit",
            "PreToolUse",
            "PermissionRequest",
            "PostToolUse",
            "Stop",
            "Interrupt",
            "SessionEnd",
        ] {
            command.arg("-c");
            command.arg(format!(
                "hooks.{event}=[{{hooks=[{{type=\"command\",command={},timeout=1}}]}}]",
                serde_json::to_string(&helper)?
            ));
        }
        Ok(())
    }
    fn unknown(&mut self, reason: &str) {
        self.status.state = State::Unknown;
        self.status.reason = reason.into();
        self.pending.clear();
        self.model_pending = false;
        self.stopped_boundary = None;
    }
    pub fn input(&mut self, boundary: u64) {
        self.barrier = millis();
        self.boundary = boundary;
        self.unknown("native input; fresh Codex evidence required");
        self.status.evidence = None;
    }
    pub fn poll(&mut self, running: bool, screen: &str, boundary: u64) -> Status {
        if screen
            .lines()
            .take(4)
            .any(|line| line.trim() == ">_ OpenAI Codex (v0.167.0)")
        {
            self.supported = true;
        }
        let mut bytes = [0; 2049];
        for count in 0..32 {
            match self.socket.recv(&mut bytes) {
                Ok(n) => {
                    let receipt = serde_json::from_slice::<Receipt>(&bytes[..n]);
                    let Ok(receipt) = receipt else {
                        self.unknown("invalid or unattributed Codex evidence");
                        continue;
                    };
                    let now = millis();
                    if n > 2048
                        || receipt.token != self.token
                        || !receipt.main
                        || (receipt.event != "SessionStart" && receipt.at < self.barrier)
                        || receipt.at > now
                        || now.saturating_sub(receipt.at) > 30_000
                        || uuid::Uuid::parse_str(&receipt.session).is_err()
                    {
                        self.unknown("invalid or unattributed Codex evidence");
                        continue;
                    }
                    if receipt.event == "SessionStart" && self.session.is_none() {
                        self.session = Some(receipt.session.clone());
                    }
                    if self.native_session() != Some(&receipt.session) {
                        self.unknown("invalid or unattributed Codex evidence");
                        continue;
                    }
                    if receipt.event == "UserPromptSubmit"
                        && uuid::Uuid::parse_str(&receipt.turn).is_ok()
                    {
                        self.turn = Some(receipt.turn.clone());
                        self.baseline_marker =
                            done_marker(screen).map(|(_, marker)| marker.to_owned());
                        self.stopped_boundary = None;
                        self.pending.clear();
                    } else if receipt.event != "SessionStart"
                        && receipt.event != "SessionEnd"
                        && (uuid::Uuid::parse_str(&receipt.turn).is_err()
                            || self.turn.as_deref() != Some(&receipt.turn))
                    {
                        self.unknown("invalid or unattributed Codex evidence");
                        continue;
                    }
                    self.received = Some(Instant::now());
                    self.status.state = State::Unknown;
                    self.status.reason = "Codex native support gate open".into();
                    if self.status.recent.len() == 16 {
                        self.status.recent.remove(0);
                    }
                    self.status.recent.push(format!(
                        "{}:{}:{}:{}",
                        receipt.event, receipt.turn, receipt.tool, receipt.tool_id
                    ));
                    self.status.evidence = Some(
                        match receipt.event.as_str() {
                            "SessionStart" => "native session started",
                            "PreModelRequest" => "native model request started",
                            "PostModelRequest" => {
                                "native model request returned (outcome unclassified)"
                            }
                            "UserPromptSubmit" => "prompt submitted (processing unconfirmed)",
                            "PreToolUse" => "tool requested (execution unconfirmed)",
                            "PermissionRequest" => "permission requested (dialog unconfirmed)",
                            "PostToolUse" => "tool returned (outcome unclassified)",
                            "Stop" => "response stop (continuation unconfirmed)",
                            "Interrupt" => "native active-turn interruption acknowledged",
                            "SessionEnd" => "native session ended",
                            _ => "unsupported Codex event",
                        }
                        .into(),
                    );
                    match receipt.event.as_str() {
                        "PreToolUse" if !receipt.tool_id.is_empty() && self.pending.len() < 16 => {
                            if self.pending.iter().any(|p| p.id == receipt.tool_id) {
                                self.pending.clear();
                            } else {
                                self.pending.push(Pending {
                                    tool: receipt.tool,
                                    id: receipt.tool_id,
                                    permission: false,
                                    received: Instant::now(),
                                });
                            }
                        }
                        "PermissionRequest" => {
                            let matching: Vec<_> = self
                                .pending
                                .iter_mut()
                                .filter(|p| {
                                    p.tool == receipt.tool
                                        && (receipt.tool_id.is_empty() || p.id == receipt.tool_id)
                                })
                                .collect();
                            if matching.len() == 1 {
                                matching.into_iter().next().unwrap().permission = true;
                            } else {
                                self.pending.clear();
                            }
                        }
                        "PostToolUse" => self.pending.retain(|p| p.id != receipt.tool_id),
                        "Interrupt" | "Stop" | "SessionEnd" => self.pending.clear(),
                        _ => {}
                    }
                    if receipt.event == "Stop" {
                        self.stopped_boundary = Some(boundary);
                    } else {
                        self.stopped_boundary = None;
                    }
                    if self.supported && receipt.event == "PreModelRequest" {
                        if self.model_pending {
                            self.unknown("overlapping model evidence; ordering uncertain");
                        } else {
                            self.model_pending = true;
                            self.status.state = State::Working;
                        }
                    } else if receipt.event == "PostModelRequest" {
                        self.model_pending = false;
                    }
                    if self.supported && receipt.event == "Interrupt" {
                        self.status.state = State::Interrupted;
                    }
                    if count == 31 {
                        self.unknown("Codex evidence backlog; ordering uncertain");
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(_) => {
                    self.unknown("Codex evidence channel unavailable");
                    break;
                }
            }
        }
        if self
            .received
            .is_some_and(|at| at.elapsed() > Duration::from_secs(30))
        {
            self.unknown("Codex evidence stale");
        }
        if self.status.state == State::Working
            && self
                .received
                .is_some_and(|at| at.elapsed() > Duration::from_secs(5))
        {
            self.unknown("Codex model evidence stale");
        }
        if matches!(
            self.status.state,
            State::Input | State::Approval | State::Completed
        ) {
            self.status.state = State::Unknown;
            self.status.reason = "current Codex screen no longer corroborates evidence".into();
        }
        // A hook interval and native modal structure must agree. Printed phrases alone
        // are deliberately insufficient. Input clears the interval before PTY delivery.
        let lines: Vec<_> = screen.lines().map(str::trim).collect();
        let footer = lines
            .iter()
            .rev()
            .find(|line| !line.is_empty())
            .copied()
            .unwrap_or_default();
        let fresh: Vec<_> = self
            .pending
            .iter()
            .filter(|p| p.received.elapsed() <= Duration::from_secs(30))
            .collect();
        if running && self.supported && boundary > self.boundary && fresh.len() == 1 {
            let pending = fresh[0];
            if pending.tool == "request_user_input"
                && !pending.permission
                && footer == "tab to add notes | enter to submit answer | esc to interrupt"
                && lines
                    .iter()
                    .any(|line| line.starts_with("Question ") && line.contains(" unanswered)"))
                && lines.iter().any(|line| line.starts_with("› 1."))
            {
                self.status.state = State::Input;
                self.status.evidence =
                    Some("open question tool + current Codex 0.167.0 modal".into());
                self.status.reason =
                    "30s bounded native question evidence; support gate open".into();
            }
            if pending.tool == "Bash"
                && pending.permission
                && footer == "Press enter to confirm or esc to cancel"
                && lines.contains(&"Would you like to run the following command?")
                && lines.contains(&"› 1. Yes, proceed (y)")
                && lines.contains(&"3. No, and tell Codex what to do differently (esc)")
            {
                self.status.state = State::Approval;
                self.status.evidence =
                    Some("open permission request + current Codex 0.167.0 modal".into());
                self.status.reason =
                    "30s bounded native approval evidence; support gate open".into();
            }
        }
        if running
            && self.supported
            && self.stopped_boundary.is_some_and(|stop| boundary > stop)
            && let Some((marker_index, marker)) = done_marker(screen)
            && self.baseline_marker.as_deref() != Some(marker)
            && footer == "← for agents · ? for shortcuts"
            && let Some(composer) = lines
                .iter()
                .rposition(|line| *line == "› Ask Codex to do anything")
            && marker_index < composer
            && lines[marker_index + 1..composer]
                .iter()
                .all(|line| line.is_empty())
        {
            self.status.state = State::Completed;
            self.status.evidence =
                Some("current-turn Stop + new Codex 0.167.0 done marker and idle composer".into());
            self.status.reason = "30s bounded completion evidence; support gate open".into();
        }
        if !running {
            self.unknown("execution not running");
        }
        self.status.clone()
    }
}
impl Drop for Monitor {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// Project native stdin to bounded identity fields; never output decisions or raw payloads.
pub fn hook(path: &Path, token: String) {
    let _ = (|| -> io::Result<()> {
        let mut bytes = Vec::new();
        io::stdin().take(65537).read_to_end(&mut bytes)?;
        if bytes.len() > 65536 {
            return Ok(());
        }
        let json: serde_json::Value = serde_json::from_slice(&bytes)?;
        let field = |name: &str| {
            json.get(name)
                .and_then(|v| v.as_str())
                .filter(|s| {
                    s.len() <= 128
                        && s.chars()
                            .all(|ch| ch.is_ascii_alphanumeric() || "_-".contains(ch))
                })
                .unwrap_or_default()
                .to_owned()
        };
        let receipt = Receipt {
            token,
            session: field("session_id"),
            turn: field("turn_id"),
            event: field("hook_event_name"),
            tool: field("tool_name"),
            tool_id: field("tool_use_id"),
            at: millis(),
            main: json.get("agent_id").is_none_or(serde_json::Value::is_null),
        };
        let socket = UnixDatagram::unbound()?;
        socket.set_nonblocking(true)?;
        socket.send_to(&serde_json::to_vec(&receipt)?, path)?;
        Ok(())
    })();
}

fn done_marker(screen: &str) -> Option<(usize, &str)> {
    screen
        .lines()
        .collect::<Vec<_>>()
        .into_iter()
        .enumerate()
        .rev()
        .find_map(|(index, line)| {
            let line = line.trim();
            let rest = line.strip_prefix("Worked for ")?;
            let (duration, clock) = rest.rsplit_once(" • ")?;
            (duration.len() < 32
                && !duration.is_empty()
                && duration
                    .chars()
                    .all(|ch| ch.is_ascii_digit() || " smh".contains(ch))
                && clock.len() == 5
                && clock.as_bytes()[2] == b':'
                && clock.chars().all(|ch| ch.is_ascii_digit() || ch == ':'))
            .then_some((index, line))
        })
}
