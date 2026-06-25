use std::{
    os::unix::net::UnixStream,
    sync::{
        Arc, Mutex,
        mpsc::{self, TryRecvError},
    },
    thread::{self, sleep},
    time::Duration,
};

use anyhow::{Result, anyhow};
use libbpf_rs::MapMut;
use procnet_core::{
    ipc::{Message, SnapshotData},
    stats::{StatsCollector, StatsRow},
};

use crate::{events::EventReader, server, stats_map::MapMutWrapper};

pub fn run(stats_map: &MapMut, events_map: &MapMut) -> Result<()> {
    let (tx, rx) = mpsc::channel::<String>();

    let stream_list: Arc<Mutex<Vec<UnixStream>>> = Arc::new(Mutex::new(Vec::with_capacity(2)));
    let list_for_server = Arc::clone(&stream_list);

    thread::spawn(move || {
        server::run_listener(list_for_server, tx);
    });

    let refresh_interval = Duration::from_secs(1);
    let mut tick: u64 = 0;

    let mut stats = StatsCollector::new();

    let mut events = EventReader::new(events_map)?;

    let mut rows: Vec<StatsRow> = Vec::with_capacity(20);

    let map_wrapper = MapMutWrapper(stats_map);

    loop {
        for event in events.drain_available()? {
            stats.apply_event(event);
        }

        stats.collect_rows(&map_wrapper, &mut rows);

        let message = Message::Snapshot(SnapshotData {
            tick,
            rows: rows.clone(),
        });

        server::update_streams(&stream_list, message)?;

        match rx.try_recv() {
            Ok(e) => return Err(anyhow!("Listener: {}", e)),
            Err(TryRecvError::Disconnected) => return Err(anyhow!("Listener thread exited")),
            Err(TryRecvError::Empty) => {}
        }

        tick = tick.wrapping_add(1);

        sleep(refresh_interval);
    }
}
