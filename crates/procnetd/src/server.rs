use std::{
    os::unix::net::{UnixListener, UnixStream},
    sync::{Arc, Mutex, mpsc::Sender},
};

use procnet_core::ipc::{self, DEFAULT_SOCKET_PATH, Message};

use crate::errors::{ListenerError, UpdateError};

#[expect(clippy::needless_pass_by_value)]
pub fn run_listener(stream_list: Arc<Mutex<Vec<UnixStream>>>, tx: Sender<ListenerError>) {
    let _ = std::fs::remove_file(DEFAULT_SOCKET_PATH);

    let listener = match UnixListener::bind(DEFAULT_SOCKET_PATH) {
        Ok(l) => l,
        Err(e) => {
            let _ = tx.send(ListenerError::Bind(e));
            return;
        }
    };

    for stream in listener.incoming() {
        match stream {
            Ok(s) => {
                if let Ok(mut list) = stream_list.lock() {
                    list.push(s);
                } else {
                    let _ = tx.send(ListenerError::StreamListPoison);
                    return;
                }
            }
            Err(e) => {
                log::debug!("error accepting connection {}", e);
            }
        }
    }
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
