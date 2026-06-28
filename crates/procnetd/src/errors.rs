use thiserror::Error;

use procnet_core::ipc::DEFAULT_SOCKET_PATH;

#[derive(Error, Debug)]
pub enum ListenerError {
    #[error("Failed to bind {DEFAULT_SOCKET_PATH}: {0}")]
    Bind(std::io::Error),
    #[error("Mutex lock has been poisoned")]
    StreamListPoison,
}

#[derive(Error, Debug)]
#[error("Mutex lock has been poisoned")]
pub struct UpdateError;
