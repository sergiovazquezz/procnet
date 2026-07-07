use std::{
    io::{BufReader, Write},
    os::unix::net::UnixListener,
    sync::{
        Arc, Mutex,
        mpsc::{self, SyncSender, TrySendError},
    },
    thread,
    time::Duration,
};

use procnet_core::ipc::{self, DEFAULT_SOCKET_PATH, DaemonCommand};

use crate::{
    errors::{
        ListenerError::{self, StreamListPoison},
        UpdateError,
    },
    state::DaemonState,
};

type SenderList = Mutex<Vec<SyncSender<Arc<[u8]>>>>;

#[expect(clippy::needless_pass_by_value)]
pub fn run_listener(
    senders: &SenderList,
    daemon_state: Arc<DaemonState>,
) -> Result<!, ListenerError> {
    let _ = std::fs::remove_file(DEFAULT_SOCKET_PATH);

    let listener = UnixListener::bind(DEFAULT_SOCKET_PATH)?;

    for stream in listener.incoming() {
        match stream {
            Ok(mut s) => {
                s.set_read_timeout(Some(Duration::from_secs(2)))?;

                let mut reader = BufReader::new(&s);

                let msg = match ipc::read_msg(&mut reader) {
                    Ok(msg) => msg,
                    Err(e) => {
                        log::warn!("Dropping client: initial command read failed: {e}");
                        drop(s);
                        continue;
                    }
                };

                if msg != DaemonCommand::Run {
                    daemon_state.update(msg);
                    drop(s);
                    continue;
                }

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
