use std::time::Duration;

use anyhow::{Context, Result};
use ratatui::{
    DefaultTerminal, Frame,
    crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    layout::{Constraint, Layout},
    style::{Modifier, Style},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
};

use crate::stats::StatsRow;

pub struct Tui {
    terminal: DefaultTerminal,
}

impl Tui {
    pub fn new() -> Result<Self> {
        let terminal = ratatui::try_init().context("failed to initialize terminal UI")?;
        Ok(Self { terminal })
    }

    fn render(frame: &mut Frame, rows: &[StatsRow]) {
        let layout =
            Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).split(frame.area());

        if rows.is_empty() {
            frame.render_widget(
                Paragraph::new("Waiting for process stats...")
                    .block(Block::default().borders(Borders::ALL)),
                layout[0],
            );
        } else {
            let table_rows = rows.iter().map(|row| {
                Row::new([
                    Cell::from(row.pid.to_string()),
                    Cell::from(row.name.as_ref()),
                    Cell::from(row.sent_bytes.to_string()),
                    Cell::from(row.recv_bytes.to_string()),
                    Cell::from(row.total_bytes.to_string()),
                ])
            });

            let table = Table::new(
                table_rows,
                [
                    Constraint::Length(8),
                    Constraint::Min(12),
                    Constraint::Length(14),
                    Constraint::Length(14),
                    Constraint::Length(14),
                    Constraint::Length(8),
                ],
            )
            .header(
                Row::new(["PID", "Name", "Sent", "Received", "Total", "Conns"])
                    .style(Style::default().add_modifier(Modifier::BOLD)),
            )
            .block(Block::default().borders(Borders::ALL))
            .column_spacing(1);

            frame.render_widget(table, layout[0]);
        }

        frame.render_widget(Paragraph::new("Ctrl-C or q to quit"), layout[1]);
    }

    pub fn draw(&mut self, rows: &[StatsRow]) -> Result<()> {
        self.terminal
            .draw(|frame| Tui::render(frame, rows))
            .context("failed to draw stats UI")?;

        Ok(())
    }

    pub fn should_quit(&self, timeout: Duration) -> Result<bool> {
        if event::poll(timeout).context("failed to poll terminal events")?
            && let Event::Key(key) = event::read().context("failed to read terminal event")?
            && key.kind == KeyEventKind::Press
        {
            let is_quit_key = matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q'))
                || (matches!(key.code, KeyCode::Char('c') | KeyCode::Char('C'))
                    && key.modifiers.contains(KeyModifiers::CONTROL));

            if is_quit_key {
                return Ok(true);
            }
        }

        Ok(false)
    }
}

impl Drop for Tui {
    fn drop(&mut self) {
        ratatui::restore();
    }
}
