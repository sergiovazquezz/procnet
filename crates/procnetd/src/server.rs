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
    errors::{ListenerError, MutexPoison},
    state::DaemonState,
};

type SenderList = Mutex<Vec<SyncSender<Arc<[u8]>>>>;

#[expect(clippy::needless_pass_by_value)]
pub fn run_listener(
    senders: Arc<SenderList>,
    daemon_state: Arc<DaemonState>,
) -> Result<!, ListenerError> {
    let _ = std::fs::remove_file(DEFAULT_SOCKET_PATH);

    let listener = UnixListener::bind(DEFAULT_SOCKET_PATH)?;

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let senders_clone = Arc::clone(&senders);
                let daemon_state_clone = Arc::clone(&daemon_state);

                thread::spawn(move || {
                    if let Err(e) = stream.set_read_timeout(Some(Duration::from_secs(2))) {
                        log::warn!("Failed to set read timeout on incoming connection: {e}");
                        return;
                    }

                    let mut reader = BufReader::new(stream);

                    let msg = match ipc::read_msg(&mut reader) {
                        Ok(msg) => msg,
                        Err(e) => {
                            log::warn!("Dropping client: initial command read failed: {e}");
                            return;
                        }
                    };

                    if msg != DaemonCommand::Run {
                        daemon_state_clone.update(msg);
                        return;
                    }

                    let mut stream = reader.into_inner();

                    if let Err(e) = stream.set_write_timeout(Some(Duration::from_millis(200))) {
                        log::warn!("Failed to set write timeout on incoming connection: {e}");
                        return;
                    }

                    let (tx, rx) = mpsc::sync_channel::<Arc<[u8]>>(4);

                    match senders_clone.lock() {
                        Ok(mut guard) => guard.push(tx),
                        Err(_) => return,
                    }

                    while let Ok(data) = rx.recv() {
                        if let Err(e) = stream.write_all(&data) {
                            log::debug!("Client writer write_all failed: {e}");
                            break;
                        }
                    }

                    log::info!("Client writer exiting (channel closed)");
                });
            }
            Err(e) => {
                log::warn!("Error accepting connection {e}");
            }
        }
    }

    unreachable!("UnixListener::incoming() never terminates")
}

// NOTE: A `sender` will be evicted on the tick following the one in which the
// `receiver` was dropped.
pub fn update_streams(tx: &SenderList, bytes: &Arc<[u8]>) -> Result<(), MutexPoison> {
    tx.lock()
        .map_err(|_| MutexPoison)?
        .retain(|sender| match sender.try_send(Arc::clone(bytes)) {
            Ok(()) | Err(TrySendError::Full(_)) => true,
            Err(TrySendError::Disconnected(_)) => false,
        });

    Ok(())
}
