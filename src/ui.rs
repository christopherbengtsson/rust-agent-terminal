use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Clear, List, ListItem, ListState, Paragraph, Wrap},
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
        let title = if let Some(browser) = &app.browser {
            format!(" {:?} · retained work · reading only ", browser.mode)
        } else if let Some(view) = &app.diff {
            format!(
                " {} · {} ",
                if view.whole {
                    "Whole workspace"
                } else {
                    "Working"
                },
                crate::working_diff::printable(&view.name)
            )
        } else if app.cleanup_open {
            " History cleanup · oldest sealed stopped sessions first ".to_owned()
        } else if app.history_open {
            format!(
                " Retained history · {} · v raw/readable · [/] page ",
                if app.history_raw {
                    "escaped raw"
                } else {
                    "readable"
                }
            )
        } else if frame.area().width < 80 || frame.area().height < 22 {
            let tag = app
                .session
                .session_identity
                .as_ref()
                .map(|identity| identity.session.chars().take(8).collect::<String>())
                .unwrap_or_else(|| "unlaunched".into());
            format!(
                " {} · {} {tag} · Focus: {focus} ",
                if app.session.connection_lost {
                    "unknown".to_owned()
                } else {
                    format!("{:?}", app.session.lifecycle)
                },
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
        if let Some(browser) = &app.browser {
            draw_browser(frame, viewport, browser);
        } else if let Some(view) = &app.diff {
            draw_working_diff(frame, viewport, view);
        } else if app.workspaces_open || app.project_input.is_some() {
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
                    let runtime = app
                        .session
                        .sessions
                        .iter()
                        .filter(|entry| entry.task == workspace.task)
                        .max_by_key(|entry| entry.reserved);
                    let context = runtime.map_or_else(
                        || format!("{} sessions", workspace.sessions.len()),
                        |runtime| {
                            format!(
                                "{:?} · {} · {} sessions",
                                runtime.lifecycle,
                                runtime.status.state.label(),
                                workspace.sessions.len()
                            )
                        },
                    );
                    ListItem::new(vec![
                        Line::from(format!(
                            "{} / {}",
                            visible(project),
                            visible(&workspace.name)
                        )),
                        Line::from(format!("{} · {}", context, visible(&path))),
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
                " Projects / retained workspaces · A add · j/k select · Enter open · a adopt · R rename "
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
                        if app.history_raw {
                            page.text.as_str()
                        } else if let Some(readable) = &page.readable {
                            readable.text.as_str()
                        } else if page.projection_preparing {
                            "Preparing readable history in bounded background steps…"
                        } else {
                            "Readable projection unavailable; v shows escaped raw bytes"
                        }
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
                Paragraph::new("Adopt (a), then c Claude / x Codex; C/X supplies an optional first prompt. R renames the workspace."),
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
                        "  └ adopted · c/x start · C/X prompt · R rename".into()
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
        if let Some(form) = &app.create_form {
            let project = app
                .session
                .registry
                .projects
                .iter()
                .find(|p| p.id == form.intent.project)
                .map(|p| p.name.as_str())
                .unwrap_or("Unavailable");
            let fields = [
                format!("Base: {}", form.intent.base),
                format!("Branch: {}", form.intent.branch),
                format!("Folder: {}", form.path),
                format!(
                    "First prompt (optional): {}",
                    form.intent.prompt.as_deref().unwrap_or_default()
                ),
                format!("Agent: {:?} (Left/Right changes)", form.intent.agent),
            ];
            let mut text = format!("Project: {project}\n");
            for (i, field) in fields.iter().enumerate() {
                text.push_str(&format!(
                    "{} {}\n",
                    if i == form.field { ">" } else { " " },
                    visible(field)
                ));
            }
            if let Some(preview) = &app.session.creation.preview {
                text.push_str(&format!("Available bases: {}\n", preview.bases.join(", ")));
                if preview.default_base.is_none() {
                    text.push_str("Choose an explicit base; no unambiguous remote default.\n");
                }
                if let Some(error) = &preview.error {
                    text.push_str(&visible(error));
                }
            }
            text.push_str("\nEnter: Create without starting\nCtrl-S: Create and start (supplied prompt sent once)\nTab: next field · Ctrl-U: clear · Esc: cancel");
            frame.render_widget(Clear, session);
            frame.render_widget(
                Paragraph::new(text)
                    .block(Block::bordered().title(" New independent workspace "))
                    .wrap(Wrap { trim: false }),
                session,
            );
        } else if let Some((_, text)) = &app.first_prompt {
            frame.render_widget(Clear, session);
            frame.render_widget(
                Paragraph::new(visible(text))
                    .block(
                        Block::bordered()
                            .title(" First prompt · Enter starts · Esc cancels · optional "),
                    )
                    .wrap(Wrap { trim: false }),
                session,
            );
        } else if let Some((_, text)) = &app.rename_workspace {
            frame.render_widget(Clear, session);
            frame.render_widget(
                Paragraph::new(visible(text))
                    .block(Block::bordered().title(
                        " Workspace name (local) · Enter saves · Ctrl-U clears · Esc cancels ",
                    ))
                    .wrap(Wrap { trim: false }),
                session,
            );
        }
        if app.creation_open
            && app.create_form.is_none()
            && app.first_prompt.is_none()
            && app.rename_workspace.is_none()
        {
            let text = if let Some(record) = app.creation_record() {
                let project = app
                    .session
                    .registry
                    .projects
                    .iter()
                    .find(|p| p.id == record.intent.project)
                    .map(|p| p.name.as_str())
                    .unwrap_or(&record.intent.project);
                let launch = app
                    .session
                    .creation
                    .launches
                    .iter()
                    .find(|r| r.request == record.intent.request);
                let launch = if record.launch_cancelled {
                    launch.map(|r|format!("Original launch intent cancelled; original process {:?}",r.lifecycle)).unwrap_or_else(||"Original launch intent cancelled; later conversations remain independent".into())
                } else if let Some(receipt) = launch {
                    if let Some(result) = &receipt.result {
                        result
                            .error
                            .as_ref()
                            .map(|e| format!("Native launch failed: {}", visible(e)))
                            .unwrap_or_else(|| {
                                if receipt.lifecycle == crate::events::Lifecycle::Running {
                                    "Native agent started; attach through its workspace".into()
                                } else {
                                    format!(
                                        "Native launch confirmed; process {:?}; history retained",
                                        receipt.lifecycle
                                    )
                                }
                            })
                    } else if receipt.pending {
                        "Native launch awaiting durable result".into()
                    } else {
                        receipt
                            .error
                            .as_ref()
                            .map(|e| format!("Launch unavailable: {}", visible(e)))
                            .unwrap_or_else(|| {
                                "No confirmed native launch result; query original operation".into()
                            })
                    }
                } else if !record.intent.start {
                    "Created without starting".into()
                } else {
                    "No confirmed native launch; explicit Start required after recovery".into()
                };
                let action = if record.phase != crate::creation::Phase::LocalChoice
                    || record.commit.is_none()
                {
                    "Local fallback unavailable for this phase".into()
                } else if record.intent.start && !record.launch_cancelled {
                    format!("L: Create locally and start {:?}", record.intent.agent)
                } else {
                    "L: Create locally without starting".into()
                };
                format!(
                    "Project: {}\nPhase: {:?}\nBase: {}\nCommit: {}\nBranch: {}\nFolder: {}\n{}\n\n{}\n\n{}\nG: Query/recover creation without starting\n[/]: receipts · Z: cancel · Esc: return",
                    visible(project),
                    record.phase,
                    visible(record.base_ref.as_deref().unwrap_or(&record.intent.base)),
                    record.commit.as_deref().unwrap_or("not yet selected"),
                    visible(&record.intent.branch),
                    visible(
                        &crate::creation::path(&record.intent.path)
                            .display()
                            .to_string()
                    ),
                    launch,
                    visible(&record.message),
                    action
                )
            } else {
                app.session
                    .creation
                    .error
                    .clone()
                    .unwrap_or_else(|| "Creation receipt pending · Z cancels · Esc returns".into())
            };
            frame.render_widget(Clear, session);
            frame.render_widget(
                Paragraph::new(text)
                    .block(Block::bordered().title(" Creation receipt · original operation "))
                    .wrap(Wrap { trim: false }),
                session,
            );
        }
        if app.create_form.is_none()
            && (app.creation_open
                || app.session.creation.pending
                || app.session.creation.error.is_some()
                || app
                    .session
                    .creation
                    .records
                    .last()
                    .is_some_and(|r| !matches!(r.phase, crate::creation::Phase::Created)))
        {
            let message=app.session.creation.error.clone().or_else(||app.creation_record().map(|r|format!("{:?}: {} · branch {} · folder {} · base {} · commit {} · L local fallback · G recover · Z cancel · Esc dismiss",r.phase,r.message,r.intent.branch,crate::creation::path(&r.intent.path).display(),r.base_ref.as_deref().unwrap_or("pending"),r.commit.as_deref().unwrap_or("pending")))).unwrap_or_else(||"Workspace creation in progress · Z cancel".into());
            frame.render_widget(
                Paragraph::new(visible(&message))
                    .block(Block::bordered().title(" Workspace creation "))
                    .wrap(Wrap { trim: false }),
                status,
            );
        }
        let compact_help;
        let help_text = if app.browser.is_some() && frame.area().width < 80 {
            "Enter search/open · ↑/↓ select · Esc back\nTab mode · n next · Ctrl-R refresh · / edit"
        } else if app.browser.is_some() {
            "Enter search/open · ↑/↓ select · Tab mode · Esc back · n next · Ctrl-R refresh · / edit"
        } else if app.create_form.is_some() {
            "Tab fields · Left/Right agent · Enter Create without starting\nCtrl-S Create and start · Ctrl-U clear · Esc cancel"
        } else if app.creation_open {
            "L explicit local creation (launch choice in receipt) · G recover without starting\n[/] receipt · Z cancel creation · Esc return · q quit"
        } else if app.diff.is_some()
            && !app.attached
            && (frame.area().width < 80 || frame.area().height < 22)
        {
            "w Working/Whole · e inspect ALL · o LIVE · j/k file\nb base · r refresh · f/Esc back · s stop · q quit"
        } else if app.first_prompt.is_some() || app.rename_workspace.is_some() {
            app.keyboard_help()
        } else if !app.attached
            && app.project_input.is_some()
            && (frame.area().width < 80 || frame.area().height < 22)
        {
            "Existing local repository path · Enter register
Esc cancel · registration never starts an agent"
        } else if !app.attached
            && app.workspaces_open
            && (frame.area().width < 80 || frame.area().height < 22)
        {
            "N create  j/k select  [/] project  a adopt  E LIVE  R rename
A add · h history · w/Esc back · d detach · q quit"
        } else if app.cleanup_open
            && !app.attached
            && (frame.area().width < 80 || frame.area().height < 22)
        {
            "j/k select · n/p page · Enter preview · D delete\nEsc cancel · g refresh · y retry · s stop · q quit"
        } else if app.history_open
            && !app.attached
            && (frame.area().width < 80 || frame.area().height < 22)
        {
            "h live · v raw/readable · [/] page · j/k scroll\nq quit · s stop · y retry · g cleanup · d detach"
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
                    "q quit s stop h history S text / find B chats {}\nN create  O receipt  w workspaces  f diff  E LIVE  d detach",
                    if app.session.lifecycle == crate::session::Lifecycle::Running {
                        "Enter attach"
                    } else {
                        "v resume"
                    }
                );
                &compact_help
            }
        } else {
            app.keyboard_help()
        };
        frame.render_widget(
            Paragraph::new(app.editor_message.as_deref().unwrap_or(help_text)),
            help,
        );
    }
}

fn draw_browser(frame: &mut Frame, area: Rect, browser: &crate::browser::Browser) {
    if let Some(page) = &browser.opened {
        let text = format!(
            "Committed passage v{} · raw {}–{} / {}\n{}",
            page.version, page.raw_start, page.raw_end, page.committed, page.text
        );
        frame.render_widget(
            Paragraph::new(text)
                .wrap(Wrap { trim: false })
                .scroll((browser.scroll, 0)),
            area,
        );
        return;
    }
    let [query, notice, rows] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(2),
        Constraint::Min(1),
    ])
    .areas(area);
    frame.render_widget(
        Paragraph::new(format!(
            "{:?}: {}{}",
            browser.mode,
            crate::persistence::search::visible(&browser.text),
            if browser.editing { "_" } else { "" }
        )),
        query,
    );
    let message = if browser.editing {
        "Enter searches · ↓ selects results"
    } else {
        browser.response.as_ref().map_or(
        "Enter searches · ↓ selects results",
        |response| {
            if let Some(error) = &response.error {
                error.as_str()
            } else if browser.selected.as_ref().is_some_and(|selected|!response.rows.iter().any(|row| &row.key==selected)) {
                "Selected session/passage unavailable; choose another row or refresh."
            } else if response.preparing {
                "Searching in bounded background steps…"
            } else if response.rows.is_empty() {
                "No results. Index catches up to committed output; Ctrl-R refreshes."
            } else {
                "Partial index of committed text · exact words (all, case-insensitive). Ctrl-R refreshes."
            }
        },
    )
    };
    frame.render_widget(Paragraph::new(message).wrap(Wrap { trim: false }), notice);
    if let Some(response) = &browser.response {
        let selected = response
            .rows
            .iter()
            .position(|r| Some(&r.key) == browser.selected.as_ref());
        let items = response.rows.iter().map(|row| {
            let width = rows.width.saturating_sub(3).max(1) as usize;
            let mut lines = Vec::new();
            let text = if row.snippet.is_empty() {
                row.label.clone()
            } else {
                format!("{}\n{}", row.snippet, row.label)
            };
            for line in text.lines() {
                let chars = line.chars().collect::<Vec<_>>();
                for chunk in chars.chunks(width) {
                    lines.push(chunk.iter().collect::<String>());
                }
            }
            ListItem::new(lines.join("\n"))
        });
        frame.render_stateful_widget(
            List::new(items)
                .highlight_symbol("> ")
                .highlight_style(Style::default().fg(Color::Cyan)),
            rows,
            &mut ListState::default().with_selected(selected),
        );
    }
}

