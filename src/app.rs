use std::path::PathBuf;

use crate::{
    events::{Action, Event, Observation, Request},
    git::Worktree,
};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

pub use crate::events::Effect;

pub struct CreateForm {
    pub intent: crate::creation::Intent,
    pub field: usize,
    pub path: String,
    original_path: Option<(String, Vec<u8>)>,
    preview_applied: bool,
}
#[derive(PartialEq)]
struct Viewport {
    owner: String,
    session: Option<String>,
    running: bool,
    dimensions: (u16, u16),
}

/// Dashboard navigation retained across a deliberate external-editor handoff.
pub struct EditorContext {
    repository: PathBuf,
    selected: Option<usize>,
    workspace_selected: Option<String>,
    workspaces_open: bool,
    selected_target: String,
    session: Option<String>,
    attached: bool,
    history_open: bool,
    history_raw: bool,
    history_scroll: u16,
    history_index: usize,
    history_session: Option<String>,
    history_offset: u64,
    history: Option<crate::persistence::HistoryPage>,
}

pub struct App {
    pub setup_input: Option<(String, String)>,
    pub create_form: Option<CreateForm>,
    pub creation_open: bool,
    pub creation_selected: Option<String>,
    pub editor_source_request: Option<(String, String)>,
    pub editor_ready: Option<crate::editor::Target>,
    pub editor_message: Option<String>,
    pub diff: Option<crate::working_diff::View>,
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
    sent_viewport: Option<Viewport>,
    pending_viewport: Option<Viewport>,
}

