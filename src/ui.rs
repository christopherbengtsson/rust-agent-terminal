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

    pub fn session_dimensions(area: Rect) -> Option<(u16, u16)> {
        let viewport = Self::session_viewport(area);
        (viewport.width > 0 && viewport.height > 0).then_some((viewport.height, viewport.width))
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
        let title = if app.cleanup_open {
            " History cleanup · oldest sealed stopped sessions first ".to_owned()
        } else if app.history_open {
            " Retained history · controls escaped · n/p session · [/] page ".to_owned()
        } else if frame.area().width < 80 || frame.area().height < 22 {
            let tag = app
                .session
                .session_identity
                .as_ref()
                .map(|identity| identity.session.chars().take(8).collect::<String>())
                .unwrap_or_else(|| "unlaunched".into());
            format!(
                " {} {tag} · Focus: {focus} ",
                app.session.agent.as_deref().unwrap_or("Session")
            )
        } else {
            format!(
                " {} · Focus: {focus} · Process: {} · semantic status: {} ",
                app.session.agent.as_deref().unwrap_or("Session"),
                if app.session.connection_lost {
                    format!("unknown (last seen {:?})", app.session.lifecycle)
                } else {
                    format!("{:?}", app.session.lifecycle)
                },
                app.session.status.state.label()
            )
        };
        frame.render_widget(
            Block::bordered()
                .border_style(Style::default().fg(Color::Cyan))
                .title(title),
            session,
        );
        let viewport = Self::session_viewport(frame.area());
        if app.workspaces_open || app.project_input.is_some() {
            let items = app
                .session
                .registry
                .workspaces
                .iter()
                .map(|workspace| {
                    let project = workspace
                        .project
                        .as_ref()
                        .and_then(|id| {
                            app.session
                                .registry
                                .projects
                                .iter()
                                .find(|project| &project.id == id)
                        })
                        .map_or("Unknown project (legacy)", |project| project.name.as_str());
                    let path = workspace
                        .path
                        .as_ref()
                        .map(|path| {
                            use std::os::unix::ffi::OsStrExt;
                            std::path::Path::new(std::ffi::OsStr::from_bytes(path))
                                .display()
                                .to_string()
                        })
                        .unwrap_or_else(|| "checkout unknown · retained legacy history".into());
                    ListItem::new(vec![
                        Line::from(format!(
                            "{} / {}",
                            visible(project),
                            visible(&workspace.name)
                        )),
                        Line::from(format!(
                            "{} · {} sessions",
                            visible(&path),
                            workspace.sessions.len()
                        )),
                    ])
                })
                .collect::<Vec<_>>();
            let selected = app
                .session
                .registry
                .workspaces
                .iter()
                .position(|workspace| Some(&workspace.id) == app.workspace_selected.as_ref());
            let mut selection = ListState::default().with_selected(selected);
            let title = if let Some(path) = &app.project_input {
                format!(" Add project: {}_ ", visible(path))
            } else if app.session.registry_pending {
                " Registering project… ".into()
            } else {
                " Projects / retained workspaces · A add · j/k select · Enter discover · a adopt "
                    .into()
            };
            frame.render_stateful_widget(
                List::new(items)
                    .block(Block::bordered().title(title))
                    .highlight_symbol("> ")
                    .highlight_style(Style::default().fg(Color::Cyan)),
                session,
                &mut selection,
            );
        } else if app.cleanup_open {
            let mut text = String::new();
            if let Some(preview) = &app.cleanup_preview {
                text.push_str(&format!("Confirm selected history deletion\n{}\nCheckout: {}\nDelete only this managed transcript segment ({} allocated bytes).\nPreserve checkout files, task working changes, native conversation records, metadata/request outcomes and other sessions.\nSpace recovery is not guaranteed. D deletes this UUID; Esc cancels.\n", visible(&preview.identity), visible(&preview.checkout), preview.bytes.map_or("unknown".into(), |bytes| bytes.to_string())));
            } else if let Some(page) = &app.session.cleanup {
                text.push_str(
                    "History cleanup — explicit retention; Enter previews selected history\n",
                );
                if page.entries.is_empty() {
                    text.push_str("No retained transcript history on this page.\n");
                }
                for (index, entry) in page.entries.iter().enumerate() {
                    let age =
                        entry
                            .created
                            .map_or("unknown (legacy timestamp)".into(), |created| {
                                let now = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .map_or(created, |time| time.as_secs());
                                format!("{} seconds", now.saturating_sub(created))
                            });
                    text.push_str(&format!(
                        "{} {}\n  age {} · {} allocated bytes · {}\n",
                        if app.cleanup_selected == index {
                            ">"
                        } else {
                            " "
                        },
                        visible(&entry.identity),
                        age,
                        entry
                            .bytes
                            .map_or("unavailable".into(), |bytes| bytes.to_string()),
                        visible(&entry.status)
                    ));
                }
                text.push_str(&format!("Page {} · n/p page · Esc back\n", page.index + 1));
            } else {
                text.push_str("Loading history cleanup; scanning storage in bounded batches…\n");
            }
            if let Some(message) = &app.session.cleanup_message {
                text.push_str(&visible(message));
            }
            frame.render_widget(Paragraph::new(text).wrap(Wrap { trim: false }), viewport);
        } else if app.history_open {
            let text = app
                .session
                .history
                .as_ref()
                .map(|page| {
                    format!(
                        "{}\n{} · recorded {:?}{} · byte {}\n{}",
                        visible(&page.identity),
                        visible(&page.checkout),
                        page.lifecycle,
                        page.process_exit
                            .as_ref()
                            .map_or(String::new(), |outcome| format!(
                                " · {}",
                                visible(&outcome.reason())
                            )),
                        page.offset,
                        page.text
                    )
                })
                .unwrap_or_else(|| "Loading retained history…".into());
            frame.render_widget(
                Paragraph::new(text)
                    .wrap(Wrap { trim: false })
                    .scroll((app.history_scroll, 0)),
                viewport,
            );
        } else if let Some(screen) = app.session.decoded.as_ref() {
            draw_screen(frame, viewport, screen, app.attached);
        } else {
            frame.render_widget(
                Paragraph::new("Adopt (a), then c Claude Code / x Codex CLI / l scripted fixture."),
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
                        format!("  └ managed session · {}", app.session.status.state.label())
                    } else {
                        "  └ adopted · c Claude / x Codex / l fixture".into()
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

        let (message, color) = if app.shutdown_pending {
            (
                "Shutdown pending: confirming managed execution has stopped…".into(),
                Color::Yellow,
            )
        } else if let Some(error) = app.session.error.as_ref().or(app.error.as_ref()) {
            (
                format!("{} · Previous results retained.", visible(error)),
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
                        .unwrap_or("ready: c Claude / x Codex / l fixture")
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
        let message = if let Some(outcome) = &app.session.process_exit {
            if outcome.failed() {
                format!(
                    "Native process failed: {}\n{message}",
                    visible(&outcome.reason())
                )
            } else {
                message
            }
        } else {
            message
        };
        let message = if app.session.connection_lost {
            format!(
                "{}\n{message}",
                app.session
                    .identity
                    .as_deref()
                    .unwrap_or("No session identity")
            )
        } else {
            format!(
                "{message}\nHistory: {} · durable bytes {}",
                if app.session.metadata_pending {
                    "metadata unconfirmed"
                } else {
                    "metadata committed"
                },
                app.session.durable_bytes
            )
        };
        let message = if app.session.capture_pending_bytes > 0 {
            format!(
                "{message}\nCapture: accepted {} · written {} · pending {} · storage wait {} ms · producer wait {} ms · y retry / s stop{}",
                app.session.accepted_bytes,
                app.session.written_bytes,
                app.session.capture_pending_bytes,
                app.session.storage_wait_millis,
                app.session.producer_wait_millis,
                app.session
                    .storage_error
                    .as_ref()
                    .map_or(String::new(), |e| format!("\n{e}"))
            )
        } else {
            message
        };
        let message = if let Some(error) = &app.session.storage_error {
            format!(
                "{error}\nPending {} bytes · y retry / s stop\n{message}",
                app.session.capture_pending_bytes
            )
        } else if app.session.storage_wait_millis >= 250 {
            format!(
                "History storage pending {} ms · {} bytes retained · y retry / s stop\n{message}",
                app.session.storage_wait_millis, app.session.capture_pending_bytes
            )
        } else {
            message
        };
        let message = if let Some(cleanup) = &app.session.cleanup_message {
            format!("{}\n{message}", visible(cleanup))
        } else {
            message
        };
        let message = if app.session.capacity.low() {
            format!(
                "Low storage: {} bytes available (warning below 1 GiB) · g cleanup · y retry / s stop\n{message}",
                app.session.capacity.available.unwrap_or(0)
            )
        } else if let Some(error) = &app.session.capacity.error {
            format!(
                "Storage capacity unavailable: {} · g cleanup\n{message}",
                visible(error)
            )
        } else {
            message
        };
        let message = if app.session.capture_loss_possible {
            format!(
                "Stopped during capture trouble: unread agent output may have been lost. Accepted bytes retained; y retry history.\n{message}"
            )
        } else {
            message
        };
        let message = if app.session.error.is_some() && !app.session.connection_lost {
            format!(
                "{message}\n{}",
                app.session
                    .identity
                    .as_deref()
                    .unwrap_or("No session identity")
            )
        } else {
            message
        };
        frame.render_widget(
            Paragraph::new(format!(
                "{message}\nEvidence: {} · {}",
                app.session
                    .status
                    .evidence
                    .as_deref()
                    .unwrap_or("unavailable"),
                app.session.status.reason
            ))
            .style(Style::default().fg(color))
            .block(
                Block::bordered()
                    .border_style(Style::default().fg(Color::Cyan))
                    .title(format!(
                        " Status · semantic status: {} · {} ",
                        app.session.status.state.label(),
                        app.session.status.reason
                    )),
            )
            .wrap(Wrap { trim: false }),
            status,
        );
        let compact_help;
        let help_text = if app.cleanup_open
            && !app.attached
            && (frame.area().width < 80 || frame.area().height < 22)
        {
            "j/k select · n/p page · Enter preview · D delete\nEsc cancel · g refresh · y retry · s stop · q quit"
        } else if !app.attached
            && app.session.lifecycle == crate::session::Lifecycle::Interrupted
            && (frame.area().width < 80 || frame.area().height < 22)
        {
            "u reconcile · v resume · h history · d detach\nProcess: Interrupted · no automatic restart"
        } else if frame.area().width < 80 || frame.area().height < 22 {
            if app.attached {
                compact_help = format!(
                    "Ctrl-C interrupt · Ctrl-] dashboard\nProcess: {} · status: {}",
                    if app.session.connection_lost {
                        format!("unknown (last seen {:?})", app.session.lifecycle)
                    } else {
                        format!("{:?}", app.session.lifecycle)
                    },
                    app.session.status.state.label()
                );
                &compact_help
            } else {
                compact_help = format!(
                    "a adopt · c/x new Claude/Codex · v resume · Enter attach\ng cleanup · q quit/stop · d detach · Process: {} · status: {}",
                    if app.session.connection_lost {
                        format!("unknown (last seen {:?})", app.session.lifecycle)
                    } else {
                        format!("{:?}", app.session.lifecycle)
                    },
                    app.session.status.state.label()
                );
                &compact_help
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
    fn retained_workspace_selection_is_visible_in_normal_and_small_terminals() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let mut app = App::new("repo".into());
        app.session.registry.workspaces = (0..20)
            .map(|index| crate::registry::Workspace {
                id: format!("workspace-{index}"),
                task: format!("task-{index}"),
                legacy_tasks: vec![format!("task-{index}")],
                name: format!("retained-{index}"),
                project: None,
                path: None,
                checkout: None,
                sessions: vec!["retained-session".into()],
            })
            .collect();
        app.input(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE));
        for _ in 0..19 {
            app.input(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        }
        for (cols, rows) in [(160, 40), (60, 18)] {
            let mut terminal = Terminal::new(TestBackend::new(cols, rows)).unwrap();
            terminal
                .draw(|frame| Ui::default().draw(frame, &app))
                .unwrap();
            let buffer = terminal.backend().buffer();
            let text = buffer
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>();
            assert!(
                text.contains("> Unknown project (legacy) / retained-19"),
                "{text}"
            );
            assert!(text.contains("checkout unknown"));
            assert!(text.contains("j/k select"));
            if cols == 160 {
                assert!(text.contains("Reviews"));
                assert!(text.contains("Unavailable"));
            }
            if let Some(directory) = std::env::var_os("RAT_REGISTRY_EVIDENCE") {
                std::fs::create_dir_all(&directory).unwrap();
                let cells = buffer.content.iter().enumerate().map(|(index,cell)| serde_json::json!({"row":index / cols as usize,"col":index % cols as usize,"text":cell.symbol(),"fg":format!("{:?}",cell.fg),"bg":format!("{:?}",cell.bg),"bold":cell.modifier.contains(Modifier::BOLD),"wide":false})).collect::<Vec<_>>();
                std::fs::write(std::path::Path::new(&directory).join(format!("05-registry-testbackend-{cols}.json")),serde_json::to_vec_pretty(&serde_json::json!({"source":"Ratatui TestBackend supporting render; not a real PTY or terminal screenshot","cols":cols,"rows":rows,"cells":cells,"hide_cursor":true})).unwrap()).unwrap();
            }
        }
    }

    #[test]
    fn cleanup_renders_unknown_age_physical_size_preview_and_low_space_recovery_controls() {
        let mut app = App::new("repo".into());
        app.cleanup_open = true;
        app.session.capacity.available = Some(0);
        let entry = crate::cleanup::Entry {
            id: "selected-session".into(),
            identity: "task=task15 session=selected-session".into(),
            bytes: Some(8192),
            status: "incomplete deletion; explicitly retry".into(),
            eligible: true,
            ..Default::default()
        };
        app.session.cleanup = Some(crate::cleanup::Page {
            entries: vec![entry.clone()],
            ..Default::default()
        });
        let mut terminal = Terminal::new(TestBackend::new(160, 40)).unwrap();
        terminal
            .draw(|frame| Ui::default().draw(frame, &app))
            .unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(text.contains("unknown (legacy timestamp)"));
        assert!(text.contains("8192 allocated bytes"));
        assert!(text.contains("task=task15"));
        assert!(text.contains("Low storage"));
        assert!(text.contains("incomplete deletion"));
        app.cleanup_preview = Some(entry);
        for (width, height) in [(160, 40), (70, 20)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal
                .draw(|frame| Ui::default().draw(frame, &app))
                .unwrap();
            let text: String = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect();
            assert!(text.contains("Confirm selected history deletion"));
            assert!(text.contains("Esc cancel"));
            assert!(text.contains("D delete"));
        }
    }

    #[test]
    fn storage_warning_has_an_exact_threshold_and_unknown_capacity_is_honest() {
        let mut app = App::new("repo".into());
        for (bytes, warning) in [
            (0, true),
            (1_073_741_823, true),
            (1_073_741_824, false),
            (u64::MAX, false),
        ] {
            app.session.capacity.available = Some(bytes);
            let mut terminal = Terminal::new(TestBackend::new(160, 40)).unwrap();
            terminal
                .draw(|frame| Ui::default().draw(frame, &app))
                .unwrap();
            let text: String = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect();
            assert_eq!(text.contains("Low storage"), warning);
        }
        app.session.capacity.available = None;
        app.session.capacity.error = Some("capacity read failed".into());
        let mut terminal = Terminal::new(TestBackend::new(160, 40)).unwrap();
        terminal
            .draw(|frame| Ui::default().draw(frame, &app))
            .unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(text.contains("Storage capacity unavailable"));
        assert!(!text.contains("Low storage"));
    }

    #[test]
    fn stopped_capture_fault_keeps_possible_loss_and_retry_visible() {
        let mut app = App::new("repo".into());
        app.session.lifecycle = crate::session::Lifecycle::Stopped;
        app.session.capture_loss_possible = true;
        app.session.capture_pending_bytes = 4096;
        app.session.storage_error =
            Some("History capture blocked: No space; y retry or s stop".into());
        let mut terminal = Terminal::new(TestBackend::new(160, 40)).unwrap();
        terminal
            .draw(|frame| Ui::default().draw(frame, &app))
            .unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(text.contains("unread agent output may have been lost"));
        assert!(text.contains("y retry"));
    }

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
    fn unusable_viewport_does_not_request_pty_resize() {
        assert_eq!(
            Ui::session_dimensions(Rect::new(0, 0, 100, 24)),
            Some((16, 64))
        );
        assert_eq!(Ui::session_dimensions(Rect::new(0, 0, 12, 4)), None);
        assert_eq!(Ui::session_dimensions(Rect::new(0, 0, 1, 30)), None);
        assert_eq!(Ui::session_dimensions(Rect::default()), None);
        assert_eq!(
            Ui::session_dimensions(Rect::new(0, 0, 60, 18)),
            Some((11, 58))
        );
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
                assert!(text.contains("x new Codex"));
                assert!(text.contains("v resume"));
                assert!(text.contains("quit/stop"));
            }
        }
    }

    #[test]
    fn interrupted_execution_shows_explicit_reconciliation_in_normal_and_small_layouts() {
        let mut app = App::new("repo".into());
        app.session.lifecycle = crate::session::Lifecycle::Interrupted;
        app.session.error = Some(
            "Execution uncertain; same-boot leader absence may require verified reboot".into(),
        );
        for width in [70, 120] {
            let mut ui = Ui::default();
            let mut terminal = Terminal::new(TestBackend::new(width, 30)).unwrap();
            terminal.draw(|frame| ui.draw(frame, &app)).unwrap();
            let text: String = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect();
            assert!(
                text.contains("u reconcile"),
                "recovery must remain available at {width} columns: {text}"
            );
            assert!(text.contains("Interrupted"));
        }
    }

    #[test]
    fn failed_native_exit_reason_remains_visible_after_a_control_error() {
        let mut app = App::new("repo".into());
        app.observe_session(crate::events::Observation {
            lifecycle: crate::events::Lifecycle::Failed,
            process_exit: Some(crate::events::ProcessExit {
                code: 7,
                signal: None,
            }),
            error: Some("History request unavailable; retry".into()),
            ..Default::default()
        });
        for width in [70, 120] {
            let mut ui = Ui::default();
            let mut terminal = Terminal::new(TestBackend::new(width, 30)).unwrap();
            terminal.draw(|frame| ui.draw(frame, &app)).unwrap();
            let text: String = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect();
            assert!(text.contains("Failed"), "{text}");
            assert!(
                text.contains("Native process failed: exit code 7"),
                "{text}"
            );
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
