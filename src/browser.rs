//! Dashboard-only retained-history selection. Reading never selects a runtime.
use crate::{
    events::{Effect, Request},
    persistence::search::{Mode, Query, Response},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
#[derive(Clone)]
pub struct Browser {
    pub mode: Mode,
    pub text: String,
    pub editing: bool,
    pub selected: Option<String>,
    pub response: Option<Response>,
    pub request: String,
    pub workspace: Option<String>,
    pub scroll: u16,
    pub opened: Option<crate::history::ReadablePage>,
}
impl Browser {
    pub fn new(mode: Mode, workspace: Option<String>) -> Self {
        Self {
            mode,
            text: String::new(),
            editing: mode != Mode::Sessions,
            selected: None,
            response: None,
            request: String::new(),
            workspace,
            scroll: 0,
            opened: None,
        }
    }
    pub fn query(&mut self, after: i64) -> Effect {
        let mut query = Query::new(self.mode, &self.text);
        query.after = after;
        query.workspace = self.workspace.clone();
        self.request = query.request.clone();
        self.response = None;
        self.opened = None;
        Effect::Session(Request::Browse(query))
    }
    pub fn observe(&mut self, response: Option<&Response>) {
        if let Some(response) = response
            && response.request == self.request
        {
            // A missing selected UUID stays unavailable; never silently choose its neighbour.
            if self.selected.is_none() {
                self.selected = response.rows.first().map(|row| row.key.clone());
            }
            if response.error.is_some() {
                self.opened = None;
            }
            if let Some(opened) = &response.opened {
                self.opened = Some(opened.clone());
            }
            self.response = Some(response.clone());
        }
    }
    pub fn changed(&mut self) -> Effect {
        let mut query = Query::new(self.mode, "");
        query.cancel = true;
        self.request = query.request.clone();
        self.response = None;
        self.editing = true;
        Effect::Session(Request::Browse(query))
    }
    pub fn key(&mut self, key: KeyEvent) -> Option<Effect> {
        if key.modifiers == KeyModifiers::CONTROL && key.code == KeyCode::Char('r') {
            return Some(self.query(0));
        }
        if key.code == KeyCode::Char('v') && key.modifiers.is_empty() && !self.editing {
            let row = self.response.as_ref()?.rows.iter()
                .find(|row| Some(&row.key) == self.selected.as_ref())?;
            return Some(Effect::Session(Request::ResumeSelected {
                request: uuid::Uuid::new_v4().to_string(),
                workspace: row.workspace.clone(), session: row.session.clone()?,
            }));
        }
        if self.opened.is_some() {
            match key.code {
                KeyCode::Down | KeyCode::Char('j') => self.scroll = self.scroll.saturating_add(1),
                KeyCode::Up | KeyCode::Char('k') => self.scroll = self.scroll.saturating_sub(1),
                KeyCode::PageDown => self.scroll = self.scroll.saturating_add(8),
                KeyCode::PageUp => self.scroll = self.scroll.saturating_sub(8),
                _ => {}
            }
            return None;
        }
        if key.code == KeyCode::Tab {
            self.mode = match self.mode {
                Mode::Workspaces => Mode::Sessions,
                Mode::Sessions => Mode::History,
                Mode::History => Mode::Workspaces,
            };
            self.editing = self.mode != Mode::Sessions;
            self.selected = None;
            self.workspace = None;
            return if self.mode == Mode::History {
                Some(self.changed())
            } else {
                Some(self.query(0))
            };
        }
        match key.code {
            KeyCode::Down | KeyCode::Up => {
                self.editing = false;
                if let Some(response) = &self.response {
                    let at = response
                        .rows
                        .iter()
                        .position(|r| Some(&r.key) == self.selected.as_ref());
                    let at = if let Some(at) = at {
                        if key.code == KeyCode::Down {
                            (at + 1).min(response.rows.len().saturating_sub(1))
                        } else {
                            at.saturating_sub(1)
                        }
                    } else {
                        0
                    };
                    self.selected = response.rows.get(at).map(|r| r.key.clone());
                }
            }
            KeyCode::Enter if self.editing => {
                self.editing = false;
                self.selected = None;
                return Some(self.query(0));
            }
            KeyCode::Enter => {
                let row = self
                    .response
                    .as_ref()?
                    .rows
                    .iter()
                    .find(|row| Some(&row.key) == self.selected.as_ref())?
                    .clone();
                if let Some(passage) = row.passage {
                    let request = uuid::Uuid::new_v4().to_string();
                    let query = Query::open(&request, passage);
                    self.request = request;
                    self.scroll = 0;
                    return Some(Effect::Session(Request::Browse(query)));
                }
                if let Some(session) = row.session {
                    // Existing stable-UUID readable page path; never native Resume.
                    return Some(Effect::Session(Request::HistorySession {
                        session,
                        offset: 0,
                    }));
                }
                self.mode = Mode::Sessions;
                self.workspace = Some(row.workspace);
                self.text.clear();
                self.selected = None;
                return Some(self.query(0));
            }
            KeyCode::Char('n') if !self.editing => {
                if let Some(response) = &self.response
                    && response.more
                {
                    let after = response.after;
                    self.selected = None;
                    return Some(self.query(after));
                }
            }
            KeyCode::Char('/') if !self.editing => return Some(self.changed()),
            KeyCode::Backspace => {
                self.editing = true;
                self.text.pop();
                return Some(self.changed());
            }
            KeyCode::Char(c) if self.text.len() + c.len_utf8() <= 256 => {
                self.editing = true;
                self.text.push(c);
                return Some(self.changed());
            }
            _ => {}
        }
        None
    }
}
