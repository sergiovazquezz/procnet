use procnet_core::{ipc::DaemonCommand, stats::StatsCollector};

pub struct DaemonState {
    /// Duration in milliseconds for which the Daemon must sleep in between
    /// iterations (100ms - 5000ms). Tied to
    /// `procnet_core::ipc::DaemonSubcommand::interval`.
    interval: u64,

    /// How many iterations have passed since the Daemon was started. First sent
    /// snapshot at startup or after reset will have tick == 1.
    tick: u64,

    pub stats: StatsCollector,
}

impl DaemonState {
    const DEFAULT_INTERVAL_MILLIS: u64 = 1000;
    const MIN_INTERVAL_MILLIS: u64 = 100;
    const MAX_INTERVAL_MILLIS: u64 = 5000;

    #[inline]
    pub const fn interval(&self) -> u64 {
        self.interval
    }

    #[inline]
    fn set_interval(&mut self, millis: u64) {
        self.interval = millis.clamp(Self::MIN_INTERVAL_MILLIS, Self::MAX_INTERVAL_MILLIS);
    }

    #[inline]
    pub const fn tick(&self) -> u64 {
        self.tick
    }

    #[inline]
    pub const fn advance_tick(&mut self) {
        self.tick += 1;
    }

    #[inline]
    const fn reset_tick(&mut self) {
        self.tick = 0;
    }

    pub fn update(&mut self, command: DaemonCommand) {
        match command {
            DaemonCommand::Run => {
                unreachable!("Run is handled by procnetd::server")
            }
            DaemonCommand::Reset => {
                self.stats.reset();
                self.reset_tick();
            }
            DaemonCommand::Interval { interval } => self.set_interval(interval),
            DaemonCommand::IntervalIncrease => self.set_interval(self.interval + 100),
            DaemonCommand::IntervalDecrease => self.set_interval(self.interval - 100),
        }
    }
}

impl Default for DaemonState {
    fn default() -> Self {
        Self {
            interval: Self::DEFAULT_INTERVAL_MILLIS,
            tick: 0,
            stats: StatsCollector::default(),
        }
    }
}