fn draw_working_diff(frame: &mut Frame, area: Rect, view: &crate::working_diff::View) {
    let compact = area.width < 70 || area.height < 18;
    let snapshot = view.snapshot.as_ref();
    let freshness = if view.loading {
        "Refreshing; previous snapshot"
    } else if view.error.is_some() {
        "Refresh failed; previous snapshot"
    } else {
        "Snapshot; not live"
    };
    let request = snapshot.map_or(view.request.as_str(), |snapshot| snapshot.request.as_str());
    let checkout = snapshot
        .map(|snapshot| crate::working_diff::printable(&snapshot.checkout.root.to_string_lossy()))
        .unwrap_or_else(|| "resolving…".into());
    let identity = format!(
        "Workspace {}\nCheckout {}\nRequest {}\n{}{}",
        crate::working_diff::printable(&view.workspace),
        checkout,
        request,
        freshness,
        snapshot
            .map(|snapshot| format!(
                " · {}{}",
                snapshot.captured_millis,
                if snapshot.changed_during_capture {
                    " · changed during capture; refresh"
                } else {
                    " · r refresh"
                }
            ))
            .unwrap_or_default()
    );
    let [identity_area, error_area, body] = Layout::vertical([
        Constraint::Length(4),
        Constraint::Length(if view.base_entry.is_some() {
            3
        } else if view.error.is_some() {
            if snapshot.is_none() { 6 } else { 3 }
        } else {
            0
        }),
        Constraint::Min(1),
    ])
    .areas(area);
    frame.render_widget(Paragraph::new(identity), identity_area);
    if let Some(entry) = &view.base_entry {
        frame.render_widget(Paragraph::new(format!("Local base (full refs/heads/… or refs/remotes/…):\n{}▏\nEnter saves and compares · Esc cancels · Ctrl-U clears", crate::working_diff::printable(entry))).wrap(Wrap { trim: false }), error_area);
    } else if let Some(error) = &view.error {
        frame.render_widget(
            Paragraph::new(error.as_str())
                .style(Style::default().fg(Color::Yellow))
                .wrap(Wrap { trim: false }),
            error_area,
        );
    }
    let Some(snapshot) = snapshot else {
        frame.render_widget(
            Paragraph::new(if view.loading {
                "Loading comparison…"
            } else {
                "Comparison unavailable; r retries. Esc returns."
            }),
            body,
        );
        return;
    };
    let (files_area, content_area) = if compact {
        let [files, content] = Layout::vertical([
            Constraint::Length(if snapshot.files.is_empty() { 0 } else { 3 }),
            Constraint::Min(1),
        ])
        .areas(body);
        (files, content)
    } else {
        let [files, content] =
            Layout::horizontal([Constraint::Percentage(32), Constraint::Min(1)]).areas(body);
        (files, content)
    };
    let items = snapshot
        .files
        .iter()
        .map(|file| {
            ListItem::new(vec![
                Line::from(file.label()),
                Line::from(format!("  {}", file.state)),
            ])
        })
        .collect::<Vec<_>>();
    let selected = snapshot
        .files
        .iter()
        .position(|file| Some(&file.path) == view.selected.as_ref());
    let mut state = ListState::default().with_selected(selected);
    let title = if snapshot.omitted > 0 {
        format!("Files: {} omitted; status limit", snapshot.omitted)
    } else {
        format!("Files ({}) · j/k", snapshot.files.len())
    };
    frame.render_stateful_widget(
        List::new(items)
            .block(Block::default().title(title))
            .highlight_symbol("> ")
            .highlight_style(Style::default().fg(Color::Cyan)),
        files_area,
        &mut state,
    );
    let content = if view.selected == snapshot.selected {
        snapshot.content.as_str()
    } else {
        "Loading selected file; previous patch withheld."
    };
    let lines = content
        .lines()
        .map(|line| {
            let color = if line.starts_with('+') {
                Color::Green
            } else if line.starts_with('-') {
                Color::LightRed
            } else if line.starts_with("@@") {
                Color::Cyan
            } else {
                Color::Gray
            };
            Line::styled(line, Style::default().fg(color))
        })
        .collect::<Vec<_>>();
    frame.render_widget(
        Paragraph::new(lines)
            .scroll((view.scroll, view.horizontal))
            .block(Block::default().title(format!(
                "Content · line {} · column {}",
                view.scroll as usize + 1,
                view.horizontal as usize + 1
            ))),
        content_area,
    );
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
    fn dashboard_help_keeps_history_stop_quit_and_native_actions_visible_without_resizing() {
        for (cols, rows, dimensions) in [(160, 40, (32, 104)), (60, 18, (11, 58))] {
            for lifecycle in [
                crate::events::Lifecycle::Stopped,
                crate::events::Lifecycle::Running,
            ] {
                let mut app = App::new("repo".into());
                app.session.lifecycle = lifecycle;
                let mut terminal = Terminal::new(TestBackend::new(cols, rows)).unwrap();
                terminal
                    .draw(|frame| Ui::default().draw(frame, &app))
                    .unwrap();
                let area = Rect::new(0, 0, cols, rows);
                assert_eq!(Ui::session_dimensions(area), Some(dimensions));
                let help = Ui::layout(area).3;
                let buffer = terminal.backend().buffer();
                let footer = (help.y..help.bottom())
                    .map(|row| {
                        (help.x..help.right())
                            .map(|col| {
                                buffer.content
                                    [usize::from(row) * usize::from(cols) + usize::from(col)]
                                .symbol()
                            })
                            .collect::<String>()
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                for action in ["h history", "s stop", "q quit", "d detach", "E "] {
                    assert!(
                        footer.contains(action),
                        "{cols}x{rows}: {action} missing: {footer}"
                    );
                }
                let continuation = if lifecycle == crate::events::Lifecycle::Running {
                    "Enter attach"
                } else {
                    "v resume"
                };
                assert!(
                    footer.contains(continuation),
                    "{cols}x{rows}: {continuation} missing: {footer}"
                );
            }
        }
    }

    #[test]
    fn history_help_describes_raw_toggle_instead_of_native_resume() {
        for (cols, rows) in [(160, 40), (60, 18)] {
            let mut app = App::new("repo".into());
            app.history_open = true;
            app.session.lifecycle = crate::events::Lifecycle::Interrupted;
            let mut terminal = Terminal::new(TestBackend::new(cols, rows)).unwrap();
            terminal
                .draw(|frame| Ui::default().draw(frame, &app))
                .unwrap();
            let help = Ui::layout(Rect::new(0, 0, cols, rows)).3;
            let buffer = terminal.backend().buffer();
            let footer = (help.y..help.bottom())
                .map(|row| {
                    (help.x..help.right())
                        .map(|col| {
                            buffer.content[usize::from(row) * usize::from(cols) + usize::from(col)]
                                .symbol()
                        })
                        .collect::<String>()
                })
                .collect::<Vec<_>>()
                .join("\n");
            for action in ["h live", "v raw/readable", "q quit", "s stop"] {
                assert!(
                    footer.contains(action),
                    "{cols}x{rows}: {action} missing: {footer}"
                );
            }
            assert!(
                !footer.contains("v resume"),
                "history toggle never resumes native execution: {footer}"
            );
        }
    }

    #[test]
    fn readable_history_and_explicit_raw_toggle_render_without_launching_resume() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let mut app = App::new("missing-checkout".into());
        app.history_open = true;
        app.session.history = Some(crate::persistence::HistoryPage {
            text: "RAW\\u{1b}[32mAnswer".into(),
            readable: Some(crate::history::ReadablePage {
                text: "Prompt café\nAnswer\ncommand output\n".into(),
                version: 1,
                ..Default::default()
            }),
            ..Default::default()
        });
        for (cols, rows) in [(160, 40), (60, 18)] {
            for raw in [false, true] {
                app.history_raw = raw;
                let mut terminal = Terminal::new(TestBackend::new(cols, rows)).unwrap();
                terminal
                    .draw(|frame| Ui::default().draw(frame, &app))
                    .unwrap();
                let text = terminal
                    .backend()
                    .buffer()
                    .content
                    .iter()
                    .map(|cell| cell.symbol())
                    .collect::<String>();
                assert!(
                    text.contains(if raw {
                        "RAW\\u{1b}[32mAnswer"
                    } else {
                        "Prompt café"
                    }),
                    "{text}"
                );
                if let Some(directory) = std::env::var_os("RAT_HISTORY_EVIDENCE") {
                    std::fs::create_dir_all(&directory).unwrap();
                    std::fs::write(
                        std::path::PathBuf::from(directory)
                            .join(format!("08-history-{cols}-{raw}.txt")),
                        &text,
                    )
                    .unwrap();
                }
            }
        }
        let effect = app.input(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE));
        assert!(
            effect.is_none(),
            "history v must never request native resume"
        );
        assert!(!app.history_raw);
        assert!(
            app.error.is_none(),
            "history toggle fell through into resume: {:?}",
            app.error
        );
    }

    #[test]
    fn workspace_picker_preserves_independent_lifecycle_and_semantic_status() {
        let mut app = App::new("repo".into());
        app.workspaces_open = true;
        for (task, attention) in [
            ("A", crate::native_status::State::Approval),
            ("B", crate::native_status::State::Unknown),
        ] {
            app.session
                .registry
                .workspaces
                .push(crate::registry::Workspace {
                    creation: None,
                    id: task.into(),
                    task: task.into(),
                    legacy_tasks: vec![],
                    name: task.into(),
                    project: None,
                    path: Some(task.as_bytes().to_vec()),
                    checkout: None,
                    sessions: vec![task.into()],
                });
            app.session.sessions.push(crate::events::SessionSummary {
                target: task.into(),
                task: task.into(),
                lifecycle: crate::events::Lifecycle::Running,
                reserved: true,
                status: crate::native_status::Status {
                    state: attention,
                    ..Default::default()
                },
                ..Default::default()
            });
        }
        for (cols, rows) in [(160, 40), (60, 18)] {
            let mut terminal = Terminal::new(TestBackend::new(cols, rows)).unwrap();
            terminal
                .draw(|frame| Ui::default().draw(frame, &app))
                .unwrap();
            let text = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>();
            assert!(text.contains("Running · approval needed"), "{text}");
            assert!(text.contains("Running · unknown"), "{text}");
        }
    }

    #[test]
    fn retained_workspace_selection_is_visible_in_normal_and_small_terminals() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let mut app = App::new("repo".into());
        app.session.registry.workspaces = (0..20)
            .map(|index| crate::registry::Workspace {
                creation: None,
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
    #[test]
    fn new_workspace_form_exposes_both_creation_actions_at_normal_and_small_sizes() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let mut app = App::new("repo".into());
        app.session
            .registry
            .projects
            .push(crate::registry::Project {
                id: "project".into(),
                name: "Example".into(),
                common_dir: b"/repo/.git".to_vec(),
            });
        app.input(KeyEvent::new(KeyCode::Char('N'), KeyModifiers::NONE));
        for (cols, rows) in [(160, 40), (60, 18)] {
            let mut terminal = Terminal::new(TestBackend::new(cols, rows)).unwrap();
            terminal
                .draw(|frame| Ui::default().draw(frame, &app))
                .unwrap();
            let buffer = terminal.backend().buffer();
            let text = (0..rows)
                .map(|y| {
                    (0..cols)
                        .map(|x| buffer[(x, y)].symbol())
                        .collect::<String>()
                })
                .collect::<Vec<_>>()
                .join("\n");
            assert!(text.contains("Create without starting"), "{text}");
            assert!(text.contains("Create and start"), "{text}");
        }
    }
    #[test]
    fn local_fallback_preview_shows_exact_commit_before_the_explicit_choice() {
        let mut app = App::new("repo".into());
        app.creation_open = true;
        let commit = "1234567890123456789012345678901234567890";
        app.session.creation.records.push(serde_json::from_value(serde_json::json!({"intent":{"request":"request","project":"project","base":"origin/main","branch":"workspace/new","path":b"/managed/workspace".to_vec(),"agent":"Codex","start":true,"prompt":null},"repository":[],"common_dir":[],"phase":"LocalChoice","base_ref":"refs/remotes/origin/main","commit":commit,"message":"Latest code could not be obtained from configured remote. ".repeat(8),"checkout":null})).unwrap());
        for (cols, rows) in [(160, 40), (60, 18)] {
            let mut terminal = Terminal::new(TestBackend::new(cols, rows)).unwrap();
            terminal
                .draw(|frame| Ui::default().draw(frame, &app))
                .unwrap();
            let buffer = terminal.backend().buffer();
            let text = (0..rows)
                .map(|y| {
                    (0..cols)
                        .map(|x| buffer[(x, y)].symbol())
                        .collect::<String>()
                })
                .collect::<Vec<_>>()
                .join("\n");
            assert!(text.contains(commit), "{text}");
            assert!(text.contains("LocalChoice"), "{text}");
        }
    }
}
