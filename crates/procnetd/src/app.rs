use std::{
    sync::{
        Arc, Mutex,
        mpsc::{self, RecvTimeoutError, SyncSender},
    },
    thread,
    time::Duration,
};

use libbpf_rs::MapMut;
use procnet_core::{
    ipc::{self, SnapshotRef},
    stats::{MAP_SIZE, StatsRow},
};

use crate::{
    errors::{DaemonError, MutexPoison},
    events::EventReader,
    server, signals,
    state::DaemonState,
    stats_map::MapMutWrapper,
};

pub fn run(stats_map: &MapMut, events_map: &MapMut) -> Result<(), DaemonError> {
    let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>();

    let socket_path = ipc::socket_path();

    let listener = server::bind_unix_listener(&socket_path)?;

    log::info!("Listening on {}", socket_path.display());

    signals::install_signal_handler(socket_path, shutdown_tx)?;

    let daemon_state = Arc::new(Mutex::new(DaemonState::default()));
    let state_clone = Arc::clone(&daemon_state);

    let senders = Arc::new(Mutex::new(Vec::<SyncSender<Arc<[u8]>>>::new()));
    let listener_senders = Arc::clone(&senders);

    let listener_handle =
        thread::spawn(move || server::accept_loop(listener, listener_senders, state_clone));

    let events = EventReader::new(events_map)?;

    let mut rows = Vec::<StatsRow>::with_capacity(MAP_SIZE);

    let map_wrapper = MapMutWrapper::new(stats_map);

    let mut buf = Vec::<u8>::with_capacity(8 * 1024);

    loop {
        let mut state_guard = daemon_state.lock().map_err(|_| MutexPoison)?;

        for event in events.drain_available()? {
            state_guard.stats.apply_event(event);
        }

        state_guard.stats.collect_rows(&map_wrapper, &mut rows);

        state_guard.advance_tick();

        let snapshot = SnapshotRef {
            interval: state_guard.interval(),
            tick: state_guard.tick(),
            rows: &rows,
        };

        let timeout = Duration::from_millis(state_guard.interval());

        drop(state_guard);

        ipc::write_msg(&mut buf, &snapshot)?;

        let shared: Arc<[u8]> = Arc::from(buf.as_slice());

        server::update_streams(&senders, &shared)?;

        match shutdown_rx.recv_timeout(timeout) {
            Ok(()) | Err(RecvTimeoutError::Disconnected) => return Ok(()),
            _ => {}
        }

        if listener_handle.is_finished() {
            match listener_handle.join() {
                Ok(Err(e)) => return Err(DaemonError::ListenerError(e)),
                Ok(Ok(never)) => match never {},
                Err(_) => return Err(DaemonError::ThreadPanic),
            }
        }
    }
}
