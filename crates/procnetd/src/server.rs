use std::{
    io::{BufReader, Write},
    os::unix::net::UnixListener,
    sync::{
        Arc, Mutex,
        mpsc::{self, Sender, SyncSender, TrySendError},
    },
    thread,
    time::Duration,
};

use procnet_core::ipc::{self, DEFAULT_SOCKET_PATH, DaemonCommand};

use crate::errors::{
    ListenerError::{self, StreamListPoison},
    UpdateError,
};

type SenderList = Mutex<Vec<SyncSender<Arc<[u8]>>>>;

#[expect(clippy::needless_pass_by_value)]
pub fn run_listener(
    senders: &SenderList,
    config_tx: Sender<DaemonCommand>,
) -> Result<!, ListenerError> {
    let _ = std::fs::remove_file(DEFAULT_SOCKET_PATH);

    let listener = UnixListener::bind(DEFAULT_SOCKET_PATH)?;

    for stream in listener.incoming() {
        match stream {
            Ok(mut s) => {
                s.set_read_timeout(Some(Duration::from_secs(1)))?;
                let mut reader = BufReader::new(&s);

                if let Ok(result) = ipc::read_msg(&mut reader) {
                    let _ = config_tx.send(result);
                    drop(s);
                    continue;
                }

                s.set_read_timeout(None)?;
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
        .retain(|sender| match sender.try_send(Arc::clone(bytes)) {
            Ok(()) | Err(TrySendError::Full(_)) => true,
            Err(TrySendError::Disconnected(_)) => false,
        });

    Ok(())
}
