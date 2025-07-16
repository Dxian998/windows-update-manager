use crate::{ui, update};
use crossterm::event::Event;
use std::{
    io,
    process::Command,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};
use tui::{Terminal, backend::CrosstermBackend, widgets::ListState};

const DEBOUNCE_DELAY: u64 = 150;

pub struct App {
    pub menu_state: ListState,
    pub update_blocked: bool,
    pub busy: Arc<Mutex<bool>>,
    last_key_press_time: Instant,
}

impl App {
    pub fn new() -> Self {
        let mut menu_state = ListState::default();
        menu_state.select(Some(0));

        Self {
            menu_state,
            update_blocked: update::check_update_status(),
            busy: Arc::new(Mutex::new(false)),
            last_key_press_time: Instant::now(),
        }
    }

    pub fn run(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    ) -> io::Result<()> {
        loop {
            terminal.draw(|f| ui::render(f, self))?;

            if crossterm::event::poll(Duration::from_millis(50))? {
                if let Event::Key(key) = crossterm::event::read()? {
                    if self.last_key_press_time.elapsed() < Duration::from_millis(DEBOUNCE_DELAY) {
                        continue;
                    }
                    self.last_key_press_time = Instant::now();

                    if !ui::handle_key_event(key, self) {
                        break;
                    }
                }
            }
        }
        Ok(())
    }

    pub fn toggle_updates(&mut self) {
        let busy = Arc::clone(&self.busy);
        let should_block = !self.update_blocked;

        {
            let mut busy_lock = busy.lock().unwrap();
            *busy_lock = true;
        }

        let _self_update_blocked = self.update_blocked;
        thread::spawn(move || {
            if should_block {
                update::block_updates();
            } else {
                update::enable_updates();
            }

            let _new_status = update::check_update_status();

            {
                let mut busy_lock = busy.lock().unwrap();
                *busy_lock = false;
            }
        });

        self.update_blocked = should_block;
    }

    // This function links to the official GitHub repository and should not be modified directly.
    // It aligns with the MIT License, which requires attribution. To propose changes, please open up a pull request.
    pub fn open_github(&self) {
        let _ = Command::new("cmd")
            .args(&[
                "/C",
                "start",
                "https://github.com/0xSovereign/windows-update-manager",
            ])
            .spawn();
    }
}
