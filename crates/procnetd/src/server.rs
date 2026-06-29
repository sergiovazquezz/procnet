use std::{
    os::unix::net::{UnixListener, UnixStream},
    sync::Mutex,
};

use procnet_core::ipc::{self, DEFAULT_SOCKET_PATH, Message};

use crate::errors::{ListenerError, UpdateError};

pub fn run_listener(stream_list: &Mutex<Vec<UnixStream>>) -> Result<!, ListenerError> {
    let _ = std::fs::remove_file(DEFAULT_SOCKET_PATH);

    let listener = UnixListener::bind(DEFAULT_SOCKET_PATH)?;

    for stream in listener.incoming() {
        match stream {
            Ok(s) => {
                if let Ok(mut list) = stream_list.lock() {
                    list.push(s);
                } else {
                    return Err(ListenerError::StreamListPoison);
                }
            }
            Err(e) => {
                log::debug!("error accepting connection {}", e);
            }
        }
    }

    unreachable!("UnixListener::incoming() never terminates")
}

pub fn update_streams(
    stream_list: &Mutex<Vec<UnixStream>>,
    msg: &Message,
) -> Result<(), UpdateError> {
    stream_list
        .lock()
        .map_err(|_| UpdateError)?
        .retain_mut(|stream| ipc::send_msg(stream, msg).is_ok());

    Ok(())
}
