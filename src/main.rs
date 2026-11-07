mod app;
mod archive;
mod archive_git;
mod browser;
mod child_scope;
mod cleanup;
mod codex_status;
mod creation;
mod editor;
mod events;
mod git;
mod history;
mod lifecycle;
mod native_input;
mod native_status;
mod persistence;
mod recovery;
mod registry;
mod session;
mod setup;
mod setup_execution;
mod terminal;
mod ui;
mod working_diff;

use std::{
    env,
    error::Error,
    io::{self, IsTerminal},
    path::PathBuf,
    time::Duration,
};

use crossterm::event::{self, Event as TerminalEvent};

use crate::{
    app::App,
    events::{Effect, Event},
    terminal::TerminalGuard,
    ui::Ui,
};

fn main() -> Result<(), Box<dyn Error>> {
    let mut arguments = env::args_os().skip(1);
    let first = arguments.next();
    if first.as_deref() == Some(std::ffi::OsStr::new("--launch-gate")) {
        let socket = PathBuf::from(arguments.next().ok_or("Missing gate socket")?);
        let nonce = arguments.next().ok_or("Missing gate nonce")?;
        return Ok(recovery::gate(
            &socket,
            nonce.to_str().ok_or("Invalid nonce")?,
            arguments.collect(),
        )?);
    }
    if first.as_deref() == Some(std::ffi::OsStr::new("--discover-worktrees")) {
        let repository = PathBuf::from(arguments.next().ok_or("Missing discovery repository")?);
        return Ok(git::discovery_helper(&repository)?);
    }
    if first.as_deref() == Some(std::ffi::OsStr::new("--supervisor")) {
        return session::supervisor(&PathBuf::from(
            arguments.next().ok_or("Missing state directory")?,
        ));
    }
    if first.as_deref() == Some(std::ffi::OsStr::new("--scripted-descendant")) {
        return Ok(session::scripted_descendant()?);
    }
    if first.as_deref() == Some(std::ffi::OsStr::new("--scripted-child")) {
        return Ok(session::scripted_child()?);
    }
    if first.as_deref() == Some(std::ffi::OsStr::new("--status-hook")) {
        if let (Some(path), Some(token), Some(session)) =
            (arguments.next(), arguments.next(), arguments.next())
        {
            native_status::hook(
                &PathBuf::from(path),
                token.to_string_lossy().into_owned(),
                session.to_string_lossy().into_owned(),
            );
        }
        return Ok(());
    }
    if first.as_deref() == Some(std::ffi::OsStr::new("--codex-status-hook")) {
        if let (Some(path), Some(token)) = (arguments.next(), arguments.next()) {
            codex_status::hook(&PathBuf::from(path), token.to_string_lossy().into_owned());
        }
        return Ok(());
    }
    let repository = match first {
        Some(argument) if argument == "--help" || argument == "-h" => {
            println!(
                "Usage: agent-terminal [repository-path]\n\nLists Git worktrees in a keyboard-driven dashboard. Defaults to the current directory.\n\nj/k or arrows: select · r: refresh · f: Working diff · q/Esc/Ctrl-C: quit\na adopt · c launch Claude Code · x launch Codex CLI · C/X optional first prompt then start · R rename workspace · l launch scripted PTY · Enter attach · Ctrl-] dashboard\ns stop session · u reconcile interrupted execution · d detach preserving execution · q quit and stop managed activity\nNative Claude/Codex launch is experimental; compatibility proof remains open. Authentication prompts stay in the native CLI; run claude auth login or codex login in your usual terminal if needed."
            );
            return Ok(());
        }
        Some(argument) => PathBuf::from(argument),
        None => env::current_dir()?,
    };
    let state_directory = match arguments.next() {
        Some(flag) if flag == "--state-dir" => {
            PathBuf::from(arguments.next().ok_or("Missing state directory")?)
        }
        Some(_) => return Err("Usage: agent-terminal [repository-path] [--state-dir path]".into()),
        None => PathBuf::from(env::var_os("HOME").ok_or("HOME unavailable")?)
            .join("Library/Application Support/rat/proof"),
    };
    if arguments.next().is_some() {
        return Err("Unexpected argument".into());
    }
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err("The dashboard needs an interactive terminal. Use --help for usage.".into());
    }

    let editor_config = editor::Config::from_environment()?;
    let mut editor_job = None;
    let mut editor_running: Option<(
        std::sync::mpsc::Receiver<io::Result<std::process::ExitStatus>>,
        bool,
        app::EditorContext,
    )> = None;
    let mut app = App::new(repository);
    let client = session::Client::start(state_directory)?;

    refresh(&mut app, &client);
    let (mut guard, mut terminal) = TerminalGuard::enter()?;
    let mut ui = Ui::default();

    loop {
        if let Some((result, inspection, _context)) = &editor_running {
            let result = match result.try_recv() {
                Ok(result) => Some(result),
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    Some(Err(io::Error::other("Editor worker disconnected")))
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => None,
            };
            if let Some(result) = result {
                let inspection = *inspection;
                guard.reacquire_editor(&mut terminal)?;
                let (_, _, context) = editor_running.take().unwrap();
                if let Ok(observation) = client.events.try_recv() {
                    app.observe_session(observation);
                }
                app.editor_returned(context);
                app.editor_message = Some(editor_result(result, inspection));
            } else {
                // Deliberate handoff coordination: editor owns stdin/stdout; no drawing/input.
                std::thread::sleep(Duration::from_millis(20));
                continue;
            }
        }
        if let Some(target) = app.editor_ready.take()
            && editor_job.is_none()
        {
            match editor::start(target) {
                Ok(job) => editor_job = Some(job),
                Err(e) => app.editor_message = Some(format!("Editor preparation failed: {e}")),
            }
        }
        if let Some(result) = editor_job.as_ref().and_then(
            |job: &std::sync::mpsc::Receiver<Result<editor::Prepared, String>>| job.try_recv().ok(),
        ) {
            editor_job = None;
            match result {
                Err(error) => app.editor_message = Some(error),
                Ok(prepared) => {
                    let inspection = prepared.inspection;
                    let context = app.editor_context();
                    guard.release_editor(&mut terminal)?;
                    match editor::launch(editor_config.clone(), prepared) {
                        Ok(result) => {
                            editor_running = Some((result, inspection, context));
                            continue;
                        }
                        Err(error) => {
                            guard.reacquire_editor(&mut terminal)?;
                            app.editor_returned(context);
                            app.editor_message = Some(editor_result(Err(error), inspection));
                        }
                    }
                }
            }
        }
        if let Ok(observation) = client.events.try_recv() {
            app.observe_session(observation);
        }
        if app.should_exit() {
            break;
        }
        let completed = terminal.draw(|frame| ui.draw(frame, &app))?;
        if let Some(Effect::Session(request)) = app.viewport(Ui::session_dimensions(completed.area))
        {
            app.viewport_dispatched(client.send(request));
        }
        if event::poll(Duration::from_millis(50))? {
            match event::read()? {
                TerminalEvent::Key(key) => match app.input(key) {
                    Some(Effect::Refresh) => refresh(&mut app, &client),
                    Some(Effect::Detach) => break,
                    Some(Effect::Session(request)) => {
                        if let Err(error) = client.send(request) {
                            app.shutdown_pending = false;
                            app.editor_source_request = None;
                            app.error = Some(error);
                        }
                    }
                    None => {}
                },
                TerminalEvent::Paste(text) => {
                    if let Some(Effect::Session(request)) = app.paste(text)
                        && let Err(error) = client.send(request)
                    {
                        app.error = Some(error);
                    }
                }
                TerminalEvent::Resize(_, _) => {} // Ratatui queries the new size on draw.
                _ => {}
            }
        }
    }
    Ok(())
}

fn refresh(app: &mut App, client: &session::Client) {
    if app.begin_refresh()
        && let Err(error) = client.send(session::Request::Discover {
            request: app.discovery_request,
            repository: session::path_bytes(&app.repository),
        })
    {
        app.apply(Event::WorktreesLoaded(Err(error)));
    }
}

fn editor_result(result: io::Result<std::process::ExitStatus>, inspection: bool) -> String {
    match result {
        Ok(status) if status.success() => {
            if inspection {
                "Returned from inspection snapshot; e reopens with fresh data".into()
            } else {
                "Returned from LIVE source editing; source is shared with the active agent. r refreshes comparison".into()
            }
        }
        Ok(status) => format!("Editor exited {status}; dashboard context restored"),
        Err(error) => format!(
            "Editor failed to start/wait: {error}; check RAT_EDITOR_JSON executable and arguments"
        ),
    }
}
