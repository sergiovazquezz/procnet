use std::{
    io::{self, BufReader, Write},
    os::unix::net::{UnixListener, UnixStream},
    path::Path,
    sync::{
        Arc, Mutex,
        mpsc::{self, SyncSender, TrySendError},
    },
    thread,
    time::Duration,
};

use procnet_core::{
    errors::MsgReadError,
    ipc::{self, DaemonCommand},
};

use crate::{
    errors::{ListenerError, MutexPoison},
    state::DaemonState,
};

type SenderList = Vec<SyncSender<Arc<[u8]>>>;

/// Binds the IPC socket, removing a stale leftover file from a previously
/// crashed daemon if no live daemon is currently listening.
pub fn bind_unix_listener(path: &Path) -> Result<UnixListener, ListenerError> {
    match UnixListener::bind(path) {
        Ok(listener) => Ok(listener),
        Err(e) if e.kind() == io::ErrorKind::AddrInUse => probe_and_bind(path),
        Err(source) => Err(ListenerError::Bind {
            path: path.to_path_buf(),
            source,
        }),
    }
}

/// Probes the path with a `connect` to decide whether the file is a stale
/// leftover (safe to remove and retry) or a live daemon we must not clobber.
fn probe_and_bind(path: &Path) -> Result<UnixListener, ListenerError> {
    match UnixStream::connect(path) {
        Ok(_) => Err(ListenerError::InUse {
            path: path.to_path_buf(),
        }),
        Err(ref e) if e.kind() == io::ErrorKind::PermissionDenied => Err(ListenerError::InUse {
            path: path.to_path_buf(),
        }),
        Err(ref e)
            if e.kind() == io::ErrorKind::ConnectionRefused
                || e.kind() == io::ErrorKind::NotFound =>
        {
            log::info!("Removing stale socket {}", path.display());

            let _ = std::fs::remove_file(path);

            UnixListener::bind(path).map_err(|source| ListenerError::Bind {
                path: path.to_path_buf(),
                source,
            })
        }
        Err(source) => Err(ListenerError::Bind {
            path: path.to_path_buf(),
            source,
        }),
    }
}

pub fn accept_loop(
    listener: UnixListener,
    senders: Arc<Mutex<SenderList>>,
    daemon_state: Arc<Mutex<DaemonState>>,
) -> Result<!, ListenerError> {
    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                let senders_clone = Arc::clone(&senders);
                let daemon_state_clone = Arc::clone(&daemon_state);

                thread::spawn(move || {
                    let Ok(stream_clone) = stream.try_clone() else {
                        log::warn!("Failed to clone stream on incoming connection");
                        return;
                    };

                    if let Err(e) = stream_clone.set_read_timeout(Some(Duration::from_secs(2))) {
                        log::warn!("Failed to set read timeout on incoming connection: {e}");
                        return;
                    }

                    let mut reader = BufReader::new(stream_clone);

                    let msg = match ipc::read_msg(&mut reader) {
                        Ok(msg) => msg,
                        Err(e) => {
                            log::warn!(
                                "Dropping client: initial command read failed (normally a new daemon instance probing): {e}"
                            );
                            return;
                        }
                    };

                    if msg != DaemonCommand::Run {
                        // NOTE: If the Mutex is poisoned, `app::run()` will discover it on
                        // its next lock attempt and exit, so it's ignored here.
                        if let Ok(mut guard) = daemon_state_clone.lock() {
                            guard.update(msg);
                        }

                        return;
                    }

                    // Remove the read timeout used for CLI's
                    let stream_clone = reader.into_inner();
                    if let Err(e) = stream_clone.set_read_timeout(None) {
                        log::warn!("Failed to set read timeout on incoming connection: {e}");
                        return;
                    }

                    let mut reader = BufReader::new(stream_clone);

                    thread::spawn(move || {
                        loop {
                            match ipc::read_msg(&mut reader) {
                                Ok(msg) => {
                                    if let Ok(mut guard) = daemon_state_clone.lock() {
                                        guard.update(msg);
                                    } else {
                                        break;
                                    }
                                }
                                Err(MsgReadError::Eof) => break,
                                Err(e) => {
                                    log::warn!("Could not read message from TUI: {e}");
                                    break;
                                }
                            }
                        }
                    });

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
                            log::debug!(
                                "Client writer write_all failed (happens on client exits): {e}"
                            );
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

#[cfg(test)]
#[expect(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use std::{
        env,
        path::PathBuf,
        process,
        sync::atomic::{AtomicU64, Ordering},
    };

    fn tmp_path() -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);

        env::temp_dir().join(format!("procnetd-bind-test-{}-{id}.sock", process::id()))
    }

    #[test]
    fn bind_unix_listener_refuses_when_already_in_use() {
        let path = tmp_path();
        let _ = std::fs::remove_file(&path);
        let existing = UnixListener::bind(&path).unwrap();

        match bind_unix_listener(&path) {
            Err(ListenerError::InUse { .. }) => {}
            other => panic!("expected InUse, got {other:?}"),
        }

        drop(existing);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn bind_unix_listener_reclaims_stale_socket() {
        let path = tmp_path();
        let _ = std::fs::remove_file(&path);
        // Bind a listener, then drop it *without* removing the file. The socket
        // file remains on disk as a stale leftover from a "crashed" daemon.
        {
            let _listener = UnixListener::bind(&path).unwrap();
        }
        assert!(path.exists(), "stale socket file should be present");

        // Should detect the stale leftover, unlink it and succeed.
        let _listener = match bind_unix_listener(&path) {
            Ok(l) => l,
            other => panic!("expected Ok, got {other:?}"),
        };

        let _ = std::fs::remove_file(&path);
    }
}
