use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
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
    fn layout(area: Rect) -> (Rect, Rect, Rect, Rect) {
        let compact = area.width < 80 || area.height < 22;
        let [body, status, help] = Layout::vertical([
            Constraint::Min(1),
            Constraint::Length(if compact { 3 } else { 5 }),
            Constraint::Length(if compact { 2 } else { 1 }),
        ])
        .areas(area);
        let [sidebar, session] = Layout::horizontal([
            Constraint::Percentage(if compact { 0 } else { 34 }),
            Constraint::Min(1),
        ])
        .areas(body);
        (sidebar, session, status, help)
    }

    pub fn session_viewport(area: Rect) -> Rect {
        let (_, session, _, _) = Self::layout(area);
        let mut viewport = Block::bordered().inner(session);
        viewport.width = viewport.width.min(crate::session::MAX_COLS);
        viewport.height = viewport.height.min(crate::session::MAX_ROWS);
        viewport
    }

    pub fn draw(&mut self, frame: &mut Frame, app: &App) {
        frame.render_widget(
            Block::default().style(Style::default().bg(Color::Rgb(12, 17, 24)).fg(Color::Gray)),
            frame.area(),
        );
        let (sidebar, session, status, help) = Self::layout(frame.area());
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
        let focus = if app.attached { "session" } else { "dashboard" };
        let title = if frame.area().width < 80 || frame.area().height < 22 {
            let tag = app
                .session
                .identity
                .as_deref()
                .and_then(|id| id.split("session=").nth(1))
                .map(|id| id.chars().take(8).collect::<String>())
                .unwrap_or_else(|| "unlaunched".into());
            format!(" Fixture {tag} · Focus: {focus} ")
        } else {
            format!(" Scripted PTY · Focus: {focus} · semantic status: unknown ")
        };
        frame.render_widget(
            Block::bordered()
                .border_style(Style::default().fg(Color::Cyan))
                .title(title),
            session,
        );
        let viewport = Self::session_viewport(frame.area());
        if let Some(screen) = app.session.decoded.as_ref() {
            draw_screen(frame, viewport, screen, app.attached);
        } else {
            frame.render_widget(
                Paragraph::new("Adopt a checkout (a), then launch the fixture (l)."),
                viewport,
            );
        }
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
        let help_text = if frame.area().width < 80 {
            if app.attached {
                "Ctrl-C interrupt · Ctrl-] dashboard\nFocus: session · status: unknown"
            } else {
                "j/k · r refresh · a adopt · l fixture · Enter attach\nq quit/stop · Focus: dashboard"
            }
        } else {
            app.keyboard_help()
        };
        frame.render_widget(Paragraph::new(help_text), help);
    }
}

fn color(color: vt100::Color) -> Color {
    match color {
        vt100::Color::Default => Color::Reset,
        vt100::Color::Idx(index) => Color::Indexed(index),
        vt100::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
    }
}

fn draw_screen(frame: &mut Frame, area: Rect, screen: &vt100::Screen, attached: bool) {
    let (rows, cols) = screen.size();
    for row in 0..area.height.min(rows.get()) {
        for col in 0..area.width.min(cols.get()) {
            let Some(cell) = screen.cell(row, col) else {
                continue;
            };
            if cell.is_wide_continuation() {
                continue;
            }
            // Never let a wide glyph overwrite the dashboard border during resize.
            if cell.is_wide() && col + 1 >= area.width {
                continue;
            }
            let mut style = Style::default()
                .fg(color(cell.fgcolor()))
                .bg(color(cell.bgcolor()));
            for (enabled, modifier) in [
                (cell.bold(), Modifier::BOLD),
                (cell.dim(), Modifier::DIM),
                (cell.italic(), Modifier::ITALIC),
                (cell.underline(), Modifier::UNDERLINED),
                (cell.inverse(), Modifier::REVERSED),
            ] {
                if enabled {
                    style = style.add_modifier(modifier);
                }
            }
            frame.buffer_mut().set_string(
                area.x + col,
                area.y + row,
                if cell.contents().is_empty() {
                    " "
                } else {
                    cell.contents()
                },
                style,
            );
        }
    }
    let (row, col) = screen.cursor_position();
    if attached && !screen.hide_cursor() && row < area.height && col < area.width {
        frame.set_cursor_position((area.x + col, area.y + row));
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
    fn native_cells_keep_style_width_and_cursor() {
        let mut parser = vt100::Parser::new(4.try_into().unwrap(), 20.try_into().unwrap(), 0);
        parser.process(b"\x1b[1;31mstyled ");
        assert!(parser.screen().cell(0, 0).unwrap().bold());
        let mut app = App::new("repo".into());
        app.attached = true;
        let formatted = parser.screen().state_formatted();
        let mut restored = vt100::Parser::new(4.try_into().unwrap(), 20.try_into().unwrap(), 0);
        restored.process(formatted.as_bytes());
        assert!(
            restored.screen().cell(0, 0).unwrap().bold(),
            "{formatted:?}"
        );
        app.session.decoded = Some(restored.screen().clone());
        let mut terminal = Terminal::new(TestBackend::new(100, 24)).unwrap();
        terminal
            .draw(|frame| Ui::default().draw(frame, &app))
            .unwrap();
        let cell = &terminal.backend().buffer()[(35, 1)];
        assert_eq!(cell.fg, Color::Indexed(1));
        assert!(cell.modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn viewport_excludes_sidebar_borders_and_status_and_small_layout_keeps_focus() {
        assert_eq!(
            Ui::session_viewport(ratatui::layout::Rect::new(0, 0, 100, 24)),
            ratatui::layout::Rect::new(35, 1, 64, 16)
        );
        let mut app = App::new("repo".into());
        app.attached = true;
        let mut terminal = Terminal::new(TestBackend::new(60, 18)).unwrap();
        terminal
            .draw(|frame| Ui::default().draw(frame, &app))
            .unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(text.contains("Focus: session"));
        assert!(text.contains("status: unknown"));
        assert!(text.contains("Ctrl-]"));
    }

    #[test]
    fn focus_and_available_actions_are_visible() {
        let mut app = App::new("repo".into());
        let mut ui = Ui::default();
        let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
        for attached in [false, true] {
            app.attached = attached;
            terminal.draw(|frame| ui.draw(frame, &app)).unwrap();
            let text: String = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect();
            if attached {
                assert!(text.contains("Focus: session"));
                assert!(text.contains("Ctrl-C interrupt"));
                assert!(text.contains("Ctrl-] / Ctrl-5 dashboard"));
                assert!(!text.contains("quit/stop"));
            } else {
                assert!(text.contains("Focus: dashboard"));
                assert!(text.contains("Enter attach"));
                assert!(text.contains("quit/stop"));
            }
        }
    }

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
