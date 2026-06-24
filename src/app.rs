use std::time::{Duration, Instant};

use anyhow::Result;
use libbpf_rs::MapMut;

use crate::{
    events::EventReader,
    stats::{StatsCollector, StatsRow},
    tui::{Action, Tui},
};

pub fn run(stats_map: &MapMut, events_map: &MapMut) -> Result<()> {
    let refresh_interval = Duration::from_secs(1);
    let mut next_draw_at = Instant::now();

    let mut stats = StatsCollector::new();
    let mut events = EventReader::new(events_map)?;
    let mut tui = Tui::new()?;

    let mut rows: Vec<StatsRow> = Vec::with_capacity(20);

    loop {
        if Instant::now() >= next_draw_at {
            for event in events.drain_available()? {
                stats.apply_event(event);
            }

            stats.collect_rows(stats_map, &mut rows);

            tui.draw(&rows)?;
            next_draw_at = Instant::now() + refresh_interval;
        }

        let timeout = next_draw_at
            .saturating_duration_since(Instant::now())
            .min(Duration::from_millis(250));

        match tui.handle_event(timeout)? {
            Action::Quit => break,
            Action::Redraw => {
                tui.draw(&rows)?;
            }
            Action::None => {}
        }
    }

    Ok(())
}
