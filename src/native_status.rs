//! Semantic observations are independent of process and transport lifecycle.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum State {
    #[default]
    Unknown,
    Working,
    Input,
    Approval,
    Completed,
    Interrupted,
}
impl State {
    pub fn label(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Working => "working",
            Self::Input => "input needed",
            Self::Approval => "approval needed",
            Self::Completed => "turn completed",
            Self::Interrupted => "interrupted",
        }
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Status {
    pub state: State,
    pub evidence: Option<String>,
    pub reason: String,
    pub recent: Vec<String>,
}

use std::{
    fs::{self, OpenOptions},
    io::{self, Read, Write},
    os::unix::{
        fs::{FileTypeExt, OpenOptionsExt, PermissionsExt},
        net::UnixDatagram,
    },
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const MAX_RECEIPT: usize = 2048;
const FRESH: Duration = Duration::from_secs(5);

/// Only sanitized event identity is retained; prompts, tool inputs and results are discarded.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Receipt {
    token: String,
    session: String,
    event: String,
    tool: String,
    main_agent: bool,
    #[serde(default)]
    tool_id: String,
    #[serde(default)]
    prompt_id: String,
    #[serde(default)]
    at: u64,
}

struct PendingTool {
    tool: String,
    id: String,
    prompt: String,
    permission: bool,
    observed: Instant,
}

pub struct Monitor {
    socket: UnixDatagram,
    path: PathBuf,
    settings: PathBuf,
    token: String,
    session: String,
    fixture: bool,
    status: Status,
    received: Option<Instant>,
    pending: Vec<PendingTool>,
    supported_screen: bool,
    supported_executable: bool,
    started: bool,
    input_boundary: Option<u64>,
    barrier: u64,
    turn: Option<Instant>,
    turn_at: u64,
    prompt: String,
    baseline_marker: Option<String>,
    stopped_boundary: Option<u64>,
    baseline_response: Option<String>,
    screen_boundary: u64,
    output_observed: Instant,
    active_screen: bool,
    interrupt_request: Option<(u64, Instant, usize)>,
    acknowledged: Option<Instant>,
    ack_count: usize,
}
impl Monitor {
    pub fn start(directory: &Path, fixture: bool, resume: Option<&str>) -> io::Result<Self> {
        let token = uuid::Uuid::new_v4().to_string();
        let session = resume
            .map(str::to_owned)
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        // Short name stays below macOS's 104-byte Unix address bound in default state dir.
        let path = directory.join("status.sock");
        if let Ok(metadata) = fs::symlink_metadata(&path) {
            if !metadata.file_type().is_socket() {
                return Err(io::Error::other("Status endpoint is not a socket"));
            }
            fs::remove_file(&path)?;
        }
        let socket = UnixDatagram::bind(&path)?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
        socket.set_nonblocking(true)?;
        let settings = directory.join("status-settings.json");
        let command = [
            std::env::current_exe()?.into_os_string(),
            "--status-hook".into(),
            path.clone().into_os_string(),
            token.clone().into(),
            session.clone().into(),
        ]
        .iter()
        .map(|part| {
            let text = part.to_str().ok_or_else(|| {
                io::Error::other("Status helper requires UTF-8 executable/state path")
            })?;
            Ok(format!("'{}'", text.replace('\'', "'\\''")))
        })
        .collect::<io::Result<Vec<_>>>()?
        .join(" ");
        let events = [
            "SessionStart",
            "SessionEnd",
            "UserPromptSubmit",
            "PreToolUse",
            "PermissionRequest",
            "PostToolUse",
            "PostToolUseFailure",
            "PostToolBatch",
            "Notification",
            "Stop",
            "StopFailure",
            "ConfigChange",
        ];
        let hooks: serde_json::Map<String, serde_json::Value> = events.into_iter().map(|event| (event.into(), serde_json::json!([{"hooks":[{"type":"command","command":command,"timeout":1}]}]))).collect();
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&settings)?;
        file.write_all(&serde_json::to_vec(&serde_json::json!({"hooks":hooks}))?)?;
        Ok(Self {
            socket,
            path,
            settings,
            token,
            session,
            fixture,
            status: Status::default(),
            received: None,
            pending: Vec::new(),
            supported_screen: false,
            supported_executable: false,
            started: fixture,
            input_boundary: None,
            barrier: millis(),
            turn: None,
            turn_at: 0,
            prompt: String::new(),
            baseline_marker: None,
            stopped_boundary: None,
            baseline_response: None,
            screen_boundary: 0,
            output_observed: Instant::now(),
            active_screen: false,
            interrupt_request: None,
            acknowledged: None,
            ack_count: 0,
        })
    }
    pub fn native_executable(&mut self, executable: Option<&Path>) {
        self.supported_executable =
            executable.is_some_and(|path| path.file_name().is_some_and(|name| name == "2.1.318"));
    }
    pub fn native_session(&self) -> &str {
        &self.session
    }

