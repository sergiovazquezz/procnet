use std::{
    fs::set_permissions,
    os::unix::{
        fs::PermissionsExt,
        net::{UnixListener, UnixStream},
    },
    sync::{Arc, Mutex, mpsc::Sender},
};

use anyhow::{Result, anyhow};
use log::debug;
use procnet_core::ipc::{self, DEFAULT_SOCKET_PATH, Message};

pub fn run_listener(stream_list: Arc<Mutex<Vec<UnixStream>>>, tx: Sender<String>) {
    let _ = std::fs::remove_file(DEFAULT_SOCKET_PATH);

    let listener = match UnixListener::bind(DEFAULT_SOCKET_PATH) {
        Ok(l) => {
            if let Err(e) =
                set_permissions(DEFAULT_SOCKET_PATH, std::fs::Permissions::from_mode(0o666))
            {
                let _ = tx.send(format!("chmod {}: {}", DEFAULT_SOCKET_PATH, e));
                return;
            }
            l
        }
        Err(e) => {
            let _ = tx.send(format!("bind {}: {}", DEFAULT_SOCKET_PATH, e));
            return;
        }
    };

    for stream in listener.incoming() {
        match stream {
            Ok(s) => {
                match stream_list.lock() {
                    Ok(mut list) => list.push(s),
                    Err(e) => {
                        let _ = tx.send(format!("failed to lock stream_list: {}", e));
                        return;
                    }
                };
            }
            Err(e) => {
                debug!("error accepting connection {}", e);
            }
        };
    }
}

pub fn update_streams(stream_list: &Mutex<Vec<UnixStream>>, msg: Message) -> Result<()> {
    let mut list = stream_list
        .lock()
        .map_err(|e| anyhow!("stream_list lock poisoned: {}", e))?;

    list.retain_mut(|stream| ipc::send_msg(stream, &msg).is_ok());

    Ok(())
}
