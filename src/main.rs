mod app;
mod events;
mod git;
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
    app::App,
    events::{Action, Event},
    git::GitWorker,
    terminal::TerminalGuard,
    ui::Ui,
};

fn main() -> Result<(), Box<dyn Error>> {
    let mut arguments = env::args_os().skip(1);
    let repository = match arguments.next() {
        Some(argument) if argument == "--help" || argument == "-h" => {
            println!(
                "Usage: agent-terminal [repository-path]\n\nLists Git worktrees in a keyboard-driven dashboard. Defaults to the current directory.\n\nj/k or arrows: select · r: refresh · q/Esc/Ctrl-C: quit\nAgent sessions are not implemented yet."
            );
            return Ok(());
        }
        Some(argument) => PathBuf::from(argument),
        None => env::current_dir()?,
    };
    if arguments.next().is_some() {
        return Err("Usage: agent-terminal [repository-path]".into());
    }
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err("The dashboard needs an interactive terminal. Use --help for usage.".into());
    }

    let worker = GitWorker::start(repository.clone())?;
    let mut app = App::new(repository);
    refresh(&mut app, &worker);
    let (_guard, mut terminal) = TerminalGuard::enter()?;
    let mut ui = Ui::default();

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
        terminal.draw(|frame| ui.draw(frame, &app))?;
        if event::poll(Duration::from_millis(50))? {
            match event::read()? {
                TerminalEvent::Key(key) => match events::action_for_key(key) {
                    Some(Action::Next) => app.next(),
                    Some(Action::Previous) => app.previous(),
                    Some(Action::Refresh) => refresh(&mut app, &worker),
                    Some(Action::Quit) => break,
                    None => {}
                },
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
