use std::{
    os::unix::net::UnixStream,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use libbpf_rs::MapMut;
use procnet_core::{
    ipc::{self, SnapshotRef},
    stats::{StatsCollector, StatsRow},
};

use crate::{errors::DaemonError, events::EventReader, server, stats_map::MapMutWrapper};

pub fn run(stats_map: &MapMut, events_map: &MapMut) -> Result<(), DaemonError> {
    let stream_list = Arc::new(Mutex::new(Vec::<UnixStream>::with_capacity(2)));
    let list_for_server = Arc::clone(&stream_list);

    let join_handle = thread::spawn(move || server::run_listener(&list_for_server));

    let refresh_interval = Duration::from_secs(1);
    let mut tick: u64 = 0;

    let mut stats = StatsCollector::default();

    let events = EventReader::new(events_map)?;

    let mut rows = Vec::<StatsRow>::with_capacity(20);

    let map_wrapper = MapMutWrapper::new(stats_map);

    let mut buf = Vec::<u8>::with_capacity(8 * 1024);

    loop {
        for event in events.drain_available()? {
            stats.apply_event(event);
        }

        stats.collect_rows(&map_wrapper, &mut rows);

        let snapshot = SnapshotRef { tick, rows: &rows };
        ipc::write_msg(&mut buf, &snapshot)?;

        server::update_streams(&stream_list, &buf)?;

        thread::sleep(refresh_interval);

        if join_handle.is_finished() {
            match join_handle.join() {
                Ok(Err(e)) => return Err(DaemonError::ListenerError(e)),
                Ok(Ok(never)) => match never {},
                Err(_) => return Err(DaemonError::ThreadPanic),
            }
        }

        tick = tick.wrapping_add(1);
    }
}