    pub fn configure(&self, command: &mut portable_pty::CommandBuilder, resume: bool) {
        if self.fixture {
            command.env("RAT_STATUS_CHANNEL", &self.path);
            command.env("RAT_STATUS_TOKEN", &self.token);
            command.env("RAT_STATUS_SESSION", &self.session);
        } else {
            if !resume {
                command.arg("--session-id");
                command.arg(&self.session);
            }
            command.arg("--settings");
            command.arg(&self.settings);
        }
    }
    pub fn input(&mut self, boundary: u64, interrupt: bool) {
        self.interrupt_request = (interrupt && self.active_screen && self.turn.is_some())
            .then(|| (boundary, Instant::now(), self.ack_count));
        self.acknowledged = None;
        if !self.fixture || interrupt {
            self.input_boundary = Some(boundary);
            self.barrier = millis();
            self.stopped_boundary = None;
            self.status.state = State::Unknown;
        }
    }
    pub fn poll(&mut self, running: bool, content: &str, boundary: u64) -> Status {
        let mut bytes = [0; MAX_RECEIPT + 1];
        for count in 0..32 {
            match self.socket.recv(&mut bytes) {
                Ok(n) => {
                    let receipt = serde_json::from_slice::<Receipt>(&bytes[..n]);
                    match receipt {
                        Ok(receipt)
                            if n <= MAX_RECEIPT
                                && receipt.token == self.token
                                && receipt.session == self.session
                                && receipt.main_agent
                                && receipt.at <= millis()
                                && millis().saturating_sub(receipt.at) <= 30_000
                                && (receipt.event == "SessionStart"
                                    || receipt.at >= self.barrier)
                                && (matches!(
                                    receipt.event.as_str(),
                                    "SessionStart" | "UserPromptSubmit"
                                ) || receipt.at >= self.turn_at)
                                && (receipt.event == "UserPromptSubmit"
                                    || receipt.prompt_id.is_empty()
                                    || self.prompt.is_empty()
                                    || receipt.prompt_id == self.prompt)
                                && matches!(
                                    receipt.event.as_str(),
                                    "SessionStart"
                                        | "SessionEnd"
                                        | "UserPromptSubmit"
                                        | "PreToolUse"
                                        | "PermissionRequest"
                                        | "PostToolUse"
                                        | "PostToolUseFailure"
                                        | "PostToolBatch"
                                        | "Notification"
                                        | "Stop"
                                        | "StopFailure"
                                        | "ConfigChange"
                                        | "FixtureWorking"
                                        | "FixtureInput"
                                        | "FixtureApproval"
                                        | "FixtureCompleted"
                                        | "FixtureInterrupted"
                                ) =>
                        {
                            self.received = Some(Instant::now());
                            if matches!(
                                receipt.event.as_str(),
                                "PreToolUse"
                                    | "PostToolUse"
                                    | "PostToolUseFailure"
                                    | "PostToolBatch"
                            ) {
                                self.baseline_response =
                                    response_header(content).map(str::to_owned);
                                self.stopped_boundary = None;
                            }
                            match receipt.event.as_str() {
                                "SessionStart" => {
                                    self.started = true;
                                }
                                "UserPromptSubmit" => {
                                    self.pending.clear();
                                    self.turn = Some(Instant::now());
                                    self.turn_at = receipt.at;
                                    self.prompt = receipt.prompt_id.clone();
                                    self.acknowledged = None;
                                    self.interrupt_request = None;
                                    self.baseline_marker =
                                        done_marker(content).map(|(_, marker)| marker.to_owned());
                                    self.baseline_response =
                                        response_header(content).map(str::to_owned);
                                    self.stopped_boundary = None;
                                }
                                "Stop" if self.turn.is_some() => {
                                    self.pending.clear();
                                    self.stopped_boundary = Some(boundary);
                                }
                                "SessionEnd" | "ConfigChange" | "StopFailure" => {
                                    self.pending.clear();
                                    self.turn = None;
                                    self.stopped_boundary = None;
                                    self.acknowledged = None;
                                    self.interrupt_request = None;
                                }
                                "PreToolUse"
                                    if self.started
                                        && !receipt.tool_id.is_empty()
                                        && self.pending.len() < 16 =>
                                {
                                    if self.pending.iter().any(|tool| tool.id == receipt.tool_id) {
                                        self.pending.clear();
                                    } else {
                                        self.pending.push(PendingTool {
                                            tool: receipt.tool.clone(),
                                            id: receipt.tool_id.clone(),
                                            prompt: receipt.prompt_id.clone(),
                                            permission: false,
                                            observed: Instant::now(),
                                        });
                                    }
                                }
                                "PermissionRequest" => {
                                    let matching: Vec<_> = self
                                        .pending
                                        .iter_mut()
                                        .filter(|tool| {
                                            tool.tool == receipt.tool
                                                && tool.prompt == receipt.prompt_id
                                        })
                                        .collect();
                                    if matching.len() == 1 {
                                        matching.into_iter().next().unwrap().permission = true;
                                    } else {
                                        self.pending.clear();
                                    }
                                }
                                "PostToolUse" | "PostToolUseFailure" => {
                                    if receipt.tool_id.is_empty() {
                                        self.pending.clear();
                                    } else {
                                        self.pending.retain(|tool| tool.id != receipt.tool_id);
                                    }
                                }
                                event if event.starts_with("Fixture") => self.pending.clear(),
                                _ => {}
                            }

                            if self.status.recent.len() == 16 {
                                self.status.recent.remove(0);
                            }
                            self.status.recent.push(format!(
                                "{}:{}:{}",
                                receipt.event, receipt.tool, receipt.tool_id
                            ));
                            self.status.state = if self.fixture {
                                match receipt.event.as_str() {
                                    "FixtureWorking" => State::Working,
                                    "FixtureInput" => State::Input,
                                    "FixtureApproval" => State::Approval,
                                    "FixtureCompleted" => State::Completed,
                                    "FixtureInterrupted" => State::Interrupted,
                                    _ => State::Unknown,
                                }
                            } else {
                                State::Unknown
                            };
                            self.status.evidence = match receipt.event.as_str() {
                                "SessionStart" => Some("native session started"),
                                "UserPromptSubmit" => {
                                    Some("prompt submitted (processing unconfirmed)")
                                }
                                "PreToolUse" if receipt.tool == "AskUserQuestion" => {
                                    Some("question tool requested (dialog unconfirmed)")
                                }
                                "PreToolUse" => Some("tool requested"),
                                "PermissionRequest" => {
                                    Some("permission requested (pending unconfirmed)")
                                }
                                "PostToolUse" => Some("tool returned"),
                                "PostToolUseFailure" => {
                                    Some("tool failed (session outcome unknown)")
                                }
                                "PostToolBatch" => Some("tool batch returned"),
                                "Stop" => Some("response stop observed (continuation unconfirmed)"),
                                "StopFailure" => Some("native API failure observed"),
                                "SessionEnd" => Some("native session ended"),
                                "Notification" => Some("native notification observed"),
                                "ConfigChange" => Some("configuration changed"),
                                event if self.fixture && event.starts_with("Fixture") => {
                                    Some("scripted fixture evidence")
                                }
                                _ => None,
                            }
                            .map(str::to_owned);
                            self.status.reason = if self.fixture {
                                "fixture evidence"
                            } else {
                                "native observation unclassified"
                            }
                            .into();
                        }
                        _ => {
                            self.status = Status {
                                reason: "invalid or unattributed evidence".into(),
                                ..Status::default()
                            };
                            self.received = None;
                            self.pending.clear();
                            self.turn = None;
                            self.stopped_boundary = None;
                            self.acknowledged = None;
                            self.interrupt_request = None;
                        }
                    }
                    if count == 31 {
                        self.turn = None;
                        self.stopped_boundary = None;
                        self.acknowledged = None;
                        self.interrupt_request = None;
                        self.pending.clear();
                        self.status.state = State::Unknown;
                        self.status.reason = "evidence backlog; ordering uncertain".into();
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(_) => {
                    self.turn = None;
                    self.stopped_boundary = None;
                    self.acknowledged = None;
                    self.interrupt_request = None;
                    self.pending.clear();
                    self.status.state = State::Unknown;
                    self.status.reason = "evidence channel unavailable".into();
                    break;
                }
            }
        }
        if self
            .received
            .is_some_and(|received| received.elapsed() > FRESH)
        {
            self.status.state = State::Unknown;
            self.status.reason = "evidence stale".into();
        }
        // Version-specific native modal evidence is corroborated by an open main-agent
        // tool call. A delayed generic Notification does not close that interval.
        // No prose-only classification; no other CLI version inherits these shapes.
        if self.started
            && content
                .lines()
                .take(4)
                .any(|line| line.contains("Claude Code v2.1.318"))
        {
            self.supported_screen = true;
        }
        if (!self.fixture || !self.pending.is_empty()) && self.started {
            self.status.state = State::Unknown;
            let fresh: Vec<_> = self
                .pending
                .iter()
                .filter(|tool| tool.observed.elapsed() <= Duration::from_secs(30))
                .collect();
            let lines: Vec<_> = content.lines().map(str::trim).collect();
            let footer = lines
                .iter()
                .rev()
                .find(|line| !line.is_empty())
                .copied()
                .unwrap_or_default();
            let modal_border = lines
                .iter()
                .filter(|line| line.starts_with("────────────────────"))
                .count()
                >= 1;
            let current_output = self.input_boundary.is_none_or(|input| boundary > input);
            if (self.fixture || (self.supported_screen && self.supported_executable))
                && current_output
                && fresh.len() == 1
                && modal_border
            {
                let tool = fresh[0];
                if tool.tool == "AskUserQuestion"
                    && footer == "Enter to select · ↑/↓ to navigate · Esc to cancel"
                    && lines.iter().any(|line| line.starts_with("☐ "))
                    && lines.iter().any(|line| line.starts_with("❯ 1."))
                {
                    self.status.state = State::Input;
                    self.status.evidence =
                        Some("open question tool + current Claude 2.1.318 modal".into());
                    self.status.reason = "30s bounded native tool/modal evidence".into();
                }
                if tool.tool == "Bash"
                    && tool.permission
                    && footer == "Esc to cancel · Tab to amend"
                    && lines.contains(&"Bash command")
                    && lines.contains(&"Do you want to proceed?")
                    && lines.contains(&"❯ 1. Yes")
                {
                    self.status.state = State::Approval;
                    self.status.evidence = Some(
                        "matched permission request + current Claude 2.1.318 Bash modal".into(),
                    );
                    self.status.reason = "30s bounded native tool/modal evidence".into();
                }
            }
        }
        if self.screen_boundary != boundary {
            self.screen_boundary = boundary;
            self.output_observed = Instant::now();
        }
        if self.turn.is_some()
            && self
                .received
                .is_some_and(|at| at.elapsed() <= Duration::from_secs(30))
            && self.output_observed.elapsed() <= Duration::from_secs(5)
            && self.pending.is_empty()
            && self.stopped_boundary.is_none()
            && self.interrupt_request.is_none()
            && self.input_boundary.is_none_or(|input| boundary > input)
            && (self.fixture || (self.supported_screen && self.supported_executable))
            && content
                .lines()
                .rfind(|line| !line.trim().is_empty())
                .is_some_and(|line| {
                    (line.trim().starts_with("⏵⏵ auto mode on")
                        || line.trim().starts_with("⏸ manual mode on"))
                        && line.ends_with(" · esc to interrupt")
                })
            && response_header(content)
                .is_some_and(|response| self.baseline_response.as_deref() != Some(response))
        {
            self.status.state = State::Working;
            self.status.evidence =
                Some("current-turn new native response + Claude 2.1.318 active footer".into());
            self.status.reason = "30s turn evidence; output freshness bounded to 5s".into();
        }
        if self.turn.is_some()
            && self
                .received
                .is_some_and(|at| at.elapsed() <= Duration::from_secs(30))
            && (self.fixture || (self.supported_screen && self.supported_executable))
            && self.stopped_boundary.is_some_and(|stop| boundary > stop)
            && let Some((marker_index, marker)) = done_marker(content)
            && self.baseline_marker.as_deref() != Some(marker)
            && idle_composer(content).is_some_and(|composer| {
                marker_index < composer
                    && content
                        .lines()
                        .skip(marker_index + 1)
                        .take(composer - marker_index - 1)
                        .all(|line| {
                            line.trim().is_empty()
                                || auxiliary_row(line.trim())
                                || line.trim().starts_with("────────────────────")
                        })
            })
        {
            self.status.state = State::Completed;
            self.status.evidence =
                Some("current-turn Stop + new Claude 2.1.318 done marker and idle composer".into());
            self.status.reason = "30s bounded native completion evidence".into();
        }
        self.active_screen = content
            .lines()
            .rfind(|line| !line.trim().is_empty())
            .is_some_and(|line| {
                (line.trim().starts_with("⏵⏵ auto mode on")
                    || line.trim().starts_with("⏸ manual mode on"))
                    && line.ends_with(" · esc to interrupt")
            });
        if (self.fixture || (self.supported_screen && self.supported_executable))
            && self
                .interrupt_request
                .is_some_and(|(request, at, previous)| {
                    boundary > request
                        && at.elapsed() <= Duration::from_secs(5)
                        && content
                            .matches("Interrupted · What should Claude do instead?")
                            .count()
                            > previous
                })
            && let Some(composer) = idle_composer(content)
            && content
                .lines()
                .take(composer)
                .map(str::trim)
                .filter(|line| {
                    !line.is_empty()
                        && !auxiliary_row(line)
                        && !line.starts_with("────────────────────")
                })
                .last()
                .is_some_and(|line| {
                    line.strip_prefix("⎿").is_some_and(|rest| {
                        rest.trim() == "Interrupted · What should Claude do instead?"
                    })
                })
        {
            self.interrupt_request = None;
            self.acknowledged = Some(Instant::now());
            self.pending.clear();
            self.turn = None;
            self.stopped_boundary = None;
        }
        if self
            .acknowledged
            .is_some_and(|at| at.elapsed() <= Duration::from_secs(30))
        {
            self.status.state = State::Interrupted;
            self.status.evidence = Some(
                "native Ctrl-C + newly observed Claude 2.1.318 interruption acknowledgment".into(),
            );
            self.status.reason = "30s bounded native interruption evidence".into();
        }
        self.ack_count = content
            .matches("Interrupted · What should Claude do instead?")
            .count();
        if !running {
            self.status.state = State::Unknown;
            self.status.reason = "execution not running".into();
        }
        self.status.clone()
    }
}
impl Drop for Monitor {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
        let _ = fs::remove_file(&self.settings);
    }
}

/// Passive command hook: bounded projection only, no stdout, decisions, context or policy edits.
/// The caller deliberately exits successfully even if monitoring is unavailable.
pub fn hook(path: &Path, token: String, session: String) {
    let _ = (|| -> io::Result<()> {
        let mut input = Vec::new();
        io::stdin().take(65537).read_to_end(&mut input)?;
        let json: serde_json::Value = if input.len() <= 65536 {
            serde_json::from_slice(&input).unwrap_or(serde_json::Value::Null)
        } else {
            serde_json::Value::Null
        };
        let actual = json
            .get("session_id")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let event = json
            .get("hook_event_name")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let tool = json
            .get("tool_name")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let identity = |name: &str| {
            json.get(name)
                .and_then(|value| value.as_str())
                .filter(|value| {
                    value.len() <= 128
                        && value
                            .chars()
                            .all(|ch| ch.is_ascii_alphanumeric() || "_-".contains(ch))
                })
                .unwrap_or_default()
                .to_owned()
        };
        let receipt = Receipt {
            token,
            tool_id: identity("tool_use_id"),
            prompt_id: identity("prompt_id"),
            session: if actual == session {
                session
            } else {
                String::new()
            },
            event: if event.len() <= 64 {
                event.into()
            } else {
                String::new()
            },
            tool: if matches!(tool, "AskUserQuestion" | "Bash") {
                tool.into()
            } else {
                String::new()
            },
            main_agent: json.get("agent_id").is_none_or(serde_json::Value::is_null),
            at: millis(),
        };
        send(path, &receipt)
    })();
}
fn send(path: &Path, receipt: &Receipt) -> io::Result<()> {
    let socket = UnixDatagram::unbound()?;
    socket.set_nonblocking(true)?;
    socket.send_to(&serde_json::to_vec(receipt)?, path)?;
    Ok(())
}
pub fn fixture(event: &str) {
    let (Ok(path), Ok(token), Ok(session)) = (
        std::env::var("RAT_STATUS_CHANNEL"),
        std::env::var("RAT_STATUS_TOKEN"),
        std::env::var("RAT_STATUS_SESSION"),
    ) else {
        return;
    };
    let receipt = Receipt {
        token,
        tool_id: if event == "FixtureNativeBashPre" {
            "fixture-bash".into()
        } else if event == "PreToolUse" || event == "PostToolUse" {
            "fixture-tool".into()
        } else {
            String::new()
        },
        prompt_id: "fixture-prompt".into(),
        session: if event == "FixtureWrongSession" {
            "wrong-session".into()
        } else {
            session
        },
        event: match event {
            "FixtureNativeBashPre" => "PreToolUse",
            "FixtureNativeBashPermission" => "PermissionRequest",
            "FixtureOldStop" | "FixturePreviousTurnStop" => "Stop",
            _ => event,
        }
        .into(),
        tool: if event.starts_with("FixtureNativeBash") {
            "Bash".into()
        } else if event == "PreToolUse" || event == "PostToolUse" {
            "AskUserQuestion".into()
        } else {
            String::new()
        },
        main_agent: true,
        at: if event == "FixtureOldStop" {
            millis().saturating_sub(31_000)
        } else if event == "FixturePreviousTurnStop" {
            millis().saturating_sub(1_000)
        } else {
            millis()
        },
    };
    let _ = send(Path::new(&path), &receipt);
}

fn millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}
fn idle_composer(screen: &str) -> Option<usize> {
    let lines: Vec<_> = screen.lines().map(str::trim).collect();
    let footer = lines.iter().rfind(|line| !line.is_empty())?;
    if !((footer.starts_with("⏵⏵ auto mode on") || footer.starts_with("⏸ manual mode on"))
        && !footer.contains("esc to interrupt")
        && !footer.contains("Esc to cancel"))
    {
        return None;
    }
    let composer = lines.iter().rposition(|line| *line == "❯")?;
    (composer > 0
        && lines[composer - 1].starts_with("────────────────────")
        && lines
            .get(composer + 1)
            .is_some_and(|line| line.starts_with("────────────────────")))
    .then_some(composer)
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
            let rest = line.strip_prefix("✻ ")?;
            let (verb, rest) = rest.split_once(" for ")?;
            let (duration, clock) = rest.split_once(" · done ")?;
            let (clock, suffix) = clock.rsplit_once(' ')?;
            (verb.len() <= 32
                && !verb.is_empty()
                && verb.chars().all(char::is_alphabetic)
                && duration.len() <= 32
                && !duration.is_empty()
                && duration
                    .chars()
                    .all(|ch| ch.is_ascii_digit() || " smh".contains(ch))
                && matches!(suffix, "AM" | "PM")
                && (4..=5).contains(&clock.len())
                && clock.contains(':')
                && clock.chars().all(|ch| ch.is_ascii_digit() || ch == ':'))
            .then_some((index, line))
        })
}

fn response_header(screen: &str) -> Option<&str> {
    screen
        .lines()
        .rev()
        .map(str::trim)
        .find(|line| line.starts_with("⏺ ") && line.len() > "⏺ ".len())
}

fn auxiliary_row(line: &str) -> bool {
    matches!(
        line,
        "✔ Update installed · Restart to update"
            | "◐ medium · /effort · ✔ Update installed · Restart to update"
    )
}
