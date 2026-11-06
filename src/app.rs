use std::path::PathBuf;

use crate::{
    events::{Action, Event, Observation, Request},
    git::Worktree,
};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

pub use crate::events::Effect;

pub struct App {
    pub workspaces_open: bool,
    pub workspace_selected: Option<String>,
    pub project_input: Option<String>,
    prompt_target: Option<(String, Option<String>)>,
    pub first_prompt: Option<(crate::events::NativeAgent, String)>,
    pub rename_workspace: Option<(String, String)>,
    pub session: Observation,
    selected_target: String,
    selecting_checkout: bool,
    pub attached: bool,
    pub history_open: bool,
    pub history_raw: bool,
    pub cleanup_open: bool,
    pub cleanup_selected: usize,
    pub cleanup_preview: Option<crate::cleanup::Entry>,
    pub history_scroll: u16,
    history_index: usize,
    history_session: Option<String>,
    history_offset: u64,
    pub shutdown_pending: bool,
    detached: bool,
    pub launch_request: String,
    pub repository: PathBuf,
    pub worktrees: Vec<Worktree>,
    pub selected: Option<usize>,
    pub loading: bool,
    pub discovery_request: u64,
    pub error: Option<String>,
    sent_viewport: Option<(u16, u16)>,
    pending_viewport: Option<(u16, u16)>,
}

impl App {
    pub fn new(repository: PathBuf) -> Self {
        Self {
            workspaces_open: false,
            workspace_selected: None,
            project_input: None,
            first_prompt: None,
            prompt_target: None,
            rename_workspace: None,
            sent_viewport: None,
            pending_viewport: None,
            session: Default::default(),
            selected_target: String::new(),
            selecting_checkout: false,
            attached: false,
            history_open: false,
            history_raw: false,
            cleanup_open: false,
            cleanup_selected: 0,
            cleanup_preview: None,
            history_scroll: 0,
            history_index: 0,
            history_session: None,
            history_offset: 0,
            shutdown_pending: false,
            detached: false,
            launch_request: uuid::Uuid::new_v4().to_string(),
            repository,
            worktrees: Vec::new(),
            selected: None,
            loading: false,
            discovery_request: uuid::Uuid::new_v4().as_u128() as u64,
            error: None,
        }
    }

    /// Ui computes geometry; App decides whether a new usable size needs dispatch.
    pub fn viewport(&mut self, dimensions: Option<(u16, u16)>) -> Option<Effect> {
        let dimensions = dimensions?;
        if self.sent_viewport == Some(dimensions) {
            return None;
        }
        self.pending_viewport = Some(dimensions);
        Some(self.session_effect(Request::Resize {
            rows: dimensions.0,
            cols: dimensions.1,
        }))
    }

    pub fn viewport_dispatched(&mut self, result: Result<(), String>) {
        match result {
            Ok(()) => self.sent_viewport = self.pending_viewport.take(),
            Err(error) => {
                self.pending_viewport = None;
                self.error = Some(error);
            }
        }
    }

    fn launch_intent(&mut self, request: fn(String) -> Request) -> Effect {
        if self.session.launch_result.is_some()
            || self.session.lifecycle == crate::session::Lifecycle::Running
        {
            self.launch_request = uuid::Uuid::new_v4().to_string();
        }
        self.session_effect(request(self.launch_request.clone()))
    }

    fn session_effect(&self, request: Request) -> Effect {
        if self.selected_target.is_empty()
            || matches!(
                request,
                Request::Shutdown
                    | Request::Adopt(_)
                    | Request::Register(_)
                    | Request::SelectSession(_)
                    | Request::Discover { .. }
            )
        {
            Effect::Session(request)
        } else {
            Effect::Session(Request::Session {
                target: self.selected_target.clone(),
                session: self
                    .session
                    .session_identity
                    .as_ref()
                    .map(|id| id.session.clone()),
                command: Box::new(request),
            })
        }
    }

    pub fn observe_session(&mut self, mut observation: Observation) {
        if let Some(discovery) = observation.discovery.take() {
            use std::os::unix::ffi::OsStringExt;
            self.apply(Event::DiscoveryCompleted {
                request: discovery.request,
                repository: PathBuf::from(std::ffi::OsString::from_vec(discovery.repository)),
                result: discovery.result,
            });
        }
        if !observation.connection_lost
            && !self.selected_target.is_empty()
            && !observation.control_target.is_empty()
            && observation.control_target != self.selected_target
            && !self.selecting_checkout
        {
            return;
        }
        if self.selected_target.is_empty() || self.selecting_checkout {
            self.selected_target = observation.control_target.clone();
        }
        if self.selecting_checkout
            && !observation.adoption_pending
            && (self.session.adoption_pending || observation.error.is_some())
        {
            self.selecting_checkout = false;
        }
        if observation.connection_lost {
            let error = observation.error.take();
            observation = self.session.clone();
            observation.connection_lost = true;
            observation.error = error;
            observation.status = crate::native_status::Status::default();
            observation.status.reason =
                "dashboard disconnected; current agent state unknown".into();
            self.attached = false;
        }
        if !observation.launch_request.is_empty() {
            self.launch_request = observation.launch_request.clone();
        }
        if observation.lifecycle == crate::session::Lifecycle::Running
            && observation.identity.is_some()
            && (observation.identity != self.session.identity
                || self.session.lifecycle != crate::session::Lifecycle::Running)
        {
            self.attached = true;
        }
        if observation.stopped
            || (observation.error.is_some()
                && observation.lifecycle != crate::session::Lifecycle::Stopping)
        {
            self.shutdown_pending = false;
        }
        if observation.lifecycle != crate::session::Lifecycle::Running {
            observation.status.state = crate::native_status::State::Unknown;
            self.attached = false;
        }
        if self.session.registry_pending
            && !observation.registry_pending
            && let Some(added) = observation.registry.workspaces.iter().find(|workspace| {
                !self
                    .session
                    .registry
                    .workspaces
                    .iter()
                    .any(|known| known.id == workspace.id)
            })
        {
            self.workspace_selected = Some(added.id.clone());
        }
        if self.workspace_selected.is_none()
            || (self.session.registry.selected.is_some()
                && self.session.registry.selected != observation.registry.selected)
        {
            self.workspace_selected = observation.registry.selected.clone();
        }
        self.session = observation;
    }

