use std::{io::BufReader, sync::mpsc, thread, time::Duration};

use anyhow::{Result, anyhow};
use procnet_core::{
    ipc::{self, Message, SnapshotData},
    stats::StatsRow,
};

use crate::tui::{Action, Tui};

mod tui;

fn main() -> Result<()> {
    let (snap_tx, snap_rx) = mpsc::channel::<SnapshotData>();

    let mut rows: Vec<StatsRow> = Vec::with_capacity(20);
    let mut tick: u64 = 0;

    let stream = ipc::connect_to_socket()?;

    let mut tui = Tui::new()?;

    thread::spawn(move || {
        let mut reader = BufReader::new(stream);

        loop {
            while let Ok(Message::Snapshot(s)) = ipc::read_msg(&mut reader) {
                if snap_tx.send(s).is_err() {
                    break;
                }
            }
        }
    });

    loop {
        loop {
            match snap_rx.try_recv() {
                Ok(snap) => {
                    tick = snap.tick;
                    rows = snap.rows;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    return Err(anyhow!("The daemon is not responding"));
                }
            }
        }

        tui.draw(tick, &rows)?;

        match tui.handle_event(Duration::from_millis(250))? {
            Action::Quit => break,
            Action::Redraw => tui.draw(tick, &rows)?,
            Action::None => {}
        }
    }

    Ok(())
}
