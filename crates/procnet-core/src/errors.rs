use thiserror::Error;

use crate::ipc::DEFAULT_SOCKET_PATH;

#[derive(Error, Debug)]
#[error(
    "Failed to connect to socket at {DEFAULT_SOCKET_PATH}: {0}\n
    Hint: is the daemon running?"
)]
pub struct ConnectError(#[from] std::io::Error);

#[derive(Error, Debug)]
pub enum MsgSendError {
    #[error("Io: {0}")]
    Io(#[from] std::io::Error),
    #[error("Encode: {0}")]
    Encode(#[from] bincode::Error),
    #[error("Serialized payload exceeds the u32 length-header limit")]
    Oversized,
}

#[derive(Debug, thiserror::Error)]
pub enum MsgReadError {
    #[error("EOF reached, daemon closed")]
    Eof,
    #[error("Io: {0}")]
    Io(#[from] std::io::Error),
    #[error("Decode: {0}")]
    Decode(#[from] bincode::Error),
}
