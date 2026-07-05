use std::{
    io::Write,
    os::unix::net::{UnixListener, UnixStream},
    sync::Mutex,
    time::Duration,
};

use procnet_core::ipc::DEFAULT_SOCKET_PATH;

use crate::errors::{ListenerError, UpdateError};

pub fn run_listener(stream_list: &Mutex<Vec<UnixStream>>) -> Result<!, ListenerError> {
    let _ = std::fs::remove_file(DEFAULT_SOCKET_PATH);

    let listener = UnixListener::bind(DEFAULT_SOCKET_PATH)?;

    for stream in listener.incoming() {
        match stream {
            Ok(s) => {
                if let Ok(mut list) = stream_list.lock() {
                    s.set_write_timeout(Some(Duration::from_millis(200)))?;

                    list.push(s);
                } else {
                    return Err(ListenerError::StreamListPoison);
                }
            }
            Err(e) => {
                log::warn!("error accepting connection {}", e);
            }
        }
    }

    unreachable!("UnixListener::incoming() never terminates")
}

pub fn update_streams(
    stream_list: &Mutex<Vec<UnixStream>>,
    bytes: &[u8],
) -> Result<(), UpdateError> {
    stream_list
        .lock()
        .map_err(|_| UpdateError)?
        .retain_mut(|stream| stream.write_all(bytes).is_ok());

    Ok(())
}
