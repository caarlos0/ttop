//! ttop — a tiny TUI that shows the processes running in every tmux pane.

mod app;
mod model;
mod tmux;
mod ui;

use std::io;
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{
    self, Event, KeyEventKind, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
    PushKeyboardEnhancementFlags,
};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::supports_keyboard_enhancement;

use app::App;

const TICK: Duration = Duration::from_millis(2000);

fn main() -> std::io::Result<()> {
    let mut app = App::new()?;
    let mut terminal = ratatui::init();
    let enhanced = enable_enhanced_keys();
    let result = run(&mut terminal, &mut app);
    if enhanced {
        let _ = execute!(io::stdout(), PopKeyboardEnhancementFlags);
    }
    ratatui::restore();
    result
}

/// Ask the terminal to disambiguate modified keys (e.g. Shift+Enter from Enter).
/// Returns whether the flags were pushed, so they can be popped on exit. Needs a
/// terminal that supports the keyboard-enhancement protocol — inside tmux that
/// also requires `set -g extended-keys on`; otherwise Shift+Enter falls back to
/// plain Enter.
fn enable_enhanced_keys() -> bool {
    supports_keyboard_enhancement().unwrap_or(false)
        && execute!(
            io::stdout(),
            PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
        )
        .is_ok()
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
