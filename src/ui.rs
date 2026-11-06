use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, List, ListItem, ListState, Paragraph, Wrap},
};

use crate::app::App;

#[derive(Default)]
pub struct Ui {
    list: ListState,
}

impl Ui {
    pub fn draw(&mut self, frame: &mut Frame, app: &App) {
        frame.render_widget(
            Block::default().style(Style::default().bg(Color::Rgb(12, 17, 24)).fg(Color::Gray)),
            frame.area(),
        );
        let [body, status, help] = Layout::vertical([
            Constraint::Min(1),
            Constraint::Length(5),
            Constraint::Length(1),
        ])
        .areas(frame.area());
        let [sidebar, session] =
            Layout::horizontal([Constraint::Percentage(34), Constraint::Percentage(66)])
                .areas(body);
        let [reviews, header, list, details] = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(6),
        ])
        .areas(sidebar);
        frame.render_widget(
            Paragraph::new("Unavailable · milestone 1").block(
                Block::bordered()
                    .border_style(Style::default().fg(Color::Cyan))
                    .title(" Reviews "),
            ),
            reviews,
        );
        let title = format!(
            " Scripted PTY · Focus: {} · semantic status: unknown ",
            if app.attached { "session" } else { "dashboard" }
        );
        let text = if app.session.screen.is_empty() {
            "Adopt a checkout (a), then launch the fixture (l)."
        } else {
            &app.session.screen
        };
        frame.render_widget(
            Paragraph::new(text).block(
                Block::bordered()
                    .border_style(Style::default().fg(Color::Cyan))
                    .title(title),
            ),
            session,
        );
        frame.render_widget(
            Paragraph::new(format!(
                "Repository: {}",
                visible(&app.repository.display().to_string())
            ))
            .block(
                Block::bordered()
                    .border_style(Style::default().fg(Color::Cyan))
                    .title(" Agent terminal · Worktrees "),
            )
            .wrap(Wrap { trim: false }),
            header,
        );
        let items: Vec<_> = app
            .worktrees
            .iter()
            .map(|worktree| {
                let mut flags = String::new();
                if worktree.locked.is_some() {
                    flags.push_str(" [locked]");
                }
                if worktree.prunable.is_some() {
                    flags.push_str(" [prunable]");
                }
                let mut lines = vec![Line::from(format!(
                    "└ {}{}  {}",
                    visible(worktree.label()),
                    flags,
                    visible(&worktree.path.display().to_string())
                ))];
                if app.session.adopted.as_deref()
                    == Some(worktree.path.display().to_string().as_str())
                {
                    lines.push(Line::from(if app.session.identity.is_some() {
                        "  └ scripted session · status unknown"
                    } else {
                        "  └ adopted · ready for fixture"
                    }));
                }
                ListItem::new(lines)
            })
            .collect();
        self.list.select(app.selected);
        let title = format!(" Project / worktrees ({}) ", app.worktrees.len());
        let widget = List::new(items)
            .block(
                Block::bordered()
                    .border_style(Style::default().fg(Color::Cyan))
                    .title(title),
            )
            .highlight_symbol("> ")
            .highlight_style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            );
        frame.render_stateful_widget(widget, list, &mut self.list);

        let detail = match app.current() {
            Some(worktree) => {
                let mut lines = vec![
                    Line::from(format!(
                        "Path: {}",
                        visible(&worktree.path.display().to_string())
                    )),
                    Line::from(format!(
                        "Branch: {}  HEAD: {}",
                        visible(worktree.label()),
                        worktree.head.as_deref().unwrap_or("none")
                    )),
                ];
                if let Some(reason) = &worktree.locked {
                    lines.push(Line::from(format!("Locked: {}", visible(reason))));
                }
                if let Some(reason) = &worktree.prunable {
                    lines.push(Line::from(format!("Prunable: {}", visible(reason))));
                }
                lines
            }
            None => vec![Line::from("No worktree selected.")],
        };
        frame.render_widget(
            Paragraph::new(detail)
                .block(
                    Block::bordered()
                        .border_style(Style::default().fg(Color::Cyan))
                        .title(" Selection "),
                )
                .wrap(Wrap { trim: false }),
            details,
        );

        let (message, color) =
            if let Some(error) = app.session.error.as_ref().or(app.error.as_ref()) {
                (
                    format!(
                        "{} · r to retry. Previous results retained.",
                        visible(error)
                    ),
                    Color::Red,
                )
            } else if let Some(path) = &app.session.adopted {
                (
                    format!(
                        "Adopted: {} · {}",
                        visible(path),
                        app.session
                            .identity
                            .as_deref()
                            .unwrap_or("ready to launch scripted fixture")
                    ),
                    Color::Cyan,
                )
            } else if app.loading {
                (
                    "Discovering worktrees… Navigation remains available.".to_owned(),
                    Color::Yellow,
                )
            } else {
                (
                    "Discovery complete. No checkout adopted; no child launched.".to_owned(),
                    Color::Green,
                )
            };
        frame.render_widget(
            Paragraph::new(message)
                .style(Style::default().fg(color))
                .block(
                    Block::bordered()
                        .border_style(Style::default().fg(Color::Cyan))
                        .title(" Status "),
                )
                .wrap(Wrap { trim: false }),
            status,
        );
        frame.render_widget(
            Paragraph::new("j/k select · r refresh · a adopt · l fixture · Enter attach · Ctrl-] dashboard · q quit/stop"),
            help,
        );
    }
}

// Paths/reasons may contain control characters. Display them as printable text,
// while retaining the original path bytes for all Git operations.
fn visible(text: &str) -> String {
    text.chars()
        .flat_map(|character| {
            if character.is_control() {
                character.escape_default().collect::<Vec<_>>()
            } else {
                vec![character]
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn renders_failure_and_handles_tiny_resize() {
        let mut app = App::new("missing repository".into());
        app.error = Some("not a git repository".into());
        let mut ui = Ui::default();
        let mut terminal = Terminal::new(TestBackend::new(100, 24)).unwrap();
        terminal.draw(|frame| ui.draw(frame, &app)).unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(text.contains("not a git repository"));
        terminal.backend_mut().resize(12, 4);
        terminal.draw(|frame| ui.draw(frame, &app)).unwrap();
    }
}
