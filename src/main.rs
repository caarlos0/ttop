//! ttop — a tiny TUI that shows the processes running in every tmux pane.

mod app;
mod model;
mod tmux;
mod ui;

use std::time::{Duration, Instant};

use ratatui::crossterm::event::{self, Event, KeyEventKind};

use app::App;

const TICK: Duration = Duration::from_millis(2000);

fn main() -> std::io::Result<()> {
    let mut app = App::new()?;
    let mut terminal = ratatui::init();
    let result = run(&mut terminal, &mut app);
    ratatui::restore();
    result
}

fn run(terminal: &mut ratatui::DefaultTerminal, app: &mut App) -> std::io::Result<()> {
    let mut last = Instant::now();
    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        let timeout = TICK.saturating_sub(last.elapsed());
        if event::poll(timeout)?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            app.on_key(key);
        }
        if last.elapsed() >= TICK {
            app.refresh();
            last = Instant::now();
        }
        if app.should_quit {
            return Ok(());
        }
    }
}
