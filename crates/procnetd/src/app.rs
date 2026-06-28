use std::{
    os::unix::net::UnixStream,
    sync::{
        Arc, Mutex,
        mpsc::{self, RecvTimeoutError},
    },
    thread,
    time::Duration,
};

use anyhow::{Result, bail};
use libbpf_rs::MapMut;

use procnet_core::{
    ipc::{Message, SnapshotData},
    stats::{StatsCollector, StatsRow},
};

use crate::{errors::ListenerError, events::EventReader, server, stats_map::MapMutWrapper};

pub fn run(stats_map: &MapMut, events_map: &MapMut) -> Result<()> {
    let (tx, rx) = mpsc::channel::<ListenerError>();

    let stream_list = Arc::new(Mutex::new(Vec::<UnixStream>::with_capacity(2)));
    let list_for_server = Arc::clone(&stream_list);

    let join_handle = thread::spawn(move || {
        server::run_listener(list_for_server, tx);
    });

    let refresh_interval = Duration::from_secs(1);
    let mut tick: u64 = 0;

    let mut stats = StatsCollector::default();

    let events = EventReader::new(events_map)?;

    let mut rows: Vec<StatsRow> = Vec::with_capacity(20);

    let map_wrapper = MapMutWrapper::new(stats_map);

    loop {
        for event in events.drain_available()? {
            stats.apply_event(event);
        }

        stats.collect_rows(&map_wrapper, &mut rows);

        let message = Message::Snapshot(SnapshotData {
            tick,
            rows: rows.clone(),
        });

        server::update_streams(&stream_list, &message)?;

        match rx.recv_timeout(refresh_interval) {
            Ok(e) => bail!("{e}"),
            Err(RecvTimeoutError::Disconnected) => match join_handle.join() {
                Ok(()) => bail!("Listener thread exited"),
                Err(_) => bail!("Listener thread panicked"),
            },
            Err(RecvTimeoutError::Timeout) => {}
        }

        tick = tick.wrapping_add(1);
    }
}
