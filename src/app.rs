use std::path::PathBuf;

use crate::{
    events::{Action, Event},
    git::Worktree,
    session::{Observation, Request},
};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

pub enum Effect {
    Refresh,
    Session(Request),
}

pub struct App {
    pub session: crate::session::Observation,
    pub attached: bool,
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
            launch_request: uuid::Uuid::new_v4().to_string(),
            repository,
            worktrees: Vec::new(),
            selected: None,
            loading: false,
            error: None,
        }
    }

    pub fn observe_session(&mut self, observation: Observation) {
        if observation.identity.is_some() && self.session.identity.is_none() {
            self.attached = true;
        }
        self.session = observation;
    }

    pub fn input(&mut self, key: KeyEvent) -> Option<Effect> {
        if key.kind == KeyEventKind::Release {
            return None;
        }
        if self.attached {
            // Legacy terminals encode Ctrl-] as 0x1d, which Crossterm names Ctrl-5.
            if key.modifiers.contains(KeyModifiers::CONTROL)
                && matches!(key.code, KeyCode::Char(']') | KeyCode::Char('5'))
            {
                self.attached = false;
                return None;
            }
            let bytes = match key.code {
                KeyCode::Char(c) if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    vec![(c as u8) & 0x1f]
                }
                KeyCode::Char(c) => c.to_string().into_bytes(),
                KeyCode::Enter => vec![b'\r'],
                KeyCode::Backspace => vec![127],
                KeyCode::Tab => vec![b'\t'],
                KeyCode::Esc => vec![27],
                _ => return None,
            };
            return Some(Effect::Session(Request::Input(bytes)));
        }
        match key.code {
            KeyCode::Char('a') => {
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
            KeyCode::Char('l') => {
                return Some(Effect::Session(Request::Launch(
                    self.launch_request.clone(),
                )));
            }
            KeyCode::Enter if self.session.identity.is_some() => {
                self.attached = true;
                return None;
            }
            _ => {}
        }
        match crate::events::action_for_key(key) {
            Some(Action::Next) => self.next(),
            Some(Action::Previous) => self.previous(),
            Some(Action::Refresh) => return Some(Effect::Refresh),
            Some(Action::Quit) => return Some(Effect::Session(Request::Shutdown)),
            None => {}
        }
        None
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
