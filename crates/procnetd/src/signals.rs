use std::{fs, process, thread};

use nix::sys::signal::{SigSet, Signal};

/// Installs a signal handler that removes the socket file and hard-exits on
/// `SIGINT` or `SIGTERM`.
///
/// Must be called before any other thread is spawned so that the blocked
/// signal mask (set via `thread_block()` -> `pthread_sigmask()`) is inherited
/// by all child threads. That guarantees `sigwait` in the spawned thread is the
/// sole delivery point for these signals.
pub fn install_signal_handler(socket_path: &'static str) -> Result<(), nix::Error> {
    let mut mask = SigSet::empty();
    mask.add(Signal::SIGINT);
    mask.add(Signal::SIGTERM);

    // Block process-wide; inherited by future threads.
    mask.thread_block()?;

    thread::spawn(move || {
        loop {
            match mask.wait() {
                Ok(sig) => {
                    log::info!("received {sig}, removing socket and exiting");

                    if let Err(e) = fs::remove_file(socket_path) {
                        log::warn!("failed to remove socket {socket_path}: {e}");
                    }

                    process::exit(0);
                }
                Err(e) => {
                    log::error!("sigwait failed: {e}; signal handler thread retrying");
                }
            }
        }
    });

    Ok(())
}
