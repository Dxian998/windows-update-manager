mod app;
mod privileges;
mod services;
mod ui;
mod update;

use app::App;
use crossterm::{execute, terminal::*};
use std::io;
use tui::{Terminal, backend::CrosstermBackend};

fn main() -> io::Result<()> {
    if !privileges::is_elevated()? {
        privileges::elevate();
        return Ok(());
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    app.run(&mut terminal)?;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}
