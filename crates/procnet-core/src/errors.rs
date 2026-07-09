use std::{io, path::PathBuf};

use thiserror::Error;

#[derive(Error, Debug)]
#[error("Failed to connect to socket at {}: {source}\nHint: is the daemon running?", path.display())]
pub struct ConnectError {
    path: PathBuf,
    source: io::Error,
}

impl ConnectError {
    #[must_use]
    pub fn new(path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self {
            path: path.into(),
            source,
        }
    }
}

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
