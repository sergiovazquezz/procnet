use std::io;

use procnet_core::{errors::MsgSendError, ipc::DEFAULT_SOCKET_PATH};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DaemonError {
    #[error("An event error happened: {0}")]
    Event(#[from] EventError),
    #[error("A listener error happened: {0}")]
    ListenerError(#[from] ListenerError),
    #[error("A stream update error happened: {0}")]
    MutexPoison(#[from] MutexPoison),
    #[error("Failed to serialize snapshot for clients: {0}")]
    Serialize(#[from] MsgSendError),
    #[error("Listener thread exited unexpectedly (likely panic)")]
    ThreadPanic,
}

#[derive(Error, Debug)]
pub enum EventError {
    #[error("Failed to build event ringbuf: {0}")]
    Build(#[source] libbpf_rs::Error),
    #[error("Failed to consume event ringbuf: {0}")]
    Consume(#[source] libbpf_rs::Error),
}

#[derive(Error, Debug)]
pub enum ListenerError {
    #[error("Failed to bind {DEFAULT_SOCKET_PATH}: {0}")]
    Bind(#[from] io::Error),
}

#[derive(Error, Debug)]
#[error("Mutex lock has been poisoned")]
pub struct MutexPoison;
