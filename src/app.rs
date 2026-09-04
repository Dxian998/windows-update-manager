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
    pub protect_service_settings: bool,
    pub status_details: Vec<(String, String)>,
    pub busy: Arc<Mutex<bool>>,
    pub verified_status: Arc<Mutex<Option<bool>>>,
    last_key_press_time: Instant,
    last_status_refresh: Instant,
}

impl App {
    pub fn new() -> Self {
        let mut menu_state = ListState::default();
        menu_state.select(Some(0));

        let (is_blocked, status_details) = update::get_update_status();
        let locked = crate::security::is_registry_key_locked("wuauserv");
        let protect_service_settings = if is_blocked { locked } else { true };

        Self {
            menu_state,
            update_blocked: is_blocked,
            protect_service_settings,
            status_details,
            busy: Arc::new(Mutex::new(false)),
            verified_status: Arc::new(Mutex::new(None)),
            last_key_press_time: Instant::now(),
            last_status_refresh: Instant::now(),
        }
    }

    pub fn refresh_status(&mut self) {
        let (is_blocked, status_details) = update::get_update_status();
        self.update_blocked = is_blocked;
        self.status_details = status_details;
        self.last_status_refresh = Instant::now();
    }

    pub fn run(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    ) -> io::Result<()> {
        loop {
            let verified = self.verified_status.lock().unwrap().take();
            if let Some(verified) = verified {
                self.update_blocked = verified;
                self.refresh_status();
            }

            if self.last_status_refresh.elapsed() >= Duration::from_millis(1500) {
                self.refresh_status();
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
        let protect = self.protect_service_settings;

        {
            let mut lock = busy.lock().unwrap();
            *lock = true;
        }

        self.update_blocked = should_block;

        thread::spawn(move || {
            if should_block {
                update::block_updates(protect);
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

    pub fn toggle_protect_settings(&mut self) {
        self.protect_service_settings = !self.protect_service_settings;
        if self.update_blocked {
            update::set_protect_service_settings(self.protect_service_settings);
        }
        self.refresh_status();
    }

    pub fn toggle_bits(&mut self) {
        update::toggle_bits(self.update_blocked);
        self.refresh_status();
    }

    pub fn open_github(&self) {
        let _ = open::that("https://github.com/Dxian998/windows-update-manager");
    }
}
