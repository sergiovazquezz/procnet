use std::time::Duration;

use procnet_core::stats::StatsRow;
use ratatui::{
    DefaultTerminal,
    crossterm::event::{self, Event, KeyEventKind},
};

use crate::errors::TuiError;

mod input;
mod state;
mod theme;
mod view;

pub use state::Action;
use state::TuiState;

pub struct Tui {
    terminal: DefaultTerminal,
    state: TuiState,
}

impl Tui {
    pub fn new() -> Self {
        let terminal = ratatui::init();
        Self {
            terminal,
            state: TuiState::new(),
        }
    }

    pub fn draw(&mut self, tick: u64, interval: u64, rows: &[StatsRow]) -> Result<(), TuiError> {
        self.terminal
            .draw(|frame| view::render(frame, tick, interval, rows, &mut self.state))
            .map_err(TuiError::Draw)?;

        Ok(())
    }

    pub const fn is_paused(&self) -> bool {
        self.state.paused
    }

    pub fn handle_event(&mut self, timeout: Duration) -> Result<Action, TuiError> {
        if !event::poll(timeout).map_err(TuiError::Event)? {
            return Ok(Action::None);
        }

        let Event::Key(key) = event::read().map_err(TuiError::Event)? else {
            return Ok(Action::None);
        };

        if key.kind != KeyEventKind::Press {
            return Ok(Action::None);
        }

        Ok(input::handle_key(&mut self.state, key))
    }
}

impl Drop for Tui {
    fn drop(&mut self) {
        ratatui::restore();
    }
}
