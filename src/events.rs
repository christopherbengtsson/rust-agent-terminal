use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::git::Worktree;

pub enum Event {
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