    pub fn should_exit(&self) -> bool {
        self.detached || self.session.stopped
    }

    pub fn input(&mut self, key: KeyEvent) -> Option<Effect> {
        if key.kind == KeyEventKind::Release {
            return None;
        }
        if self.attached {
            // Legacy terminals encode Ctrl-] as 0x1d, which Crossterm names Ctrl-5.
            if key.modifiers == KeyModifiers::CONTROL
                && matches!(key.code, KeyCode::Char(']') | KeyCode::Char('5'))
            {
                self.attached = false;
                return None;
            }
            return Some(self.session_effect(Request::Key(key)));
        }
        if self.first_prompt.is_some() || self.rename_workspace.is_some() {
            match key.code {
                KeyCode::Esc => {
                    self.first_prompt = None;
                    self.prompt_target = None;
                    self.rename_workspace = None;
                }
                KeyCode::Enter => {
                    if let Some((agent, prompt)) = self.first_prompt.take() {
                        let Effect::Session(_) = self.launch_intent(Request::Launch) else {
                            unreachable!()
                        };
                        let command = Request::LaunchNative {
                            request: self.launch_request.clone(),
                            agent,
                            prompt: (!prompt.is_empty()).then_some(prompt),
                        };
                        let effect = match self.prompt_target.take() {
                            Some((target, session)) if !target.is_empty() => {
                                Effect::Session(Request::Session {
                                    target,
                                    session,
                                    command: Box::new(command),
                                })
                            }
                            _ => self.session_effect(command),
                        };
                        return Some(effect);
                    }
                    if let Some((workspace, name)) = self.rename_workspace.take() {
                        return Some(Effect::Session(Request::RenameWorkspace {
                            workspace,
                            name,
                        }));
                    }
                }
                KeyCode::Char('u') if key.modifiers == KeyModifiers::CONTROL => {
                    if let Some((_, text)) = self.first_prompt.as_mut() {
                        text.clear();
                    } else if let Some((_, text)) = self.rename_workspace.as_mut() {
                        text.clear();
                    }
                }
                KeyCode::Backspace => {
                    if let Some((_, text)) = self.first_prompt.as_mut() {
                        text.pop();
                    } else if let Some((_, text)) = self.rename_workspace.as_mut() {
                        text.pop();
                    }
                }
                KeyCode::Char(c)
                    if !key.modifiers.intersects(
                        KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER,
                    ) =>
                {
                    let limit = if self.first_prompt.is_some() {
                        4084
                    } else {
                        240
                    };
                    let text = if let Some((_, text)) = self.first_prompt.as_mut() {
                        text
                    } else {
                        &mut self.rename_workspace.as_mut().unwrap().1
                    };
                    if text.len() + c.len_utf8() <= limit {
                        text.push(c);
                    }
                }
                _ => {}
            }
            return None;
        }
        if let Some(path) = self.project_input.as_mut() {
            match key.code {
                KeyCode::Esc => self.project_input = None,
                KeyCode::Enter => {
                    let path = self.project_input.take().unwrap();
                    if path.is_empty() {
                        self.error = Some("Enter an existing local repository path".into());
                        return None;
                    }
                    self.workspaces_open = true;
                    return Some(self.session_effect(Request::Register(path.into_bytes())));
                }
                KeyCode::Backspace => {
                    path.pop();
                }
                KeyCode::Char(character)
                    if !key.modifiers.intersects(
                        KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER,
                    ) && path.len() < 32768 =>
                {
                    path.push(character);
                }
                _ => {}
            }
            return None;
        }
        if !key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
        {
            if matches!(key.code, KeyCode::Char('C' | 'X')) && !self.workspaces_open {
                if self.workspace_selected.is_some()
                    && self.workspace_selected != self.session.registry.selected
                {
                    self.error = Some("Adopt the selected workspace before starting".into());
                    return None;
                }
                self.prompt_target = Some((
                    self.selected_target.clone(),
                    self.session
                        .session_identity
                        .as_ref()
                        .map(|id| id.session.clone()),
                ));
                self.first_prompt = Some((
                    if key.code == KeyCode::Char('C') {
                        crate::events::NativeAgent::Claude
                    } else {
                        crate::events::NativeAgent::Codex
                    },
                    String::new(),
                ));
                return None;
            }
            if key.code == KeyCode::Char('R') {
                if let Some(workspace) = self
                    .session
                    .registry
                    .workspaces
                    .iter()
                    .find(|workspace| Some(&workspace.id) == self.workspace_selected.as_ref())
                {
                    self.rename_workspace = Some((workspace.id.clone(), workspace.name.clone()));
                }
                return None;
            }
        }
        if key.code == KeyCode::Char('A')
            && !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
        {
            self.project_input = Some(String::new());
            self.workspaces_open = true;
            return None;
        }
        if key.modifiers.is_empty() && key.code == KeyCode::Char('w') {
            self.workspaces_open = !self.workspaces_open;
            if self.workspace_selected.is_none() {
                self.workspace_selected = self.session.registry.selected.clone().or_else(|| {
                    self.session
                        .registry
                        .workspaces
                        .first()
                        .map(|workspace| workspace.id.clone())
                });
            }
            return None;
        }
        if self.workspaces_open && key.modifiers.is_empty() {
            let workspaces = &self.session.registry.workspaces;
            let index = workspaces
                .iter()
                .position(|workspace| Some(&workspace.id) == self.workspace_selected.as_ref())
                .unwrap_or(0);
            match key.code {
                KeyCode::Esc => {
                    self.workspaces_open = false;
                }
                KeyCode::Char('j')
                | KeyCode::Down
                | KeyCode::Char('k')
                | KeyCode::Up
                | KeyCode::Char(']')
                | KeyCode::Char('[') => {
                    let next = match key.code {
                        KeyCode::Char('j') | KeyCode::Down => {
                            (index + 1).min(workspaces.len().saturating_sub(1))
                        }
                        KeyCode::Char('k') | KeyCode::Up => index.saturating_sub(1),
                        KeyCode::Char(']') => ((index + 1)..workspaces.len())
                            .find(|&next| workspaces[next].project != workspaces[index].project)
                            .unwrap_or(index),
                        _ => (0..index)
                            .rev()
                            .find(|&next| workspaces[next].project != workspaces[index].project)
                            .unwrap_or(index),
                    };
                    self.workspace_selected =
                        workspaces.get(next).map(|workspace| workspace.id.clone());
                }
                KeyCode::Char('h') => {
                    let Some(session) = workspaces
                        .get(index)
                        .and_then(|workspace| workspace.sessions.last())
                    else {
                        self.error = Some("No sessions recorded for this workspace".into());
                        return None;
                    };
                    self.history_session = Some(session.clone());
                    self.history_open = true;
                    self.workspaces_open = false;
                    self.history_offset = 0;
                    self.history_scroll = 0;
                    self.session.history = None;
                    return Some(self.session_effect(Request::HistorySession {
                        session: session.clone(),
                        offset: 0,
                    }));
                }
                KeyCode::Enter | KeyCode::Char('a') => {
                    let Some(workspace) = workspaces.get(index) else {
                        self.error = Some("No retained workspace; A adds a project".into());
                        return None;
                    };
                    let Some(path) = workspace.path.as_ref() else {
                        self.error = Some("Checkout path unknown for this retained legacy workspace; history remains retained".into());
                        return None;
                    };
                    use std::os::unix::ffi::OsStringExt;
                    if key.code == KeyCode::Char('a') {
                        self.selected_target.clear();
                        self.selecting_checkout = true;
                        self.sent_viewport = None;
                        return Some(self.session_effect(Request::Adopt(path.clone())));
                    }
                    if let Some(target) = self
                        .session
                        .sessions
                        .iter()
                        .filter(|entry| entry.task == workspace.task)
                        .max_by_key(|entry| entry.reserved)
                        .map(|entry| entry.target.clone())
                    {
                        self.selected_target = target.clone();
                        self.attached = false;
                        self.sent_viewport = None;
                        self.workspaces_open = false;
                        return Some(Effect::Session(Request::SelectSession(target)));
                    }
                    let repository = PathBuf::from(std::ffi::OsString::from_vec(path.clone()));
                    if repository != self.repository {
                        self.worktrees.clear();
                        self.selected = None;
                    }
                    self.repository = repository;
                    self.loading = false;
                    self.workspaces_open = false;
                    return Some(Effect::Refresh);
                }
                KeyCode::Char('q') => {
                    self.shutdown_pending = true;
                    return Some(self.session_effect(Request::Shutdown));
                }
                KeyCode::Char('d') => {
                    self.detached = true;
                    return Some(Effect::Detach);
                }
                _ => {}
            }
            return None;
        }
        if self.history_open && key.modifiers.is_empty() && key.code == KeyCode::Char('v') {
            self.history_raw = !self.history_raw;
            self.history_scroll = 0;
            return None;
        }
        if key.modifiers.is_empty()
            && matches!(
                key.code,
                KeyCode::Char('c' | 'x' | 'l' | 'v') | KeyCode::Enter
            )
            && self.workspace_selected.is_some()
            && self.workspace_selected != self.session.registry.selected
        {
            self.error = Some("Selected workspace is not adopted; w then a explicitly adopts it before launch or attachment".into());
            return None;
        }
        if key.modifiers.is_empty() && key.code == KeyCode::Char('g') {
            self.cleanup_open = true;
            self.cleanup_preview = None;
            self.cleanup_selected = 0;
            self.history_open = false;
            self.session.cleanup = None;
            return Some(self.session_effect(Request::Cleanup { page: 0 }));
        }
        if self.cleanup_open
            && (key.modifiers.is_empty()
                || (key.code == KeyCode::Char('D') && key.modifiers == KeyModifiers::SHIFT))
        {
            match key.code {
                KeyCode::Esc => {
                    if self.cleanup_preview.take().is_none() {
                        self.cleanup_open = false;
                    }
                    return None;
                }
                KeyCode::Char('D') => {
                    if let Some(preview) = self.cleanup_preview.take() {
                        self.history_index = 0;
                        self.history_offset = 0;
                        self.history_scroll = 0;
                        self.session.history = None;
                        return Some(
                            self.session_effect(Request::DeleteHistory { id: preview.id }),
                        );
                    }
                    return None;
                }
                KeyCode::Enter if self.cleanup_preview.is_none() => {
                    if let Some(entry) = self
                        .session
                        .cleanup
                        .as_ref()
                        .and_then(|page| page.entries.get(self.cleanup_selected))
                    {
                        if entry.eligible {
                            self.cleanup_preview = Some(entry.clone());
                        } else {
                            self.error = Some(entry.status.clone());
                        }
                    }
                    return None;
                }
                KeyCode::Char('j') | KeyCode::Down if self.cleanup_preview.is_none() => {
                    self.cleanup_selected = (self.cleanup_selected + 1).min(
                        self.session
                            .cleanup
                            .as_ref()
                            .map_or(0, |page| page.entries.len().saturating_sub(1)),
                    );
                    return None;
                }
                KeyCode::Char('k') | KeyCode::Up if self.cleanup_preview.is_none() => {
                    self.cleanup_selected = self.cleanup_selected.saturating_sub(1);
                    return None;
                }
                KeyCode::Char('n' | 'p') if self.cleanup_preview.is_none() => {
                    let page = self.session.cleanup.as_ref()?;
                    let index = if key.code == KeyCode::Char('n') && page.next {
                        page.index + 1
                    } else if key.code == KeyCode::Char('p') {
                        page.index.saturating_sub(1)
                    } else {
                        return None;
                    };
                    self.cleanup_selected = 0;
                    self.session.cleanup = None;
                    return Some(self.session_effect(Request::Cleanup { page: index }));
                }
                KeyCode::Char('y' | 's' | 'q') => {}
                _ => return None,
            }
        }
        if key.modifiers.is_empty() {
            match key.code {
                KeyCode::Char('j') | KeyCode::Down if self.history_open => {
                    self.history_scroll = self.history_scroll.saturating_add(1);
                    return None;
                }
                KeyCode::Char('k') | KeyCode::Up if self.history_open => {
                    self.history_scroll = self.history_scroll.saturating_sub(1);
                    return None;
                }
                KeyCode::Char('h') => {
                    self.history_open = !self.history_open;
                    self.history_index = self.session.history_index;
                    self.history_offset = 0;
                }
                KeyCode::Char('n') if self.history_open => {
                    self.history_session = None;
                    self.history_index += 1;
                    self.history_offset = 0;
                }
                KeyCode::Char('p') if self.history_open => {
                    self.history_session = None;
                    self.history_index = self.history_index.saturating_sub(1);
                    self.history_offset = 0;
                }
                KeyCode::Char(']') if self.history_open => {
                    self.history_offset = self
                        .session
                        .history
                        .as_ref()
                        .and_then(|p| p.next)
                        .unwrap_or(self.history_offset);
                }
                KeyCode::Char('[') if self.history_open => {
                    self.history_offset =
                        self.history_offset.saturating_sub(crate::persistence::PAGE);
                }
                _ => {}
            }
            if self.history_open && matches!(key.code, KeyCode::Char('h' | 'n' | 'p' | '[' | ']')) {
                self.history_scroll = 0;
                if let Some(session) = &self.history_session {
                    return Some(self.session_effect(Request::HistorySession {
                        session: session.clone(),
                        offset: self.history_offset,
                    }));
                }
                return Some(self.session_effect(Request::History {
                    index: self.history_index,
                    offset: self.history_offset,
                }));
            }
        }
        match key.code {
            KeyCode::Char('d') if key.modifiers.is_empty() => {
                if self.shutdown_pending {
                    self.error = Some("Shutdown already requested; wait for confirmation or retry q after failure".into());
                    return None;
                }
                self.detached = true;
                return Some(Effect::Detach);
            }
            KeyCode::Char('u') if key.modifiers.is_empty() => {
                return Some(self.session_effect(Request::Reconcile));
            }
            KeyCode::Char('y') if key.modifiers.is_empty() => {
                return Some(self.session_effect(Request::RetryStorage));
            }
            KeyCode::Char('s') if key.modifiers.is_empty() => {
                return Some(self.session_effect(Request::Stop));
            }
            KeyCode::Char('a') if key.modifiers.is_empty() => {
                let tree = self.current()?;
                if tree.bare || tree.prunable.is_some() {
                    self.error = Some(
                        "Checkout unavailable or bare; refresh and choose a usable checkout".into(),
                    );
                    return None;
                }
                let path = crate::session::path_bytes(&tree.path);
                self.selected_target.clear();
                self.selecting_checkout = true;
                self.sent_viewport = None;
                return Some(self.session_effect(Request::Adopt(path)));
            }
            KeyCode::Char('v') if key.modifiers.is_empty() => {
                return Some(self.launch_intent(Request::Resume));
            }
            KeyCode::Char('c') if key.modifiers.is_empty() => {
                return Some(self.launch_intent(Request::LaunchClaude));
            }
            KeyCode::Char('x') if key.modifiers.is_empty() => {
                return Some(self.launch_intent(Request::LaunchCodex));
            }
            KeyCode::Char('l') if key.modifiers.is_empty() => {
                return Some(self.launch_intent(Request::Launch));
            }
            KeyCode::Enter if key.modifiers.is_empty() && self.session.identity.is_some() => {
                self.history_open = false;
                self.attached = true;
                return None;
            }
            _ => {}
        }
        match crate::events::action_for_key(key) {
            Some(Action::Next) => self.next(),
            Some(Action::Previous) => self.previous(),
            Some(Action::Refresh) => return Some(Effect::Refresh),
            Some(Action::Quit) => {
                self.shutdown_pending = true;
                return Some(self.session_effect(Request::Shutdown));
            }
            None => {}
        }
        None
    }