impl App {
    pub fn new(repository: PathBuf) -> Self {
        Self {
            setup_input: None,
            create_form: None,
            creation_open: false,
            creation_selected: None,
            editor_source_request: None,
            editor_ready: None,
            editor_message: None,
            diff: None,
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

    pub fn editor_context(&self) -> EditorContext {
        EditorContext {
            repository: self.repository.clone(),
            selected: self.selected,
            workspace_selected: self.workspace_selected.clone(),
            workspaces_open: self.workspaces_open,
            selected_target: self.selected_target.clone(),
            session: self
                .session
                .session_identity
                .as_ref()
                .map(|id| id.session.clone()),
            attached: self.attached,
            history_open: self.history_open,
            history_raw: self.history_raw,
            history_scroll: self.history_scroll,
            history_index: self.history_index,
            history_session: self.history_session.clone(),
            history_offset: self.history_offset,
            history: self.session.history.clone(),
        }
    }

    pub fn editor_returned(&mut self, context: EditorContext) {
        self.attached = context.attached
            && !self.session.connection_lost
            && !self.session.control_denied
            && self.session.lifecycle == crate::events::Lifecycle::Running
            && self.session.control_target == context.selected_target
            && self.session.session_identity.as_ref().map(|id| &id.session)
                == context.session.as_ref();
        self.repository = context.repository;
        self.selected = context.selected;
        self.workspace_selected = context.workspace_selected;
        self.workspaces_open = context.workspaces_open;
        self.selected_target = context.selected_target;
        self.history_open = context.history_open;
        self.history_raw = context.history_raw;
        self.history_scroll = context.history_scroll;
        self.history_index = context.history_index;
        self.history_session = context.history_session;
        self.history_offset = context.history_offset;
        if self.history_open {
            self.session.history = context.history;
        }
    }

    /// Ui computes geometry; App decides whether a new usable size needs dispatch.
    pub fn viewport(&mut self, dimensions: Option<(u16, u16)>) -> Option<Effect> {
        let dimensions = dimensions?;
        if !self.selected_target.is_empty() && self.selected_target != self.session.control_target {
            return None;
        }
        let viewport = Viewport {
            owner: self.session.control_target.clone(),
            session: self
                .session
                .session_identity
                .as_ref()
                .map(|identity| identity.session.clone()),
            // Creation can finish after a resize updated only its prepared parser.
            // Reapply that geometry when the actual native PTY becomes running.
            running: self.session.lifecycle == crate::events::Lifecycle::Running,
            dimensions,
        };
        if self.sent_viewport.as_ref() == Some(&viewport) {
            return None;
        }
        self.pending_viewport = Some(viewport);
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
                    | Request::PreviewCreation { .. }
                    | Request::CreateWorkspace(_)
                    | Request::RecoverCreation { .. }
                    | Request::CancelCreation { .. }
                    | Request::SelectSession(_)
                    | Request::Discover { .. }
                    | Request::InspectWorking { .. }
                    | Request::EditorSource { .. }
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
        if let (Some(form), Some(preview)) = (&mut self.create_form, &observation.creation.preview)
            && !form.preview_applied
            && preview.project == form.intent.project
        {
            if let Some(base) = &preview.default_base {
                form.intent.base = base.clone();
            }
            form.path = crate::creation::path(&preview.managed_root)
                .join(&form.intent.request)
                .to_string_lossy()
                .into_owned();
            form.original_path = Some((
                form.path.clone(),
                crate::session::path_bytes(
                    &crate::creation::path(&preview.managed_root).join(&form.intent.request),
                ),
            ));
            form.preview_applied = true;
        }
        if let Some(response) = observation.editor_source.take()
            && self.editor_source_request.as_ref()
                == Some(&(response.request.clone(), response.workspace.clone()))
        {
            self.editor_source_request = None;
            match response.result {
                Ok((path, line)) => {
                    use std::os::unix::ffi::OsStringExt;
                    self.editor_ready = Some(crate::editor::Target::Source {
                        path: PathBuf::from(std::ffi::OsString::from_vec(path)),
                        line,
                    });
                }
                Err(error) => self.editor_message = Some(crate::working_diff::printable(&error)),
            }
        }

        if let Some(response) = observation.working_diff.take()
            && let Some(view) = &mut self.diff
        {
            let editor_ready = view.editor_request.as_ref() == Some(&response.request)
                && response.workspace == view.workspace
                && response.request == view.request;
            view.observe(response);
            if editor_ready && !view.loading {
                view.editor_request = None;
                if view.error.is_none() {
                    self.editor_ready = view
                        .snapshot
                        .clone()
                        .map(|snapshot| crate::editor::Target::Comparison(Box::new(snapshot)));
                }
            }
        }
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
        if self.selecting_checkout && self.session.creation.pending && !observation.creation.pending
        {
            self.selecting_checkout = false;
        }
        if self.selecting_checkout
            && !observation.adoption_pending
            && (self.session.adoption_pending || observation.error.is_some())
        {
            self.selecting_checkout = false;
        }
        if observation.connection_lost {
            self.editor_source_request = None;
            if let Some(view) = &mut self.diff {
                view.loading = false;
                view.error = Some(
                    "Dashboard disconnected; snapshot is stale. Detach and reconnect to refresh."
                        .into(),
                );
            }
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
            self.attached = self.diff.is_none();
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
        self.editor_message = None;
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
        if let Some((_, command)) = &mut self.setup_input {
            match key.code {
                KeyCode::Esc => self.setup_input = None,
                KeyCode::Enter => {
                    let (project, command) = self.setup_input.take().unwrap();
                    return Some(Effect::Session(Request::ConfigureSetup {
                        project,
                        command: (!command.trim().is_empty()).then_some(command),
                    }));
                }
                KeyCode::Backspace => {
                    command.pop();
                }
                KeyCode::Char('u') if key.modifiers == KeyModifiers::CONTROL => command.clear(),
                KeyCode::Char(c)
                    if !key.modifiers.intersects(
                        KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER,
                    ) && command.len() + c.len_utf8() <= 4096 =>
                {
                    command.push(c);
                }
                _ => {}
            }
            return None;
        }
        if self.create_form.is_none()
            && self.project_input.is_none()
            && self.first_prompt.is_none()
            && self.rename_workspace.is_none()
            && !self.diff.as_ref().is_some_and(|v| v.base_entry.is_some())
            && key.code == KeyCode::Char('U')
            && !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
        {
            let project = self
                .workspace_selected
                .as_ref()
                .or(self.session.registry.selected.as_ref())
                .and_then(|id| {
                    self.session
                        .registry
                        .workspaces
                        .iter()
                        .find(|w| &w.id == id)
                })
                .and_then(|w| w.project.as_ref())
                .and_then(|id| self.session.registry.projects.iter().find(|p| &p.id == id))
                .or_else(|| self.session.registry.projects.first());
            if let Some(project) = project {
                self.setup_input = Some((
                    project.id.clone(),
                    project.setup_command.clone().unwrap_or_default(),
                ));
            }
            return None;
        }
        if self.creation_open
            && self.create_form.is_none()
            && matches!(key.code, KeyCode::Char('T' | 'I' | 'Y'))
        {
            if let Some(record) = self.setup_record() {
                return Some(Effect::Session(Request::RecoverSetup {
                    attempt: record.attempt.clone(),
                    retry: key.code != KeyCode::Char('Y'),
                    informed: matches!(key.code, KeyCode::Char('I' | 'Y')),
                }));
            }
            return None;
        }
        if self.creation_open
            && self.create_form.is_none()
            && matches!(key.code, KeyCode::Char('[' | ']'))
            && key.modifiers.is_empty()
        {
            let records = &self.session.creation.records;
            if !records.is_empty() {
                let index = records
                    .iter()
                    .position(|r| Some(&r.intent.request) == self.creation_selected.as_ref())
                    .unwrap_or(records.len() - 1);
                let index = if key.code == KeyCode::Char(']') {
                    (index + 1) % records.len()
                } else {
                    (index + records.len() - 1) % records.len()
                };
                self.creation_selected = Some(records[index].intent.request.clone());
            }
            return None;
        }
        if self.create_form.is_none() && self.creation_open && key.code == KeyCode::Esc {
            self.creation_open = false;
            return None;
        }
        if key.code == KeyCode::Char('O')
            && self.create_form.is_none()
            && !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
        {
            self.creation_open = !self.creation_open;
            return None;
        }
        if let Some(form) = &mut self.create_form {
            match key.code {
                KeyCode::Esc => self.create_form = None,
                KeyCode::Tab => form.field = (form.field + 1) % 5,
                KeyCode::BackTab => form.field = (form.field + 4) % 5,
                KeyCode::Left | KeyCode::Right if form.field == 4 => {
                    form.intent.agent = match form.intent.agent {
                        crate::events::NativeAgent::Claude => crate::events::NativeAgent::Codex,
                        crate::events::NativeAgent::Codex => crate::events::NativeAgent::Claude,
                    }
                }
                KeyCode::Enter | KeyCode::Char('s')
                    if key.code == KeyCode::Enter || key.modifiers == KeyModifiers::CONTROL =>
                {
                    let mut form = self.create_form.take().unwrap();
                    form.intent.start = key.code != KeyCode::Enter;
                    form.intent.prompt = form.intent.prompt.filter(|p| !p.trim().is_empty());
                    form.intent.path = form
                        .original_path
                        .as_ref()
                        .filter(|(display, _)| display == &form.path)
                        .map(|(_, bytes)| bytes.clone())
                        .unwrap_or_else(|| form.path.as_bytes().to_vec());
                    if let Err(error) = form.intent.validate() {
                        self.error = Some(error);
                        self.create_form = Some(form);
                        return None;
                    }
                    self.selecting_checkout = true;
                    self.creation_selected = Some(form.intent.request.clone());
                    return Some(Effect::Session(Request::CreateWorkspace(form.intent)));
                }
                code => {
                    let text = match form.field {
                        0 => Some(&mut form.intent.base),
                        1 => Some(&mut form.intent.branch),
                        2 => Some(&mut form.path),
                        3 => Some(form.intent.prompt.get_or_insert_with(String::new)),
                        _ => None,
                    };
                    if let Some(text) = text {
                        match code {
                            KeyCode::Backspace => {
                                text.pop();
                            }
                            KeyCode::Char('u') if key.modifiers == KeyModifiers::CONTROL => {
                                text.clear()
                            }
                            KeyCode::Char(c)
                                if !key.modifiers.intersects(
                                    KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER,
                                ) && text.len() + c.len_utf8() <= 4084 =>
                            {
                                text.push(c)
                            }
                            _ => {}
                        }
                    }
                }
            }
            return None;
        }
        if key.code == KeyCode::Char('N')
            && !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
        {
            let workspace = self.session.registry.workspaces.iter().find(|w| {
                Some(&w.id)
                    == self
                        .workspace_selected
                        .as_ref()
                        .or(self.session.registry.selected.as_ref())
            });
            let project = workspace
                .and_then(|w| w.project.clone())
                .or_else(|| self.session.registry.projects.first().map(|p| p.id.clone()));
            if let Some(project) = project {
                let request = uuid::Uuid::new_v4().to_string();
                self.creation_open = true;
                self.create_form = Some(CreateForm {
                    intent: crate::creation::Intent {
                        branch: format!("workspace/{}", &request[..12]),
                        request,
                        project: project.clone(),
                        base: String::new(),
                        path: Vec::new(),
                        agent: crate::events::NativeAgent::Claude,
                        start: false,
                        prompt: None,
                    },
                    field: 0,
                    path: String::new(),
                    original_path: None,
                    preview_applied: false,
                });
                return Some(Effect::Session(Request::PreviewCreation { project }));
            }
            self.error = Some("Register a project with A before creating a workspace".into());
            return None;
        }
        if matches!(key.code, KeyCode::Char('L' | 'G' | 'Z'))
            && !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
        {
            if key.code == KeyCode::Char('Z') {
                let request = self
                    .creation_selected
                    .clone()
                    .or_else(|| self.session.creation.active_request.clone())
                    .or_else(|| self.creation_record().map(|r| r.intent.request.clone()))?;
                let native_session = self
                    .session
                    .creation
                    .launches
                    .iter()
                    .find(|r| r.request == request)
                    .and_then(|r| r.session.clone());
                return Some(Effect::Session(Request::CancelCreation {
                    request,
                    native_session,
                }));
            }
            self.creation_open = true;
            if let Some(request) = self
                .creation_selected
                .clone()
                .filter(|id| {
                    self.session.creation.retry_request.as_ref() == Some(id)
                        || self
                            .session
                            .creation
                            .records
                            .iter()
                            .any(|r| r.intent.request == *id)
                })
                .or_else(|| self.session.creation.retry_request.clone())
                .or_else(|| self.creation_record().map(|r| r.intent.request.clone()))
            {
                return Some(Effect::Session(Request::RecoverCreation {
                    request,
                    local: key.code == KeyCode::Char('L'),
                }));
            }
        }
        if key.code == KeyCode::Char('E')
            && !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
            && self.project_input.is_none()
            && self.first_prompt.is_none()
            && self.rename_workspace.is_none()
            && !self.diff.as_ref().is_some_and(|v| v.base_entry.is_some())
        {
            let workspace = self.session.registry.workspaces.iter().find(|w| {
                if let Some(view) = &self.diff {
                    w.id == view.workspace
                } else if self.workspaces_open {
                    Some(&w.id) == self.workspace_selected.as_ref()
                } else {
                    self.current().is_some_and(|c| {
                        w.checkout
                            .as_ref()
                            .is_some_and(|known| known.root == c.path)
                    })
                }
            });
            if let Some(workspace) = workspace
                .filter(|w| w.checkout.is_some())
                .map(|w| w.id.clone())
            {
                self.editor_message = Some("Opening LIVE workspace shared with the active agent; saves change working files".into());
                return self.source_editor(workspace, None, 1);
            }
            self.editor_message =
                Some("No existing retained workspace checkout selected; adopt it first".into());
            return None;
        }
        if let Some(view) = &mut self.diff {
            if let Some(entry) = &mut view.base_entry {
                match key.code {
                    KeyCode::Esc => view.base_entry = None,
                    KeyCode::Enter => {
                        view.base_selection = Some(entry.as_bytes().to_vec());
                        view.base_entry = None;
                        view.whole = true;
                        view.snapshot = None;
                        view.scroll = 0;
                        return Some(Effect::Session(view.refresh()));
                    }
                    KeyCode::Backspace => {
                        entry.pop();
                    }
                    KeyCode::Char('u') if key.modifiers == KeyModifiers::CONTROL => entry.clear(),
                    KeyCode::Char(c)
                        if !key
                            .modifiers
                            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                            && entry.len() + c.len_utf8() <= 32768
                            && !c.is_control() =>
                    {
                        entry.push(c)
                    }
                    _ => {}
                }
                return None;
            }
            if key.modifiers.is_empty() {
                match key.code {
                    KeyCode::Esc | KeyCode::Char('f') => {
                        self.diff = None;
                        return None;
                    }
                    KeyCode::Char('w') if !view.loading => {
                        view.whole = !view.whole;
                        view.snapshot = None;
                        view.scroll = 0;
                        view.horizontal = 0;
                        return Some(Effect::Session(view.refresh()));
                    }
                    KeyCode::Char('b') if !view.loading => {
                        view.base_entry = Some(String::new());
                        return None;
                    }
                    KeyCode::Char('e') if !view.loading => {
                        if self.session.editor_protocol != 1 {
                            self.editor_message = Some("Editor comparison requires an upgraded supervisor; detach and upgrade after stopping managed activity".into());
                            return None;
                        }
                        let mut request = view.refresh();
                        if let Request::InspectWorking { editor, .. } = &mut request {
                            *editor = true;
                        }
                        view.editor_request = Some(view.request.clone());
                        self.editor_message =
                            Some("Preparing fresh full comparison for editor…".into());
                        return Some(Effect::Session(request));
                    }
                    KeyCode::Char('o') if !view.loading => {
                        if let Some(snapshot) = &view.snapshot
                            && let Some(path) = &snapshot.selected
                        {
                            self.editor_message = Some("Opening LIVE source shared with the active agent; saving changes this workspace".into());
                            let workspace = view.workspace.clone();
                            let path = path.clone();
                            let line =
                                crate::editor::source_line(&snapshot.content, view.scroll as usize);
                            return self.source_editor(workspace, Some(path), line);
                        }
                        self.editor_message =
                            Some("No source selected; comparison remains inspectable".into());
                        return None;
                    }
                    KeyCode::Char('r') => {
                        return Some(Effect::Session(view.refresh()));
                    }
                    KeyCode::Char('j') => return view.navigate(true).map(Effect::Session),
                    KeyCode::Char('k') => return view.navigate(false).map(Effect::Session),
                    KeyCode::Down => {
                        view.scroll = view.scroll.saturating_add(1);
                        return None;
                    }
                    KeyCode::Up => {
                        view.scroll = view.scroll.saturating_sub(1);
                        return None;
                    }
                    KeyCode::PageDown => {
                        view.scroll = view.scroll.saturating_add(8);
                        return None;
                    }
                    KeyCode::PageUp => {
                        view.scroll = view.scroll.saturating_sub(8);
                        return None;
                    }
                    KeyCode::Right | KeyCode::Char('l') => {
                        view.horizontal = view.horizontal.saturating_add(8);
                        return None;
                    }
                    KeyCode::Left | KeyCode::Char('h') => {
                        view.horizontal = view.horizontal.saturating_sub(8);
                        return None;
                    }
                    KeyCode::Home => {
                        view.scroll = 0;
                        view.horizontal = 0;
                        return None;
                    }
                    KeyCode::Char('s' | 'q' | 'd' | 'y') => {} // Keep execution/storage controls available.
                    _ => return None,
                }
            } else if key.modifiers != KeyModifiers::CONTROL || key.code != KeyCode::Char('c') {
                return None;
            }
        }
        if key.modifiers.is_empty() && key.code == KeyCode::Char('f') {
            if self.session.control_denied {
                self.error = Some("Comparison unavailable without supervisor control".into());
                return None;
            }
            let workspace = self.session.registry.workspaces.iter().find(|workspace| {
                if !self.workspaces_open
                    && let Some(current) = self.current()
                {
                    workspace
                        .checkout
                        .as_ref()
                        .is_some_and(|checkout| checkout.root == current.path)
                } else {
                    Some(&workspace.id) == self.workspace_selected.as_ref()
                }
            });
            let Some(workspace) = workspace else {
                self.error = Some("Checkout has no retained workspace yet; wait for adoption metadata or explicitly adopt it before Working inspection".into());
                return None;
            };
            let mut view =
                crate::working_diff::View::new(workspace.id.clone(), workspace.name.clone());
            let request = view.refresh();
            self.diff = Some(view);
            self.workspaces_open = false;
            self.history_open = false;
            self.cleanup_open = false;
            return Some(Effect::Session(request));
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

    fn source_editor(
        &mut self,
        workspace: String,
        path: Option<Vec<u8>>,
        line: usize,
    ) -> Option<Effect> {
        if self.session.editor_protocol != 1
            || self.session.control_denied
            || self.session.connection_lost
        {
            self.editor_message = Some(
                "Live source location unavailable; reconnect to a compatible local supervisor"
                    .into(),
            );
            return None;
        }
        if self.editor_source_request.is_some() {
            self.editor_message = Some("Source location preparation already pending".into());
            return None;
        }
        let request = uuid::Uuid::new_v4().to_string();
        self.editor_source_request = Some((request.clone(), workspace.clone()));
        Some(Effect::Session(Request::EditorSource {
            request,
            workspace,
            path,
            line,
        }))
    }

    pub fn setup_record(&self) -> Option<&crate::setup::Record> {
        let request = self.creation_record().map(|r| r.intent.request.as_str());
        self.session
            .setup
            .records
            .iter()
            .rev()
            .find(|r| Some(r.creation.intent.request.as_str()) == request)
    }
    pub fn creation_record(&self) -> Option<&crate::creation::Record> {
        if let Some(request) = &self.creation_selected {
            self.session
                .creation
                .records
                .iter()
                .find(|r| r.intent.request == *request)
        } else {
            self.session.creation.records.last()
        }
    }
    pub fn keyboard_help(&self) -> &'static str {
        if self.setup_input.is_some() {
            "Project setup: /bin/sh -c command · Enter save · empty disables · Ctrl-U clear · Esc cancel"
        } else if self.create_form.is_some() {
            "New workspace: Tab fields · Left/Right agent · Enter Create without starting · Ctrl-S Create and start · Esc cancel"
        } else if self.diff.is_some() {
            "f/Esc back · s stop · q quit · e inspect ALL · o edit LIVE file · E LIVE workspace · j/k file · arrows scroll · w Working/Whole · b base · r refresh · d detach"
        } else if self.first_prompt.is_some() {
            "First prompt (optional): Enter starts native agent · Esc cancels without launch"
        } else if self.rename_workspace.is_some() {
            "Workspace name: Enter saves · Ctrl-U clears · Esc cancels · no folder or conversation changes"
        } else if self.project_input.is_some() {
            "Add project: existing local path · Enter register without launch · Esc cancel"
        } else if self.workspaces_open {
            "q quit · h latest history · N create · U setup · w/Esc back · j/k select · [/] project · Enter open · a adopt · R rename · A add · E edit LIVE · f diff · d detach"
        } else if self.attached {
            "Native input · Ctrl-C interrupt · Ctrl-] / Ctrl-5 dashboard (keeps session running)"
        } else if self.cleanup_open {
            "Cleanup: j/k select · n/p page · Enter preview · D confirm selected deletion · Esc cancel/back · g refresh · y capture retry · s stop · q quit"
        } else if self.history_open {
            "q quit/stop · s stop · h live · v raw/readable · [/] page · j/k scroll · n/p session · y retry · g cleanup · d detach"
        } else if self.session.lifecycle == crate::session::Lifecycle::Interrupted {
            "u reconcile · v resume · g cleanup · h history · d detach · q quit/stop · j/k select · r refresh"
        } else {
            "q quit/stop · s stop · h history · Enter attach · x new Codex · v resume · N create · O receipt · U setup · E LIVE · f diff · w workspaces · a adopt · d detach"
        }
    }

    pub fn paste(&mut self, text: String) -> Option<Effect> {
        if let Some((_, command)) = &mut self.setup_input {
            if command.len() + text.len() <= 4096 && !text.contains('\0') {
                command.push_str(&text);
            }
            return None;
        }
        if let Some(form) = &mut self.create_form {
            let dest = match form.field {
                0 => Some(&mut form.intent.base),
                1 => Some(&mut form.intent.branch),
                2 => Some(&mut form.path),
                3 => Some(form.intent.prompt.get_or_insert_with(String::new)),
                _ => None,
            };
            if let Some(dest) = dest {
                if dest.len() + text.len() <= 4084 && !text.contains('\0') {
                    dest.push_str(&text);
                } else {
                    self.error = Some(
                        "Creation input exceeds 4084 bytes or contains NUL; input not changed"
                            .into(),
                    );
                }
            }
            return None;
        }

        if let Some(entry) = self.diff.as_mut().and_then(|view| view.base_entry.as_mut()) {
            if entry.len() + text.len() <= 32768 && !text.chars().any(char::is_control) {
                entry.push_str(&text);
            }
            return None;
        }
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
    fn editor_return_keeps_selected_history_passage_and_diff_context_after_observation() {
        let mut app = App::new("/original/project".into());
        app.selected = Some(2);
        app.workspace_selected = Some("workspace".into());
        app.selected_target = "owner".into();
        app.session.control_target = "owner".into();
        app.session.lifecycle = crate::events::Lifecycle::Running;
        app.session.session_identity = Some(crate::events::SessionIdentity {
            task: "task".into(),
            session: "session".into(),
            request: "request".into(),
        });
        app.history_open = true;
        app.history_raw = true;
        app.history_scroll = 29;
        app.history_index = 4;
        app.history_session = Some("historical-session".into());
        app.history_offset = 16384;
        app.session.history = Some(crate::persistence::HistoryPage {
            identity: "retained-history".into(),
            session: "historical-session".into(),
            committed: 20000,
            checkout: "/original/project".into(),
            lifecycle: crate::events::Lifecycle::Exited,
            process_exit: None,
            text: "selected raw passage\\x1b".into(),
            readable: None,
            projection_preparing: false,
            offset: 16384,
            next: Some(20000),
        });
        let mut diff = crate::working_diff::View::new("workspace".into(), "name".into());
        diff.selected = Some(b"selected-file".to_vec());
        diff.scroll = 13;
        diff.horizontal = 24;
        app.diff = Some(diff);
        let context = app.editor_context();
        let mut next = app.session.clone();
        next.output_bytes = 999;
        next.screen = "agent kept running".into();
        next.history.as_mut().unwrap().text = "intervening live observation".into();
        app.observe_session(next);
        app.editor_returned(context);
        assert_eq!(app.repository, PathBuf::from("/original/project"));
        assert_eq!(app.selected, Some(2));
        assert_eq!(app.workspace_selected.as_deref(), Some("workspace"));
        assert_eq!(app.selected_target, "owner");
        assert!(app.history_open && app.history_raw);
        assert_eq!(app.history_scroll, 29);
        assert_eq!(app.history_session.as_deref(), Some("historical-session"));
        assert_eq!(app.history_index, 4);
        assert_eq!(app.history_offset, 16384);
        assert_eq!(
            app.session.history.as_ref().unwrap().text,
            "selected raw passage\\x1b"
        );
        assert_eq!(app.session.history.as_ref().unwrap().offset, 16384);
        assert_eq!(
            app.diff.as_ref().unwrap().selected.as_deref(),
            Some(b"selected-file".as_slice())
        );
        assert_eq!(app.diff.as_ref().unwrap().scroll, 13);
        assert_eq!(app.diff.as_ref().unwrap().horizontal, 24);
        assert_eq!(app.session.output_bytes, 999);
        assert!(!app.attached);
        app.history_open = false;
        app.diff = None;
        app.attached = true;
        let context = app.editor_context();
        let same = app.session.clone();
        app.observe_session(same);
        app.editor_returned(context);
        assert!(app.attached);
        app.attached = true;
        let context = app.editor_context();
        let mut ended = app.session.clone();
        ended.lifecycle = crate::events::Lifecycle::Exited;
        app.observe_session(ended);
        app.editor_returned(context);
        assert!(!app.attached);
        app.session.lifecycle = crate::events::Lifecycle::Running;
        app.attached = true;
        let context = app.editor_context();
        let mut replaced = app.session.clone();
        replaced.session_identity.as_mut().unwrap().session = "replacement".into();
        app.observe_session(replaced);
        app.editor_returned(context);
        assert!(!app.attached);
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
            creation: None,
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
        assert!(
            app.viewport(Some((24, 100))).is_none(),
            "geometry waits for the selected owner's identity"
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
        assert!(app.viewport(Some((24, 100))).is_some());
        app.viewport_dispatched(Ok(()));
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
        app.viewport_dispatched(Ok(()));
        assert!(
            matches!(app.input(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE)), Some(Effect::Session(Request::Session { target, session: None, command })) if target == "new-owner" && matches!(*command, Request::Launch(_)))
        );
    }

    #[test]
    fn viewport_acknowledgement_is_bound_to_conversation_and_execution_readiness() {
        let mut app = App::new("repo".into());
        let state = |session: Option<&str>, lifecycle| Observation {
            control_target: "owner-a".into(),
            session_identity: session.map(|session| crate::events::SessionIdentity {
                task: "workspace-a".into(),
                session: session.into(),
                request: "launch-a".into(),
            }),
            lifecycle,
            ..Observation::default()
        };
        app.observe_session(state(None, crate::events::Lifecycle::Idle));
        assert!(app.viewport(Some((32, 104))).is_some());
        app.viewport_dispatched(Ok(()));
        assert!(app.viewport(Some((32, 104))).is_none());
        app.observe_session(state(
            Some("new-conversation"),
            crate::events::Lifecycle::Interrupted,
        ));
        assert!(
            matches!(app.viewport(Some((32, 104))), Some(Effect::Session(Request::Session { target, session: Some(session), .. })) if target == "owner-a" && session == "new-conversation")
        );
        app.viewport_dispatched(Ok(()));
        app.observe_session(state(
            Some("new-conversation"),
            crate::events::Lifecycle::Running,
        ));
        assert!(
            app.viewport(Some((32, 104))).is_some(),
            "the created PTY must receive the current geometry even if preparation already accepted it"
        );
        app.viewport_dispatched(Ok(()));
        assert!(app.viewport(Some((32, 104))).is_none());
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
                creation: None,
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
    #[test]
    fn independent_creation_form_has_explicit_start_choices_and_preserves_suggested_path_bytes() {
        for start in [false, true] {
            let mut app = App::new("repo".into());
            app.session
                .registry
                .projects
                .push(crate::registry::Project {
                    setup_command: None,
                    id: "project".into(),
                    name: "My project".into(),
                    common_dir: b"/repo/.git".to_vec(),
                });
            assert!(matches!(
                app.input(KeyEvent::new(KeyCode::Char('N'), KeyModifiers::NONE)),
                Some(Effect::Session(Request::PreviewCreation { .. }))
            ));
            let mut observation = app.session.clone();
            observation.creation.preview = Some(crate::creation::Preview {
                project: "project".into(),
                default_base: Some("origin/main".into()),
                managed_root: b"/external/managed-\xff".to_vec(),
                ..Default::default()
            });
            app.observe_session(observation);
            let request = app.create_form.as_ref().unwrap().intent.request.clone();
            let action = app.input(if start {
                KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL)
            } else {
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)
            });
            let Some(Effect::Session(Request::CreateWorkspace(intent))) = action else {
                panic!("explicit creation request expected")
            };
            assert_eq!(intent.start, start);
            assert_eq!(intent.base, "origin/main");
            assert_eq!(intent.request, request);
            assert!(intent.prompt.is_none());
            assert_eq!(
                intent.path,
                [b"/external/managed-\xff/".as_slice(), request.as_bytes()].concat()
            );
        }
    }
    #[test]
    fn workspace_creation_keeps_pasted_first_prompt_as_supplied() {
        let mut app = App::new("repo".into());
        app.session
            .registry
            .projects
            .push(crate::registry::Project {
                setup_command: None,
                id: "project".into(),
                name: "Project".into(),
                common_dir: vec![],
            });
        app.input(KeyEvent::new(KeyCode::Char('N'), KeyModifiers::NONE));
        for _ in 0..3 {
            app.input(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        }
        app.paste("Fix unicode λ\nKeep the index unchanged".into());
        assert_eq!(
            app.create_form.as_ref().unwrap().intent.prompt.as_deref(),
            Some("Fix unicode λ\nKeep the index unchanged")
        );
    }
    #[test]
    fn creation_fields_consume_mixed_case_before_editor_and_other_modal_shortcuts() {
        for field in 0..4 {
            let mut app = App::new("repo".into());
            app.session.editor_protocol = 1;
            app.session
                .registry
                .projects
                .push(crate::registry::Project {
                    setup_command: None,
                    id: "project".into(),
                    name: "Project".into(),
                    common_dir: vec![],
                });
            app.input(KeyEvent::new(KeyCode::Char('N'), KeyModifiers::NONE));
            let request = app.create_form.as_ref().unwrap().intent.request.clone();
            for _ in 0..field {
                app.input(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
            }
            app.input(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
            for c in "ECXRNcamelCase".chars() {
                assert!(
                    app.input(KeyEvent::new(
                        KeyCode::Char(c),
                        if c.is_uppercase() {
                            KeyModifiers::SHIFT
                        } else {
                            KeyModifiers::NONE
                        }
                    ))
                    .is_none()
                );
            }
            let form = app.create_form.as_ref().unwrap();
            let text = match field {
                0 => form.intent.base.as_str(),
                1 => form.intent.branch.as_str(),
                2 => form.path.as_str(),
                _ => form.intent.prompt.as_deref().unwrap(),
            };
            assert_eq!(text, "ECXRNcamelCase");
            assert_eq!(form.intent.request, request);
            assert!(
                app.first_prompt.is_none()
                    && app.rename_workspace.is_none()
                    && app.editor_source_request.is_none()
                    && app.editor_ready.is_none()
                    && app.editor_message.is_none()
            );
        }
    }
    #[test]
    fn setup_command_consumes_shortcuts_paths_and_mixed_case_as_literal_text() {
        let mut app = App::new("repo".into());
        app.setup_input = Some(("project".into(), String::new()));
        let command = "ECXRNOU/SBcamelCase='x'; echo \"$PWD\"";
        for c in command.chars() {
            assert!(
                app.input(KeyEvent::new(
                    KeyCode::Char(c),
                    if c.is_ascii_uppercase() {
                        KeyModifiers::SHIFT
                    } else {
                        KeyModifiers::NONE
                    }
                ))
                .is_none()
            );
        }
        assert_eq!(app.setup_input.as_ref().unwrap().1, command);
        assert!(
            app.create_form.is_none()
                && app.first_prompt.is_none()
                && app.rename_workspace.is_none()
                && app.editor_source_request.is_none()
        );
        assert!(
            matches!(app.input(KeyEvent::new(KeyCode::Enter,KeyModifiers::NONE)),Some(Effect::Session(Request::ConfigureSetup{project,command:Some(saved)})) if project=="project" && saved==command)
        );
    }
}
