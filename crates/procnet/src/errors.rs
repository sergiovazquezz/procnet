use std::io;

use procnet_core::errors::{ConnectError, MsgSendError};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ClientError {
    #[error("Socket connection error: {0}")]
    SocketConnection(#[from] ConnectError),
    #[error("Could not clone socket: {0}")]
    StreamClone(#[from] io::Error),
    #[error("{0}")]
    MsgSendError(#[from] MsgSendError),
    #[error("Tui error: {0}")]
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
