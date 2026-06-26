mod input;
mod state;
mod theme;
mod view;

use std::time::Duration;

use anyhow::{Context, Result};
use ratatui::{
    DefaultTerminal,
    crossterm::event::{self, Event, KeyEventKind},
};

use procnet_core::stats::StatsRow;

pub use state::Action;
use state::TuiState;

pub struct Tui {
    terminal: DefaultTerminal,
    state: TuiState,
}

impl Tui {
    pub fn new() -> Result<Self> {
        let terminal = ratatui::try_init().context("failed to initialize terminal UI")?;
        Ok(Self {
            terminal,
            state: TuiState::new(),
        })
    }

    pub fn draw(&mut self, tick: u64, rows: &[StatsRow]) -> Result<()> {
        self.terminal
            .draw(|frame| view::render(frame, tick, rows, &self.state))
            .context("failed to draw stats UI")?;

        Ok(())
    }

    pub fn handle_event(&mut self, timeout: Duration) -> Result<Action> {
        if !event::poll(timeout)? {
            return Ok(Action::None);
        }

        let key = match event::read()? {
            Event::Key(k) => k,
            _ => return Ok(Action::None),
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
