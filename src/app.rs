use std::path::PathBuf;

use crate::{
    events::{Action, Event},
    git::Worktree,
    session::{Observation, Request},
};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

pub enum Effect {
    Refresh,
    Detach,
    Session(Request),
}

pub struct App {
    pub session: crate::session::Observation,
    pub attached: bool,
    pub history_open: bool,
    pub history_scroll: u16,
    history_index: usize,
    history_offset: u64,
    pub shutdown_pending: bool,
    detached: bool,
    pub launch_request: String,
    pub repository: PathBuf,
    pub worktrees: Vec<Worktree>,
    pub selected: Option<usize>,
    pub loading: bool,
    pub error: Option<String>,
}

impl App {
    pub fn new(repository: PathBuf) -> Self {
        Self {
            session: Default::default(),
            attached: false,
            history_open: false,
            history_scroll: 0,
            history_index: 0,
            history_offset: 0,
            shutdown_pending: false,
            detached: false,
            launch_request: uuid::Uuid::new_v4().to_string(),
            repository,
            worktrees: Vec::new(),
            selected: None,
            loading: false,
            error: None,
        }
    }

    pub fn observe_session(&mut self, mut observation: Observation) {
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
        if observation.identity.is_some() && observation.identity != self.session.identity {
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
            return Some(Effect::Session(Request::Key(key)));
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
                    self.history_index += 1;
                    self.history_offset = 0;
                }
                KeyCode::Char('p') if self.history_open => {
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
                return Some(Effect::Session(Request::History {
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
                return Some(Effect::Session(Request::Reconcile));
            }
            KeyCode::Char('s') if key.modifiers.is_empty() => {
                return Some(Effect::Session(Request::Stop));
            }
            KeyCode::Char('a') if key.modifiers.is_empty() => {
                let tree = self.current()?;
                if tree.bare || tree.prunable.is_some() {
                    self.error = Some(
                        "Checkout unavailable or bare; refresh and choose a usable checkout".into(),
                    );
                    return None;
                }
                return Some(Effect::Session(Request::Adopt(crate::session::path_bytes(
                    &tree.path,
                ))));
            }
            KeyCode::Char('v') if key.modifiers.is_empty() => {
                if self.session.launch_result.is_some() {
                    self.launch_request = uuid::Uuid::new_v4().to_string();
                }
                return Some(Effect::Session(Request::Resume(
                    self.launch_request.clone(),
                )));
            }
            KeyCode::Char('c') if key.modifiers.is_empty() => {
                if self.session.launch_result.is_some() {
                    self.launch_request = uuid::Uuid::new_v4().to_string();
                }
                return Some(Effect::Session(Request::LaunchClaude(
                    self.launch_request.clone(),
                )));
            }
            KeyCode::Char('x') if key.modifiers.is_empty() => {
                if self.session.launch_result.is_some() {
                    self.launch_request = uuid::Uuid::new_v4().to_string();
                }
                return Some(Effect::Session(Request::LaunchCodex(
                    self.launch_request.clone(),
                )));
            }
            KeyCode::Char('l') if key.modifiers.is_empty() => {
                if self.session.launch_result.is_some() {
                    // An explicit new launch after a known result is a new intent.
                    self.launch_request = uuid::Uuid::new_v4().to_string();
                }
                return Some(Effect::Session(Request::Launch(
                    self.launch_request.clone(),
                )));
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
                return Some(Effect::Session(Request::Shutdown));
            }
            None => {}
        }
        None
    }

    pub fn keyboard_help(&self) -> &'static str {
        if self.attached {
            "Native input · Ctrl-C interrupt · Ctrl-] / Ctrl-5 dashboard (keeps session running)"
        } else if self.session.lifecycle == crate::session::Lifecycle::Interrupted {
            "u reconcile · v resume · h history · d detach · q quit/stop · j/k select · r refresh"
        } else if self.history_open {
            "q quit/stop/retry · s stop · h live · n/p session · [/] page · j/k scroll · d detach"
        } else {
            "q quit/stop · h history · v resume · a adopt · c Claude · x Codex · l fixture · Enter attach · s stop · d detach"
        }
    }

    pub fn paste(&mut self, text: String) -> Option<Effect> {
        if !self.attached {
            return None;
        }
        if text.len() > 4084 {
            self.error = Some("Paste exceeds 4084-byte limit; split it and retry".into());
            return None;
        }
        Some(Effect::Session(Request::Paste(text)))
    }

    pub fn begin_refresh(&mut self) -> bool {
        if self.loading {
            return false;
        }
        self.loading = true;
        self.error = None;
        true
    }

    pub fn apply(&mut self, event: Event) {
        match event {
            Event::WorktreesLoaded(result) => {
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
