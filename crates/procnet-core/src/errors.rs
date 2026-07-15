use std::io;

use thiserror::Error;

#[derive(Error, Debug)]
#[error("Failed to connect to socket: {0}\nHint: is the daemon running?")]
pub struct ConnectError(#[from] pub io::Error);

#[derive(Debug, Error)]
pub enum MsgSendError {
    #[error("Io: {0}")]
    Io(#[from] std::io::Error),
    #[error("Encode: {0}")]
    Encode(#[from] bincode::Error),
    #[error("Serialized payload exceeds the u16 length-header limit")]
    Oversized,
}

#[derive(Debug, Error)]
pub enum MsgReadError {
    #[error("EOF reached")]
    Eof,
    #[error("Io: {0}")]
    Io(#[from] std::io::Error),
    #[error("Decode: {0}")]
    Decode(#[from] bincode::Error),
}
