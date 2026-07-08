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

use procnet_core::ipc::{self, DaemonCommand};

use crate::{
    errors::{ListenerError, MutexPoison},
    state::DaemonState,
};

type SenderList = Vec<SyncSender<Arc<[u8]>>>;

#[expect(clippy::needless_pass_by_value)]
pub fn run_listener(
    senders: Arc<Mutex<SenderList>>,
    daemon_state: Arc<Mutex<DaemonState>>,
) -> Result<!, ListenerError> {
    let socket_path = ipc::socket_path();

    let _ = std::fs::remove_file(&socket_path);

    let listener = UnixListener::bind(&socket_path).map_err(|source| ListenerError::Bind {
        path: socket_path,
        source,
    })?;

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
                        // NOTE: If the Mutex is poisoned, `app::run()` will discover it on
                        // its next lock attempt and exit, so we ignore the error here.
                        if let Ok(mut guard) = daemon_state_clone.lock() {
                            guard.update(msg);
                        }

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

// NOTE: A `sender` will be evicted on the tick following the one in which the
// `receiver` was dropped.
pub fn update_streams(tx: &Mutex<SenderList>, bytes: &Arc<[u8]>) -> Result<(), MutexPoison> {
    tx.lock()
        .map_err(|_| MutexPoison)?
        .retain(|sender| match sender.try_send(Arc::clone(bytes)) {
            Ok(()) | Err(TrySendError::Full(_)) => true,
            Err(TrySendError::Disconnected(_)) => false,
        });

    Ok(())
}
