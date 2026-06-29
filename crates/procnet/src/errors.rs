use std::io;

use procnet_core::errors::ConnectError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ClientError {
    #[error("Failed to connect to socket")]
    Ipc(#[from] ConnectError),
    #[error("Failed to draw or handle TUI")]
    Tui(#[from] TuiError),
    #[error("The daemon is not responding")]
    DaemonHangup,
    #[error("Reciever thread has panicked")]
    ThreadPanic,
}

#[derive(Error, Debug)]
pub enum TuiError {
    #[error("Failed to draw TUI: {0}")]
    Draw(#[source] io::Error),
    #[error("Failed to poll or read event: {0}")]
    Event(#[source] io::Error),
}
