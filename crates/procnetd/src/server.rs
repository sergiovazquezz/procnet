use std::{
    io::Write,
    os::unix::net::UnixListener,
    sync::{
        Arc, Mutex,
        mpsc::{self, SyncSender},
    },
    thread,
    time::Duration,
};

use procnet_core::ipc::DEFAULT_SOCKET_PATH;

use crate::errors::{
    ListenerError::{self, StreamListPoison},
    UpdateError,
};

type SenderList = Mutex<Vec<SyncSender<Arc<[u8]>>>>;

pub fn run_listener(senders: &SenderList) -> Result<!, ListenerError> {
    let _ = std::fs::remove_file(DEFAULT_SOCKET_PATH);

    let listener = UnixListener::bind(DEFAULT_SOCKET_PATH)?;

    for stream in listener.incoming() {
        match stream {
            Ok(mut s) => {
                s.set_write_timeout(Some(Duration::from_millis(200)))?;

                let (tx, rx) = mpsc::sync_channel::<Arc<[u8]>>(4);
                senders.lock().map_err(|_| StreamListPoison)?.push(tx);

                thread::spawn(move || {
                    while let Ok(data) = rx.recv() {
                        if let Err(e) = s.write_all(&data) {
                            log::debug!("Client writer write_all failed: {e}");
                            break;
                        }
                    }

                    log::debug!("Client writer exiting (channel closed)");
                });
            }
            Err(e) => {
                log::warn!("Error accepting connection {e}");
            }
        }
    }

    unreachable!("UnixListener::incoming() never terminates")
}

pub fn update_streams(tx: &SenderList, bytes: &Arc<[u8]>) -> Result<(), UpdateError> {
    tx.lock()
        .map_err(|_| UpdateError)?
        .retain_mut(|s| s.try_send(Arc::clone(bytes)).is_ok());

    Ok(())
}
