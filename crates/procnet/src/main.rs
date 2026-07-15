use std::{
    io::BufReader,
    sync::mpsc::{self, TryRecvError},
    thread,
    time::Duration,
};

use clap::Parser;
use procnet_core::ipc::{self, DaemonCommand, SnapshotData};

use crate::{
    cli::{Cli, Command},
    errors::ClientError,
    tui::{Action, Tui},
};

mod cli;
mod errors;
mod tui;

fn main() -> Result<(), ClientError> {
    let args = Cli::parse();

    let mut stream = ipc::client::connect_to_socket()?;
    let stream_clone = stream.try_clone()?;

    match args.command {
        Some(Command::Run) | None => cli::send_daemon_command(DaemonCommand::Run, &mut stream)?,
        Some(Command::Daemon { command }) => {
            cli::send_daemon_command(command, &mut stream)?;
            return Ok(());
        }
    }

    let (snap_tx, snap_rx) = mpsc::channel::<SnapshotData>();

    let mut snap = SnapshotData::default();

    let mut tui = Tui::new();

    let join_handle = thread::spawn(move || {
        let mut reader = BufReader::new(stream_clone);

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
                Ok(new_snap) => {
                    if !tui.is_paused() {
                        snap = new_snap;
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => match join_handle.join() {
                    Ok(()) => return Err(ClientError::DaemonHangup),
                    Err(_) => return Err(ClientError::ThreadPanic),
                },
            }
        }

        tui.draw(&snap)?;

        match tui.handle_event(Duration::from_millis(250), &mut stream)? {
            Action::Quit => break,
            Action::Redraw => tui.draw(&snap)?,
            Action::None => {}
        }
    }

    Ok(())
}
