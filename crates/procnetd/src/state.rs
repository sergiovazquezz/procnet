use std::{
    sync::atomic::{
        AtomicU64,
        Ordering::{self},
    },
    time::Duration,
};

use procnet_core::ipc::DaemonCommand;

pub struct DaemonState {
    /// Duration in milliseconds for which the Daemon must sleep in between
    /// iterations (100ms - 5000ms). Tied to
    /// `procnet_core::ipc::DaemonSubcommand::interval`.
    interval: AtomicU64,

    /// How many iterations have passed since the Daemon was started.
    tick: AtomicU64,
}

impl DaemonState {
    const DEFAULT_INTERVAL_MILLIS: u64 = 1000;
    const MIN_INTERVAL_MILLIS: u64 = 100;
    const MAX_INTERVAL_MILLIS: u64 = 5000;

    pub fn interval(&self) -> Duration {
        Duration::from_millis(self.interval.load(Ordering::Relaxed))
    }

    fn set_interval(&self, millis: u64) {
        self.interval.store(
            millis.clamp(Self::MIN_INTERVAL_MILLIS, Self::MAX_INTERVAL_MILLIS),
            Ordering::Relaxed,
        );
    }

    pub fn tick(&self) -> u64 {
        self.tick.load(Ordering::Relaxed)
    }

    pub fn advance_tick(&self) {
        self.tick.fetch_add(1, Ordering::Relaxed);
    }

    fn reset_tick(&self) {
        self.tick.store(0, Ordering::Relaxed);
    }

    #[expect(clippy::todo)]
    pub fn update(&self, command: DaemonCommand) {
        match command {
            DaemonCommand::Run => {
                unreachable!("Run is handled by procnetd::server::run_listener()")
            }
            DaemonCommand::Status => todo!(),
            DaemonCommand::Reset => self.reset_tick(),
            DaemonCommand::Interval { interval } => self.set_interval(interval),
        }
    }
}

impl Default for DaemonState {
    fn default() -> Self {
        Self {
            interval: AtomicU64::new(Self::DEFAULT_INTERVAL_MILLIS),
            tick: AtomicU64::new(0),
        }
    }
}
