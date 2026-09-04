use crate::{ui, update};
use crossterm::event::Event;
use std::{
    io,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};
use tui::{Terminal, backend::CrosstermBackend, widgets::ListState};
use open;

const DEBOUNCE_DELAY: u64 = 150;

pub struct App {
    pub menu_state: ListState,
    pub update_blocked: bool,
    pub busy: Arc<Mutex<bool>>,
    pub verified_status: Arc<Mutex<Option<bool>>>,
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
            verified_status: Arc::new(Mutex::new(None)),
            last_key_press_time: Instant::now(),
        }
    }

    pub fn run(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    ) -> io::Result<()> {
        loop {
            if let Some(verified) = self.verified_status.lock().unwrap().take() {
                self.update_blocked = verified;
            }

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
        let verified_status = Arc::clone(&self.verified_status);
        let should_block = !self.update_blocked;

        {
            let mut lock = busy.lock().unwrap();
            *lock = true;
        }

        self.update_blocked = should_block;

        thread::spawn(move || {
            if should_block {
                update::block_updates();
            } else {
                update::enable_updates();
            }

            let actual = update::check_update_status();
            {
                let mut vs = verified_status.lock().unwrap();
                *vs = Some(actual);
            }
            {
                let mut lock = busy.lock().unwrap();
                *lock = false;
            }
        });
    }

    // 69
    pub fn open_github(&self) {
        let _ = open::that("https://github.com/Dxian998/windows-update-manager");
    }
}
