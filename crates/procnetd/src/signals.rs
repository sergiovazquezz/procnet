use std::{fs, path::PathBuf, sync::mpsc::Sender, thread};

use nix::sys::signal::{SigSet, Signal};

/// Installs a signal handler that removes the socket file and signals shutdown
/// on `SIGINT` or `SIGTERM`.
///
/// Must be called before any other thread is spawned so that the blocked
/// signal mask (set via `thread_block()` -> `pthread_sigmask()`) is inherited
/// by all child threads. That guarantees `sigwait` in the spawned thread is the
/// sole delivery point for these signals.
pub fn install_signal_handler(
    socket_path: PathBuf,
    shutdown_tx: Sender<()>,
) -> Result<(), nix::Error> {
    let mut mask = SigSet::empty();
    mask.add(Signal::SIGINT);
    mask.add(Signal::SIGTERM);

    // Block process-wide, inherited by future threads.
    mask.thread_block()?;

    thread::spawn(move || {
        loop {
            match mask.wait() {
                Ok(sig) => {
                    log::info!("Received {sig}, removing socket and exiting");

                    if let Err(e) = fs::remove_file(&socket_path) {
                        log::warn!("Failed to remove socket {}: {e}", socket_path.display());
                    }

                    let _ = shutdown_tx.send(());

                    break;
                }
                Err(e) => {
                    log::error!("Sigwait failed, signal handler thread retrying: {e}");
                }
            }
        }
    });

    Ok(())
}
