mod tui;

use std::{io::BufReader, sync::mpsc, thread, time::Duration};

use anyhow::Result;
use procnet_core::{
    ipc::{self, Message, SnapshotData},
    stats::StatsRow,
};

use crate::tui::{Action, Tui};

fn main() -> Result<()> {
    let stream = ipc::connect_to_socket()?;

    let (snap_tx, snap_rx) = mpsc::channel::<SnapshotData>();

    let mut tui = Tui::new()?;

    let mut rows: Vec<StatsRow> = Vec::with_capacity(20);

    thread::spawn(move || {
        let mut reader = BufReader::new(stream);

        loop {
            match ipc::read_msg(&mut reader) {
                Ok(Message::Snapshot(s)) => {
                    if snap_tx.send(s).is_err() {
                        break;
                    }
                }
                Ok(Message::Error(e)) => {
                    log::error!("{}", e);
                    break;
                }
                Err(e) => {
                    log::error!("{}", e);
                    break;
                }
            };
        }
    });

    loop {
        loop {
            match snap_rx.try_recv() {
                Ok(snap) => rows = snap.rows,
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    // TODO: replace with some visual in TUI
                    return Ok(());
                }
            }
        }

        tui.draw(&rows)?;

        match tui.handle_event(Duration::from_millis(250))? {
            Action::Quit => break,
            Action::Redraw => tui.draw(&rows)?,
            Action::None => {}
        }
    }

    Ok(())
}
