use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use serde::{Deserialize, Serialize};

use crate::git::Worktree;

pub enum Event {
    DiscoveryCompleted {
        request: u64,
        repository: std::path::PathBuf,
        result: Result<Vec<Worktree>, String>,
    },
    WorktreesLoaded(Result<Vec<Worktree>, String>),
}

#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    Next,
    Previous,
    Refresh,
    Quit,
}

pub fn action_for_key(key: KeyEvent) -> Option<Action> {
    if key.kind == KeyEventKind::Release {
        return None;
    }
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return (key.code == KeyCode::Char('c')).then_some(Action::Quit);
    }
    if key
        .modifiers
        .intersects(KeyModifiers::ALT | KeyModifiers::SUPER)
    {
        return None;
    }
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => Some(Action::Next),
        KeyCode::Char('k') | KeyCode::Up => Some(Action::Previous),
        KeyCode::Char('r') => Some(Action::Refresh),
        KeyCode::Char('q') | KeyCode::Esc => Some(Action::Quit),
        _ => None,
    }
}

pub enum Effect {
    Refresh,
    Detach,
    Session(Request),
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum NativeAgent {
    Claude,
    Codex,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Request {
    EditorSource {
        request: String,
        workspace: String,
        path: Option<Vec<u8>>,
        line: usize,
    },
    InspectWorking {
        #[serde(default)]
        editor: bool,
        #[serde(default)]
        whole: bool,
        #[serde(default)]
        base: Option<Vec<u8>>,
        request: String,
        workspace: String,
        selected: Option<Vec<u8>>,
    },
    /// Freeze a runtime target when an action is queued. Never resolve it from later selection.
    Session {
        target: String,
        #[serde(default)]
        session: Option<String>,
        command: Box<Request>,
    },
    PreviewCreation {
        project: String,
    },
    CreateWorkspace(crate::creation::Intent),
    ConfigureSetup {
        project: String,
        command: Option<String>,
    },
    RecoverSetup {
        attempt: String,
        retry: bool,
        #[serde(default)]
        informed: bool,
    },
    RecoverCreation {
        request: String,
        local: bool,
    },
    CancelCreation {
        request: String,
        #[serde(default)]
        native_session: Option<String>,
    },
    SelectSession(String),
    Discover {
        request: u64,
        repository: Vec<u8>,
    },
    Adopt(Vec<u8>),
    Register(Vec<u8>),
    Launch(String),
    LaunchClaude(String),
    LaunchCodex(String),
    LaunchNative {
        request: String,
        agent: NativeAgent,
        prompt: Option<String>,
    },
    RenameWorkspace {
        workspace: String,
        name: String,
    },
    Resume(String),
    QueryLaunch(String),
    Reconcile,
    History {
        index: usize,
        offset: u64,
    },
    HistorySession {
        session: String,
        offset: u64,
    },
    Input(Vec<u8>),
    Key(crossterm::event::KeyEvent),
    Paste(String),
    Screen,
    Stop,
    RetryStorage,
    Cleanup {
        page: usize,
    },
    DeleteHistory {
        id: String,
    },
    Resize {
        rows: u16,
        cols: u16,
    },
    Shutdown,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Lifecycle {
    #[default]
    Idle,
    Running,
    Stopping,
    Exited,
    Failed,
    Stopped,
    StopFailed,
    Interrupted,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LaunchResult {
    pub request: String,
    pub identity: Option<String>,
    pub error: Option<String>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Observation {
    #[serde(default)]
    pub setup_protocol: u32,
    #[serde(default)]
    pub setup: crate::setup::View,
    #[serde(default)]
    pub creation_protocol: u32,
    #[serde(default)]
    pub creation: crate::creation::View,
    #[serde(default)]
    pub diff_protocol: u32,
    #[serde(default)]
    pub editor_protocol: u32,
    #[serde(default)]
    pub editor_source: Option<crate::editor::SourceResponse>,
    #[serde(default)]
    pub working_diff: Option<crate::working_diff::Response>,
    #[serde(default)]
    pub sessions_protocol: u32,
    #[serde(default)]
    pub control_target: String,
    #[serde(default)]
    pub sessions: Vec<SessionSummary>,
    #[serde(default)]
    pub registry_protocol: u32,
    #[serde(default)]
    pub prompt_naming_protocol: u32,
    #[serde(default)]
    pub registry_pending: bool,
    #[serde(default)]
    pub registry: crate::registry::Registry,
    #[serde(default)]
    pub git_jobs_protocol: u32,
    #[serde(default)]
    pub discovery: Option<crate::git::Discovery>,
    #[serde(default)]
    pub adoption_pending: bool,
    #[serde(default)]
    pub checkout_identity: Option<crate::git::CheckoutIdentity>,
    #[serde(default)]
    pub connection_lost: bool,
    #[serde(default)]
    pub capacity: crate::cleanup::Capacity,
    #[serde(default)]
    pub cleanup: Option<crate::cleanup::Page>,
    #[serde(default)]
    pub cleanup_message: Option<String>,
    #[serde(default)]
    pub capture_pending_bytes: usize,
    #[serde(default)]
    /// Current managed-session capture bytes, including producer blocks waiting
    /// for queue space. Resets for a new session and remains final after sealing;
    /// restoration recovers only its committed prefix, not lost process-local queues.
    pub accepted_bytes: u64,
    #[serde(default)]
    /// Current managed-session raw prefix written to transcript files; it may
    /// exceed the committed durable prefix during a sync/commit failure.
    pub written_bytes: u64,
    #[serde(default)]
    pub storage_wait_millis: u64,
    #[serde(default)]
    pub producer_wait_millis: u64,
    #[serde(default)]
    pub storage_error: Option<String>,
    #[serde(default)]
    pub capture_loss_possible: bool,
    #[serde(default)]
    pub capture_high_water: usize,
    #[serde(default)]
    pub capture_backpressure_count: u64,
    #[serde(default)]
    pub capture_backpressure_micros: u64,
    #[serde(default)]
    pub subscriber_frame_high_water: usize,
    #[serde(default)]
    pub execution: Option<crate::recovery::Execution>,
    #[serde(default)]
    pub reconciled: bool,
    #[serde(default)]
    pub history: Option<crate::persistence::HistoryPage>,
    #[serde(default)]
    pub history_index: usize,
    #[serde(default)]
    /// Committed raw prefix for the current managed session, directly comparable
    /// with accepted_bytes/written_bytes without a launch baseline. Only the
    /// separate in-process screen counter output_bytes needs a launch baseline.
    pub durable_bytes: u64,
    #[serde(default)]
    pub metadata_pending: bool,
    #[serde(default)]
    pub durable_flush_count: u64,
    #[serde(default)]
    pub durable_commits: std::collections::VecDeque<crate::persistence::DurableCommit>,
    #[serde(default)]
    pub durable_completed_millis: u64,
    #[serde(default)]
    pub sync_commit_micros: u64,
    #[serde(default)]
    pub accepted_to_durable_millis: u64,
    #[serde(default)]
    pub launch_request: String,
    #[serde(default)]
    pub launch_result: Option<LaunchResult>,
    pub adopted: Option<String>,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub native_conversation: Option<String>,
    pub identity: Option<String>,
    #[serde(default)]
    pub session_identity: Option<SessionIdentity>,
    #[serde(default)]
    pub process_exit: Option<ProcessExit>,
    #[serde(default)]
    pub status: crate::native_status::Status,
    pub screen: String,
    pub formatted: String,
    pub rows: u16,
    pub cols: u16,
    /// Bytes applied to this owner's in-process screen across its conversations.
    /// Reinitialized on owner restoration; baseline at launch for session comparisons.
    pub output_bytes: u64,
    #[serde(skip)]
    pub decoded: Option<vt100::Screen>,
    pub error: Option<String>,
    pub stopped: bool,
    #[serde(default)]
    pub lifecycle: Lifecycle,
    pub control_denied: bool,
}

/// Runtime and durable identity; the wire label remains a derived compatibility field.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionIdentity {
    pub task: String,
    pub session: String,
    pub request: String,
}
impl SessionIdentity {
    pub fn label(&self) -> String {
        format!(
            "task={} session={} request={}",
            self.task, self.session, self.request
        )
    }
}

/// Original native leader outcome, retained independently of cleanup/storage errors.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProcessExit {
    pub code: u32,
    pub signal: Option<String>,
}
impl ProcessExit {
    pub fn failed(&self) -> bool {
        self.code != 0 || self.signal.is_some()
    }
    pub fn reason(&self) -> String {
        match &self.signal {
            Some(signal) => format!("signal {signal}"),
            None => format!("exit code {}", self.code),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SessionSummary {
    #[serde(default)]
    pub status: crate::native_status::Status,
    #[serde(default)]
    pub reserved: bool,
    pub target: String,
    pub task: String,
    pub session: Option<String>,
    pub lifecycle: Lifecycle,
    pub checkout: Option<Vec<u8>>,
}
