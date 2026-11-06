mod app;
mod events;
mod git;
mod lifecycle;
mod native_input;
mod persistence;
mod session;
mod terminal;
mod ui;

use std::{
    env,
    error::Error,
    io::{self, IsTerminal},
    path::PathBuf,
    sync::mpsc::TryRecvError,
    time::Duration,
};

use crossterm::event::{self, Event as TerminalEvent};

use crate::{
    app::{App, Effect},
    events::Event,
    git::GitWorker,
    terminal::TerminalGuard,
    ui::Ui,
};

fn main() -> Result<(), Box<dyn Error>> {
    let mut arguments = env::args_os().skip(1);
    let first = arguments.next();
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
    let repository = match first {
        Some(argument) if argument == "--help" || argument == "-h" => {
            println!(
                "Usage: agent-terminal [repository-path]\n\nLists Git worktrees in a keyboard-driven dashboard. Defaults to the current directory.\n\nj/k or arrows: select · r: refresh · q/Esc/Ctrl-C: quit\na adopt · c launch Claude Code · x launch Codex CLI · l launch scripted PTY · Enter attach · Ctrl-] dashboard\ns stop session · d detach preserving execution · q quit and stop managed activity\nNative Claude/Codex launch is experimental; compatibility proof remains open. Authentication prompts stay in the native CLI; run claude auth login or codex login in your usual terminal if needed."
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

    let worker = GitWorker::start(repository.clone())?;
    let mut app = App::new(repository);
    let client = session::Client::start(state_directory)?;

    refresh(&mut app, &worker);
    let (_guard, mut terminal) = TerminalGuard::enter()?;
    let mut ui = Ui::default();

    let mut sent_viewport = None;
    loop {
        // Bound the work per frame so future streaming sources cannot monopolize it.
        for _ in 0..32 {
            match worker.events.try_recv() {
                Ok(event) => app.apply(event),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    app.apply(Event::WorktreesLoaded(Err(
                        "Git worker stopped; restart the dashboard.".into(),
                    )));
                    break;
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
        if let Some(dimensions) = Ui::session_dimensions(completed.area)
            && sent_viewport != Some(dimensions)
        {
            match client.send(session::Request::Resize {
                rows: dimensions.0,
                cols: dimensions.1,
            }) {
                Ok(()) => sent_viewport = Some(dimensions),
                Err(error) => app.error = Some(error),
            }
        }
        if event::poll(Duration::from_millis(50))? {
            match event::read()? {
                TerminalEvent::Key(key) => match app.input(key) {
                    Some(Effect::Refresh) => refresh(&mut app, &worker),
                    Some(Effect::Detach) => break,
                    Some(Effect::Session(request)) => {
                        if let Err(error) = client.send(request) {
                            app.shutdown_pending = false;
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

fn refresh(app: &mut App, worker: &GitWorker) {
    if app.begin_refresh()
        && let Err(error) = worker.refresh()
    {
        app.apply(Event::WorktreesLoaded(Err(error)));
    }
}
