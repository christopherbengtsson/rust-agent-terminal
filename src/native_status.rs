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
    time::{Duration, Instant},
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
}
impl Monitor {
    pub fn start(directory: &Path, fixture: bool) -> io::Result<Self> {
        let token = uuid::Uuid::new_v4().to_string();
        let session = uuid::Uuid::new_v4().to_string();
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
        })
    }
    pub fn native_executable(&mut self, executable: Option<&Path>) {
        self.supported_executable =
            executable.is_some_and(|path| path.file_name().is_some_and(|name| name == "2.1.318"));
    }
    pub fn configure(&self, command: &mut portable_pty::CommandBuilder) {
        if self.fixture {
            command.env("RAT_STATUS_CHANNEL", &self.path);
            command.env("RAT_STATUS_TOKEN", &self.token);
            command.env("RAT_STATUS_SESSION", &self.session);
        } else {
            command.arg("--session-id");
            command.arg(&self.session);
            command.arg("--settings");
            command.arg(&self.settings);
        }
    }
    pub fn input(&mut self, boundary: u64) {
        if !self.fixture {
            self.input_boundary = Some(boundary);
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
                            match receipt.event.as_str() {
                                "SessionStart" => {
                                    self.started = true;
                                }
                                "UserPromptSubmit" | "SessionEnd" | "ConfigChange" => {
                                    self.pending.clear()
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
                                "native status support gate open"
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
                        }
                    }
                    if count == 31 {
                        self.pending.clear();
                        self.status.state = State::Unknown;
                        self.status.reason = "evidence backlog; ordering uncertain".into();
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(_) => {
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
                    self.status.reason =
                        "30s bounded evidence; native support gate remains open".into();
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
                    self.status.reason =
                        "30s bounded evidence; native support gate remains open".into();
                }
            }
        }
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
    };
    let _ = send(Path::new(&path), &receipt);
}
