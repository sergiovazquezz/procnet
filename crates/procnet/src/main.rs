use std::{
    io::BufReader,
    sync::mpsc::{self, TryRecvError},
    thread,
    time::Duration,
};

use procnet_core::{
    ipc::{self, SnapshotData},
    stats::StatsRow,
};

use crate::{
    errors::ClientError,
    tui::{Action, Tui},
};

mod errors;
mod tui;

fn main() -> Result<(), ClientError> {
    let (snap_tx, snap_rx) = mpsc::channel::<SnapshotData>();

    let mut rows = Vec::<StatsRow>::with_capacity(20);

    let mut tick: u64 = 0;

    let stream = ipc::connect_to_socket()?;

    let mut tui = Tui::new();

    let join_handle = thread::spawn(move || {
        let mut reader = BufReader::new(stream);

        loop {
            match ipc::read_msg(&mut reader) {
                Ok(s) => {
                    if snap_tx.send(s).is_err() {
                        return;
                    }
                }
                Err(_) => return,
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
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => match join_handle.join() {
                    Ok(()) => return Err(ClientError::DaemonHangup),
                    Err(_) => return Err(ClientError::ThreadPanic),
                },
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
