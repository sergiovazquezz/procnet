use std::{
    sync::atomic::{
        AtomicU64,
        Ordering::{self},
    },
    time::Duration,
};

const DEFAULT_INTERVAL_MILLIS: u64 = 1000;

pub struct DaemonState {
    /// Duration in milliseconds for which the Daemon must sleep in between
    /// iterations (100ms - 5000ms). Tied to
    /// `procnet_core::ipc::DaemonSubcommand::interval`.
    interval: AtomicU64,

    /// How many iterations have passed since the Daemon was started.
    tick: AtomicU64,
}

impl DaemonState {
    pub fn interval(&self) -> Duration {
        Duration::from_millis(self.interval.load(Ordering::Relaxed))
    }

    pub fn set_interval(&self, millis: u64) {
        self.interval.store(millis, Ordering::Relaxed);
    }

    pub fn tick(&self) -> u64 {
        self.tick.load(Ordering::Relaxed)
    }

    pub fn advance_tick(&self) {
        self.tick.fetch_add(1, Ordering::Relaxed);
    }
}

impl Default for DaemonState {
    fn default() -> Self {
        Self {
            interval: AtomicU64::new(DEFAULT_INTERVAL_MILLIS),
            tick: AtomicU64::new(0),
        }
    }
}
