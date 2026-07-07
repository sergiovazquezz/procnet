use std::{
    sync::{Arc, Mutex, mpsc::SyncSender},
    thread,
};

use libbpf_rs::MapMut;
use procnet_core::{
    ipc::{self, SnapshotRef},
    stats::{MAP_SIZE, StatsCollector, StatsRow},
};

use crate::{
    errors::DaemonError, events::EventReader, server, state::DaemonState, stats_map::MapMutWrapper,
};

pub fn run(stats_map: &MapMut, events_map: &MapMut) -> Result<(), DaemonError> {
    let daemon_state = Arc::new(DaemonState::default());
    let state_clone = Arc::clone(&daemon_state);

    let senders = Arc::new(Mutex::new(Vec::<SyncSender<Arc<[u8]>>>::new()));
    let listener_senders = Arc::clone(&senders);

    let join_handle = thread::spawn(move || server::run_listener(&listener_senders, state_clone));

    let mut stats = StatsCollector::default();

    let events = EventReader::new(events_map)?;

    let mut rows = Vec::<StatsRow>::with_capacity(MAP_SIZE);

    let map_wrapper = MapMutWrapper::new(stats_map);

    let mut buf = Vec::<u8>::with_capacity(8 * 1024);

    loop {
        for event in events.drain_available()? {
            stats.apply_event(event);
        }

        stats.collect_rows(&map_wrapper, &mut rows);

        let snapshot = SnapshotRef {
            tick: daemon_state.tick(),
            rows: &rows,
        };
        ipc::write_msg(&mut buf, &snapshot)?;

        let shared: Arc<[u8]> = Arc::from(buf.as_slice());

        server::update_streams(&senders, &shared)?;

        thread::sleep(daemon_state.interval());

        if join_handle.is_finished() {
            match join_handle.join() {
                Ok(Err(e)) => return Err(DaemonError::ListenerError(e)),
                Ok(Ok(never)) => match never {},
                Err(_) => return Err(DaemonError::ThreadPanic),
            }
        }

        daemon_state.advance_tick();
    }
}
