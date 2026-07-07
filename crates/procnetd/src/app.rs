use std::{
    sync::{Arc, Mutex, mpsc::SyncSender},
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
    server,
    state::DaemonState,
    stats_map::MapMutWrapper,
};

pub fn run(stats_map: &MapMut, events_map: &MapMut) -> Result<(), DaemonError> {
    let daemon_state = Arc::new(DaemonState::default());
    let state_clone = Arc::clone(&daemon_state);

    let senders = Arc::new(Mutex::new(Vec::<SyncSender<Arc<[u8]>>>::new()));
    let listener_senders = Arc::clone(&senders);

    let listener_handle =
        thread::spawn(move || server::run_listener(listener_senders, state_clone));

    let events = EventReader::new(events_map)?;

    let mut rows = Vec::<StatsRow>::with_capacity(MAP_SIZE);

    let map_wrapper = MapMutWrapper::new(stats_map);

    let mut buf = Vec::<u8>::with_capacity(8 * 1024);

    loop {
        let mut stats_guard = daemon_state.stats.lock().map_err(|_| MutexPoison)?;

        for event in events.drain_available()? {
            stats_guard.apply_event(event);
        }

        stats_guard.collect_rows(&map_wrapper, &mut rows);

        let snapshot = SnapshotRef {
            interval: daemon_state.interval(),
            tick: daemon_state.tick(),
            rows: &rows,
        };

        drop(stats_guard);

        ipc::write_msg(&mut buf, &snapshot)?;

        let shared: Arc<[u8]> = Arc::from(buf.as_slice());

        server::update_streams(&senders, &shared)?;

        thread::sleep(Duration::from_millis(daemon_state.interval()));

        if listener_handle.is_finished() {
            match listener_handle.join() {
                Ok(Err(e)) => return Err(DaemonError::ListenerError(e)),
                Ok(Ok(never)) => match never {},
                Err(_) => return Err(DaemonError::ThreadPanic),
            }
        }

        daemon_state.advance_tick();
    }
}