    pub fn keyboard_help(&self) -> &'static str {
        if self.first_prompt.is_some() {
            "First prompt (optional): Enter starts native agent · Esc cancels without launch"
        } else if self.rename_workspace.is_some() {
            "Workspace name: Enter saves · Ctrl-U clears · Esc cancels · no folder or conversation changes"
        } else if self.project_input.is_some() {
            "Add project: existing local path · Enter register without launch · Esc cancel"
        } else if self.workspaces_open {
            "Workspaces: j/k select · [/] project · Enter open · a adopt · R rename · h latest history · A add project · w/Esc back · d detach · q quit"
        } else if self.attached {
            "Native input · Ctrl-C interrupt · Ctrl-] / Ctrl-5 dashboard (keeps session running)"
        } else if self.cleanup_open {
            "Cleanup: j/k select · n/p page · Enter preview · D confirm selected deletion · Esc cancel/back · g refresh · y capture retry · s stop · q quit"
        } else if self.session.lifecycle == crate::session::Lifecycle::Interrupted {
            "u reconcile · v resume · g cleanup · h history · d detach · q quit/stop · j/k select · r refresh"
        } else if self.history_open {
            "q quit/stop/retry · s stop · y retry history · g cleanup · h live · n/p session · [/] page · j/k scroll · d detach"
        } else {
            "w workspaces · A add · Enter attach · x new Codex · v resume · q quit/stop · a adopt · c Claude · C/X prompt and start · R rename · h history · g cleanup · y retry · s stop · d detach · j/k · r refresh"
        }
    }

    pub fn paste(&mut self, text: String) -> Option<Effect> {
        if self.first_prompt.is_some() || self.rename_workspace.is_some() {
            let limit = if self.first_prompt.is_some() {
                4084
            } else {
                240
            };
            let entry = if let Some((_, entry)) = self.first_prompt.as_mut() {
                entry
            } else {
                &mut self.rename_workspace.as_mut().unwrap().1
            };
            if entry.len() + text.len() <= limit && !text.contains('\0') {
                entry.push_str(&text);
            } else {
                self.error = Some(format!("Input exceeds {limit}-byte limit or contains NUL"));
            }
            return None;
        }
        if let Some(path) = self.project_input.as_mut() {
            if path.len() + text.len() <= 32768 {
                path.push_str(&text);
            }
            return None;
        }
        if !self.attached {
            return None;
        }
        if text.len() > 4084 {
            self.error = Some("Paste exceeds 4084-byte limit; split it and retry".into());
            return None;
        }
        Some(self.session_effect(Request::Paste(text)))
    }

    pub fn begin_refresh(&mut self) -> bool {
        if self.loading {
            return false;
        }
        self.loading = true;
        self.discovery_request = self.discovery_request.wrapping_add(1);
        self.error = None;
        true
    }

    pub fn apply(&mut self, event: Event) {
        let result = match event {
            Event::DiscoveryCompleted {
                request,
                repository,
                result,
            } => {
                if request != self.discovery_request
                    || repository != self.repository
                    || !self.loading
                {
                    return;
                }
                result
            }
            Event::WorktreesLoaded(result) => result,
        };
        self.loading = false;
        match result {
            Ok(worktrees) => {
                let selected_path = self.current().map(|w| w.path.clone());
                self.selected = selected_path
                    .and_then(|path| worktrees.iter().position(|w| w.path == path))
                    .or_else(|| (!worktrees.is_empty()).then_some(0));
                self.worktrees = worktrees;
                self.error = None;
            }
            Err(error) => self.error = Some(error),
        }
    }

    pub fn current(&self) -> Option<&Worktree> {
        self.selected.and_then(|index| self.worktrees.get(index))
    }

    pub fn next(&mut self) {
        if !self.worktrees.is_empty() {
            self.selected = Some(self.selected.map_or(0, |i| (i + 1) % self.worktrees.len()));
        }
    }

    pub fn previous(&mut self) {
        if !self.worktrees.is_empty() {
            self.selected = Some(
                self.selected
                    .map_or(0, |i| (i + self.worktrees.len() - 1) % self.worktrees.len()),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn worktree(path: &str) -> Worktree {
        Worktree {
            path: PathBuf::from(path),
            ..Worktree::default()
        }
    }

    #[test]
    fn first_prompt_freezes_the_conversation_when_its_modal_opens() {
        let mut app = App::new("repo".into());
        let identity = |session: &str| crate::events::SessionIdentity {
            task: "task-a".into(),
            session: session.into(),
            request: "request-a".into(),
        };
        app.observe_session(Observation {
            control_target: "owner-a".into(),
            session_identity: Some(identity("conversation-a")),
            ..Observation::default()
        });
        app.input(KeyEvent::new(KeyCode::Char('C'), KeyModifiers::SHIFT));
        app.paste("Explicit first prompt".into());
        app.observe_session(Observation {
            control_target: "owner-a".into(),
            session_identity: Some(identity("replacement-conversation")),
            ..Observation::default()
        });
        assert!(
            matches!(app.input(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)), Some(Effect::Session(Request::Session { target, session: Some(session), command }))
            if target == "owner-a" && session == "conversation-a" && matches!(*command, Request::LaunchNative { .. }))
        );
    }

    #[test]
    fn workspace_enter_selects_its_running_owner_and_rejects_late_previous_screen() {
        let mut app = App::new("repo".into());
        let mut state = Observation {
            control_target: "owner-a".into(),
            ..Observation::default()
        };
        state.registry.workspaces.push(crate::registry::Workspace {
            id: "workspace-b".into(),
            task: "task-b".into(),
            legacy_tasks: vec![],
            name: "B".into(),
            project: None,
            path: Some(b"other".to_vec()),
            checkout: None,
            sessions: vec!["conversation-b".into()],
        });
        state.sessions.push(crate::events::SessionSummary {
            target: "owner-b".into(),
            task: "task-b".into(),
            session: Some("conversation-b".into()),
            lifecycle: crate::events::Lifecycle::Running,
            reserved: true,
            ..Default::default()
        });
        app.observe_session(state);
        app.input(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE));
        assert!(
            matches!(app.input(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)), Some(Effect::Session(Request::SelectSession(target))) if target == "owner-b")
        );
        app.observe_session(Observation {
            control_target: "owner-a".into(),
            screen: "late A".into(),
            ..Observation::default()
        });
        app.observe_session(Observation {
            control_target: "owner-b".into(),
            screen: "current B".into(),
            lifecycle: crate::events::Lifecycle::Running,
            identity: Some("session-b".into()),
            ..Observation::default()
        });
        assert_eq!(app.session.screen, "current B");
        assert!(app.attached);
        assert!(
            matches!(app.viewport(Some((24,100))), Some(Effect::Session(Request::Session { target, .. })) if target == "owner-b")
        );
    }

    #[test]
    fn explicit_adoption_accepts_new_owner_after_pending_old_owner_reply() {
        let mut app = App::new("repo".into());
        app.apply(Event::WorktreesLoaded(Ok(vec![worktree("other")])));
        app.observe_session(Observation {
            control_target: "old-owner".into(),
            ..Observation::default()
        });
        assert!(matches!(
            app.input(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE)),
            Some(Effect::Session(Request::Adopt(_)))
        ));
        app.observe_session(Observation {
            control_target: "old-owner".into(),
            adoption_pending: true,
            ..Observation::default()
        });
        app.observe_session(Observation {
            control_target: "new-owner".into(),
            adopted: Some("other".into()),
            ..Observation::default()
        });
        assert_eq!(app.session.control_target, "new-owner");
        assert_eq!(app.session.adopted.as_deref(), Some("other"));
        assert!(
            matches!(app.viewport(Some((24,100))), Some(Effect::Session(Request::Session { target, .. })) if target == "new-owner")
        );
    }

    #[test]
    fn first_prompt_requires_explicit_submit_and_can_be_cancelled() {
        let mut app = App::new(PathBuf::from("/repo"));
        assert!(
            app.input(KeyEvent::new(KeyCode::Char('C'), KeyModifiers::SHIFT))
                .is_none()
        );
        assert!(app.paste("Fix the unicode λ search".into()).is_none());
        assert!(
            app.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
                .is_none()
        );
        assert!(app.first_prompt.is_none());
        assert!(
            app.input(KeyEvent::new(KeyCode::Char('X'), KeyModifiers::SHIFT))
                .is_none()
        );
        app.paste("Fix the unicode λ search".into());
        assert!(
            matches!(app.input(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            Some(Effect::Session(Request::LaunchNative { agent: crate::events::NativeAgent::Codex, prompt: Some(prompt), .. })) if prompt == "Fix the unicode λ search")
        );
    }

    #[test]
    fn session_actions_freeze_owner_and_conversation_identity() {
        let mut app = App::new("repo".into());
        app.observe_session(Observation {
            control_target: "owner-a".into(),
            session_identity: Some(crate::events::SessionIdentity {
                task: "workspace-a".into(),
                session: "conversation-a".into(),
                request: "launch-a".into(),
            }),
            ..Observation::default()
        });
        assert!(
            matches!(app.viewport(Some((24, 100))), Some(Effect::Session(Request::Session { target, session: Some(session), command }))
            if target == "owner-a" && session == "conversation-a" && matches!(*command, Request::Resize { rows: 24, cols: 100 }))
        );
        assert!(
            matches!(app.input(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE)), Some(Effect::Session(Request::Session { target, session: Some(session), command }))
            if target == "owner-a" && session == "conversation-a" && matches!(*command, Request::Stop))
        );
    }

    #[test]
    fn late_projection_cannot_replace_explicitly_selected_runtime() {
        let mut app = App::new("repo".into());
        app.observe_session(Observation {
            control_target: "owner-b".into(),
            screen: "B".into(),
            ..Observation::default()
        });
        app.observe_session(Observation {
            control_target: "owner-a".into(),
            screen: "late A".into(),
            ..Observation::default()
        });
        assert_eq!(app.session.control_target, "owner-b");
        assert_eq!(app.session.screen, "B");
    }

    #[test]
    fn retained_workspace_picker_keeps_missing_selection_and_never_dispatches_launch() {
        let mut app = App::new(PathBuf::from("repo"));
        app.session
            .registry
            .workspaces
            .push(crate::registry::Workspace {
                id: "legacy-workspace".into(),
                legacy_tasks: vec!["opaque task".into()],
                task: "opaque task".into(),
                name: "Retained work".into(),
                project: None,
                path: None,
                checkout: None,
                sessions: vec!["historical-session".into()],
            });
        let key = |code| KeyEvent::new(code, KeyModifiers::NONE);
        assert!(app.input(key(KeyCode::Char('w'))).is_none());
        assert!(app.workspaces_open);
        assert_eq!(app.workspace_selected.as_deref(), Some("legacy-workspace"));
        assert!(app.input(key(KeyCode::Enter)).is_none());
        assert!(app.error.as_deref().unwrap().contains("unknown"));
        assert!(app.input(key(KeyCode::Char('l'))).is_none());
        assert!(app.session.identity.is_none());
    }

    #[test]
    fn viewport_changes_retry_after_saturation_and_ignore_unusable_geometry() {
        let mut app = App::new(PathBuf::from("."));
        assert!(matches!(
            app.viewport(Some((24, 100))),
            Some(Effect::Session(Request::Resize {
                rows: 24,
                cols: 100
            }))
        ));
        app.viewport_dispatched(Err("bounded session queue full".into()));
        assert!(app.error.as_deref().unwrap().contains("queue full"));
        assert!(matches!(
            app.viewport(Some((24, 100))),
            Some(Effect::Session(Request::Resize { .. }))
        ));
        app.viewport_dispatched(Ok(()));
        assert!(app.viewport(Some((24, 100))).is_none());
        assert!(app.viewport(None).is_none());
        assert!(app.viewport(Some((24, 100))).is_none());
        assert!(matches!(
            app.viewport(Some((32, 104))),
            Some(Effect::Session(Request::Resize {
                rows: 32,
                cols: 104
            }))
        ));
    }

    #[test]
    fn cleanup_selection_preview_cancel_and_confirmation_are_explicit_and_uuid_bound() {
        let mut app = App::new("repo".into());
        assert!(matches!(
            app.input(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE)),
            Some(Effect::Session(Request::Cleanup { page: 0 }))
        ));
        let selected = crate::cleanup::Entry {
            id: "selected-session".into(),
            eligible: true,
            ..Default::default()
        };
        app.session.cleanup = Some(crate::cleanup::Page {
            entries: vec![selected.clone()],
            ..Default::default()
        });
        assert!(
            app.input(KeyEvent::new(KeyCode::Char('D'), KeyModifiers::SHIFT))
                .is_none()
        );
        assert!(
            app.input(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
                .is_none()
        );
        assert_eq!(app.cleanup_preview.as_ref().unwrap().id, "selected-session");
        assert!(
            app.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
                .is_none()
        );
        assert!(app.cleanup_preview.is_none());
        assert!(app.cleanup_open);
        app.input(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        // A new observation or reordered page cannot retarget a reviewed preview.
        app.session.cleanup.as_mut().unwrap().entries[0].id = "different-session".into();
        assert!(
            matches!(app.input(KeyEvent::new(KeyCode::Char('D'), KeyModifiers::SHIFT)), Some(Effect::Session(Request::DeleteHistory { id })) if id == "selected-session")
        );
        app.attached = true;
        assert!(matches!(
            app.input(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE)),
            Some(Effect::Session(Request::Key(_)))
        ));
    }

    #[test]
    fn semantic_evidence_is_separate_from_process_and_terminal_prose() {
        let mut app = App::new("repo".into());
        app.observe_session(Observation {
            lifecycle: crate::session::Lifecycle::Running,
            screen: "Completed · Approve action? · Interrupted · What should Claude do instead?"
                .into(),
            ..Observation::default()
        });
        assert_eq!(
            app.session.status.state,
            crate::native_status::State::Unknown
        );
        app.observe_session(Observation {
            lifecycle: crate::session::Lifecycle::Running,
            status: crate::native_status::Status {
                state: crate::native_status::State::Working,
                ..crate::native_status::Status::default()
            },
            screen: "test result: FAILED; agent continues".into(),
            ..Observation::default()
        });
        assert_eq!(
            app.session.status.state,
            crate::native_status::State::Working
        );
        app.observe_session(Observation {
            lifecycle: crate::session::Lifecycle::Exited,
            ..Observation::default()
        });
        assert_eq!(
            app.session.status.state,
            crate::native_status::State::Unknown
        );
    }

    #[test]
    fn claude_is_an_explicit_launch_choice_separate_from_fixture() {
        let mut app = App::new("repo".into());
        assert!(
            matches!(app.input(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE)),
            Some(Effect::Session(Request::LaunchClaude(id))) if id == app.launch_request)
        );
        assert!(
            matches!(app.input(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE)),
            Some(Effect::Session(Request::Launch(id))) if id == app.launch_request)
        );
    }

    #[test]
    fn native_resume_requires_its_own_explicit_action() {
        let mut app = App::new("repo".into());
        app.observe_session(Observation {
            lifecycle: crate::session::Lifecycle::Exited,
            identity: Some("old-session".into()),
            ..Observation::default()
        });
        assert!(!app.attached);
        assert!(matches!(
            app.input(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE)),
            Some(Effect::Session(Request::Resume(_)))
        ));
        assert!(matches!(
            app.input(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE)),
            Some(Effect::Session(Request::LaunchCodex(_)))
        ));
    }

    #[test]
    fn codex_is_an_explicit_launch_choice() {
        let mut app = App::new("repo".into());
        assert!(matches!(
            app.input(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE)),
            Some(Effect::Session(Request::LaunchCodex(id))) if id == app.launch_request
        ));
    }

    #[test]
    fn quit_waits_for_confirmation_and_stop_failure_can_be_retried() {
        let mut app = App::new("repo".into());
        assert!(matches!(
            app.input(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE)),
            Some(Effect::Session(Request::Shutdown))
        ));
        assert!(app.shutdown_pending);
        app.observe_session(Observation {
            error: Some("Stop unconfirmed; retry s or q".into()),
            ..Observation::default()
        });
        assert!(!app.shutdown_pending);
        assert!(!app.should_exit());
        assert!(matches!(
            app.input(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE)),
            Some(Effect::Session(Request::Shutdown))
        ));
        app.observe_session(Observation {
            stopped: true,
            ..Observation::default()
        });
        assert!(app.should_exit());
    }

    #[test]
    fn dashboard_modifiers_do_not_adopt_launch_or_attach_and_paste_is_focus_sensitive() {
        let mut app = App::new(PathBuf::from("repo"));
        app.apply(Event::WorktreesLoaded(Ok(vec![worktree("repo")])));
        app.session.identity = Some("existing".into());
        for code in [KeyCode::Char('a'), KeyCode::Char('l'), KeyCode::Enter] {
            assert!(
                app.input(KeyEvent::new(code, KeyModifiers::CONTROL))
                    .is_none()
            );
            assert!(!app.attached);
        }
        assert!(app.paste("q\nl".into()).is_none());
        app.attached = true;
        assert!(matches!(
            app.paste("λ\ntext".into()),
            Some(Effect::Session(Request::Paste(_)))
        ));
        assert!(app.paste("x".repeat(4085)).is_none());
        assert!(app.error.as_deref().unwrap().contains("4084-byte limit"));
    }

    #[test]
    fn native_focus_preserves_shortcuts_and_returns_without_stopping() {
        let mut app = App::new(PathBuf::from("repo"));
        app.attached = true;
        for key in [
            KeyEvent::new(KeyCode::Char('λ'), KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::ALT),
            KeyEvent::new(KeyCode::Up, KeyModifiers::CONTROL),
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
            KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT),
        ] {
            assert!(
                matches!(app.input(key), Some(Effect::Session(Request::Key(forwarded))) if forwarded == key),
                "native input intercepted: {key:?}"
            );
            assert!(app.attached);
        }
        for alias in [']', '5'] {
            assert!(
                app.input(KeyEvent::new(KeyCode::Char(alias), KeyModifiers::CONTROL))
                    .is_none()
            );
            assert!(!app.attached);
            app.session.identity = Some("same session".into());
            app.input(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
            assert!(app.attached);
            assert_eq!(app.session.identity.as_deref(), Some("same session"));
        }
    }

    #[test]
    fn attributed_discovery_rejects_other_projects_and_late_requests() {
        let mut app = App::new(PathBuf::from("project-a"));
        app.apply(Event::WorktreesLoaded(Ok(vec![worktree("retained")])));
        app.begin_refresh();
        let request = app.discovery_request;
        app.apply(Event::DiscoveryCompleted {
            request,
            repository: PathBuf::from("project-b"),
            result: Ok(vec![worktree("other")]),
        });
        assert_eq!(app.current().unwrap().path, PathBuf::from("retained"));
        assert!(app.loading);
        app.apply(Event::DiscoveryCompleted {
            request: request.wrapping_sub(1),
            repository: app.repository.clone(),
            result: Ok(vec![worktree("old")]),
        });
        assert!(app.loading);
        app.apply(Event::DiscoveryCompleted {
            request,
            repository: app.repository.clone(),
            result: Err("timed out".into()),
        });
        assert!(!app.loading);
        assert_eq!(app.current().unwrap().path, PathBuf::from("retained"));
        assert_eq!(app.error.as_deref(), Some("timed out"));
        app.apply(Event::DiscoveryCompleted {
            request,
            repository: app.repository.clone(),
            result: Ok(vec![worktree("late")]),
        });
        assert_eq!(app.current().unwrap().path, PathBuf::from("retained"));
    }

    #[test]
    fn refresh_preserves_selected_path_and_failure_preserves_data() {
        let mut app = App::new(PathBuf::from("repo"));
        app.apply(Event::WorktreesLoaded(Ok(vec![
            worktree("a"),
            worktree("b"),
        ])));
        app.next();
        assert!(app.begin_refresh());
        assert!(!app.begin_refresh(), "refreshes must coalesce");
        // Navigation is still available during background work.
        app.previous();
        app.next();
        app.apply(Event::WorktreesLoaded(Ok(vec![
            worktree("b"),
            worktree("a"),
        ])));
        assert_eq!(app.selected, Some(0));
        assert_eq!(app.current().unwrap().path, PathBuf::from("b"));
        app.begin_refresh();
        app.apply(Event::WorktreesLoaded(Err("Git unavailable".into())));
        assert!(!app.loading);
        assert_eq!(app.worktrees.len(), 2);
        assert_eq!(app.selected, Some(0));
        assert_eq!(app.error.as_deref(), Some("Git unavailable"));
    }

    #[test]
    fn ownership_refusal_preserves_checkout_discovery_and_navigation() {
        let mut app = App::new(PathBuf::from("repo"));
        app.apply(Event::WorktreesLoaded(Ok(vec![
            worktree("a"),
            worktree("b"),
        ])));
        app.observe_session(Observation {
            control_denied: true,
            error: Some("Another dashboard controls this supervisor".into()),
            ..Observation::default()
        });
        assert_eq!(app.current().unwrap().path, PathBuf::from("a"));
        app.input(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        assert_eq!(app.current().unwrap().path, PathBuf::from("b"));
        assert_eq!(app.worktrees.len(), 2);
        assert!(app.session.control_denied);
        assert!(!app.attached);
    }

    #[test]
    fn disconnected_transport_preserves_attribution_and_allows_local_detach() {
        let mut app = App::new(PathBuf::from("repo"));
        app.observe_session(Observation {
            identity: Some("known-session".into()),
            lifecycle: crate::session::Lifecycle::Running,
            ..Observation::default()
        });
        app.observe_session(Observation {
            connection_lost: true,
            error: Some("press d to detach locally, then reopen".into()),
            ..Observation::default()
        });
        assert_eq!(app.session.identity.as_deref(), Some("known-session"));
        assert_eq!(app.session.lifecycle, crate::session::Lifecycle::Running);
        assert!(app.session.connection_lost);
        assert!(!app.attached);
        assert_eq!(
            app.session.status.state,
            crate::native_status::State::Unknown
        );
        assert!(matches!(
            app.input(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE)),
            Some(Effect::Detach)
        ));
        assert!(app.should_exit());
    }

    #[test]
    fn empty_and_removed_selections_are_safe() {
        let mut app = App::new(PathBuf::from("repo"));
        app.next();
        app.previous();
        assert_eq!(app.selected, None);
        app.apply(Event::WorktreesLoaded(Ok(vec![
            worktree("a"),
            worktree("b"),
        ])));
        app.previous();
        assert_eq!(app.selected, Some(1));
        app.apply(Event::WorktreesLoaded(Ok(vec![worktree("a")])));
        assert_eq!(app.selected, Some(0));
        app.apply(Event::WorktreesLoaded(Ok(vec![])));
        assert_eq!(app.selected, None);
    }
}
