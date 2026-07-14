use serde::{Deserialize, Serialize};

use crate::events::ProcStartEvent;

pub const MAP_SIZE: usize = 512;

/// Maximum number of recently dead processes retained by the daemon.
pub const DEAD_PROC_LIMIT: usize = 20;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatsBytes {
    pub sent: u64,
    pub recv: u64,
}

impl StatsBytes {
    #[must_use]
    pub const fn combine(&self) -> u64 {
        self.sent.saturating_add(self.recv)
    }

    #[must_use]
    pub const fn merge(self, other: Self) -> Self {
        Self {
            sent: self.sent.saturating_add(other.sent),
            recv: self.recv.saturating_add(other.recv),
        }
    }
}

#[repr(C)]
#[derive(Clone, Debug, Default, PartialEq)]
struct ProcStats {
    pub tcp: StatsBytes,
    pub udp: StatsBytes,
}

/// Abstraction over a per-CPU BPF map lookup so the stats collector can be
/// tested and used without a direct `libbpf-rs` dependency.
pub trait StatsMap {
    fn lookup_percpu(&self, key: &[u8]) -> Option<Vec<Vec<u8>>>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcInfo {
    pub pid: u32,
    pub name: Box<str>,
    pub tcp_cum: StatsBytes,
    pub udp_cum: StatsBytes,
}

impl ProcInfo {
    #[must_use]
    pub fn new<T>(pid: u32, name: T, tcp_cum: StatsBytes, udp_cum: StatsBytes) -> Self
    where
        T: Into<Box<str>>,
    {
        Self {
            pid,
            name: name.into(),
            tcp_cum,
            udp_cum,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct StatsRow {
    pub pid: u32,
    pub name: Box<str>,
    pub tcp: StatsBytes,
    pub udp: StatsBytes,
    tcp_cum: StatsBytes,
    udp_cum: StatsBytes,
}

#[expect(clippy::missing_const_for_fn)]
impl StatsRow {
    #[must_use]
    pub fn new<T>(
        pid: u32,
        name: T,
        tcp: StatsBytes,
        udp: StatsBytes,
        tcp_cum: StatsBytes,
        udp_cum: StatsBytes,
    ) -> Self
    where
        T: Into<Box<str>>,
    {
        Self {
            pid,
            name: name.into(),
            tcp,
            udp,
            tcp_cum,
            udp_cum,
        }
    }

    /// (`tcp_cum`, `udp_cum`)
    #[must_use]
    pub fn cum(&self) -> (StatsBytes, StatsBytes) {
        (self.tcp_cum, self.udp_cum)
    }

    /// Per-tick total (TCP + UDP).
    #[must_use]
    pub fn total(&self) -> StatsBytes {
        self.tcp.merge(self.udp)
    }

    /// Cumulative total (TCP + UDP).
    #[must_use]
    pub fn total_cum(&self) -> StatsBytes {
        self.tcp_cum.merge(self.udp_cum)
    }
}

pub struct StatsCollector {
    procs: Vec<ProcInfo>,
    dead_procs: Vec<ProcInfo>,
}

impl Default for StatsCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl StatsCollector {
    #[must_use]
    pub fn new() -> Self {
        Self {
            procs: Vec::with_capacity(MAP_SIZE),
            dead_procs: Vec::with_capacity(DEAD_PROC_LIMIT),
        }
    }

    /// Read-only view of the recently dead processes.
    #[must_use]
    pub fn dead_procs(&self) -> &[ProcInfo] {
        &self.dead_procs
    }

    pub fn reset(&mut self) {
        for proc in &mut self.procs {
            proc.tcp_cum = StatsBytes::default();
            proc.udp_cum = StatsBytes::default();
        }

        self.dead_procs.clear();
    }

    pub fn apply_event(&mut self, mut event: ProcStartEvent) {
        event.name.make_ascii_lowercase();

        if let Some(p) = self.procs.iter_mut().find(|p| p.pid == event.pid) {
            p.name = Box::from(event.name);
            p.tcp_cum = StatsBytes::default();
            p.udp_cum = StatsBytes::default();
        } else {
            self.procs.push(ProcInfo::new(
                event.pid,
                event.name,
                StatsBytes::default(),
                StatsBytes::default(),
            ));
        }
    }

    pub fn collect_rows(&mut self, stats_map: &impl StatsMap, out: &mut Vec<StatsRow>) {
        out.clear();

        let mut idx: usize = 0;

        while idx < self.procs.len() {
            let proc_info = &mut self.procs[idx];

            if let Some(new_stats) = merge_values_for_pid(stats_map, proc_info.pid) {
                let tcp_delta = StatsBytes {
                    sent: new_stats.tcp.sent.saturating_sub(proc_info.tcp_cum.sent),
                    recv: new_stats.tcp.recv.saturating_sub(proc_info.tcp_cum.recv),
                };

                let udp_delta = StatsBytes {
                    sent: new_stats.udp.sent.saturating_sub(proc_info.udp_cum.sent),
                    recv: new_stats.udp.recv.saturating_sub(proc_info.udp_cum.recv),
                };

                proc_info.tcp_cum = new_stats.tcp;
                proc_info.udp_cum = new_stats.udp;

                let row = StatsRow::new(
                    proc_info.pid,
                    proc_info.name.clone(),
                    tcp_delta,
                    udp_delta,
                    proc_info.tcp_cum,
                    proc_info.udp_cum,
                );

                out.push(row);

                idx += 1;
            } else {
                let dead_proc = self.procs.swap_remove(idx);
                self.dead_procs.push(dead_proc);

                if self.dead_procs.len() > DEAD_PROC_LIMIT {
                    self.dead_procs.remove(0);
                }
            }
        }
    }
}

fn merge_values_for_pid(stats_map: &impl StatsMap, pid: u32) -> Option<ProcStats> {
    let key = pid.to_ne_bytes();

    let per_cpu_values = stats_map.lookup_percpu(&key)?;

    let mut merged = ProcStats::default();

    for value in per_cpu_values {
        if let Some(value) = proc_stats_from_bytes(&value) {
            merged.tcp.sent = merged.tcp.sent.saturating_add(value.tcp.sent);
            merged.tcp.recv = merged.tcp.recv.saturating_add(value.tcp.recv);
            merged.udp.sent = merged.udp.sent.saturating_add(value.udp.sent);
            merged.udp.recv = merged.udp.recv.saturating_add(value.udp.recv);
        }
    }

    Some(merged)
}

#[expect(clippy::missing_const_for_fn)]
fn proc_stats_from_bytes(data: &[u8]) -> Option<ProcStats> {
    if data.len() != size_of::<ProcStats>() {
        return None;
    }

    let stats = unsafe { data.as_ptr().cast::<ProcStats>().read_unaligned() };

    Some(stats)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[derive(Default)]
    struct FakeStatsMap {
        data: HashMap<Vec<u8>, Vec<Vec<u8>>>,
    }

    impl StatsMap for FakeStatsMap {
        fn lookup_percpu(&self, key: &[u8]) -> Option<Vec<Vec<u8>>> {
            self.data.get(key).cloned()
        }
    }

    impl FakeStatsMap {
        fn new(pid: u32, tcp: StatsBytes, udp: StatsBytes) -> Self {
            let mut map = Self {
                data: HashMap::new(),
            };

            map.data
                .insert(pid.to_ne_bytes().to_vec(), vec![proc_stats_bytes(tcp, udp)]);

            map
        }

        /// Creates two vectors for a given `pid`, the second having `10` for
        /// sent and recv.
        fn new_two_values(pid: u32, tcp: StatsBytes, udp: StatsBytes) -> Self {
            let mut map = Self {
                data: HashMap::new(),
            };

            let second_bytes = StatsBytes { sent: 10, recv: 10 };

            map.data.insert(
                pid.to_ne_bytes().to_vec(),
                vec![
                    proc_stats_bytes(tcp, udp),
                    proc_stats_bytes(second_bytes, second_bytes),
                ],
            );

            map
        }

        fn remove(&mut self, pid: u32) {
            self.data.remove(pid.to_ne_bytes().as_slice());
        }
    }

    fn proc_stats_bytes(tcp: StatsBytes, udp: StatsBytes) -> Vec<u8> {
        let mut v: Vec<u8> = Vec::with_capacity(size_of::<ProcStats>());

        v.extend_from_slice(&tcp.sent.to_ne_bytes());
        v.extend_from_slice(&tcp.recv.to_ne_bytes());
        v.extend_from_slice(&udp.sent.to_ne_bytes());
        v.extend_from_slice(&udp.recv.to_ne_bytes());

        v
    }

    #[test]
    fn reset_zeroes_cumulative_stats() {
        let mut stats = StatsCollector::default();
        stats.apply_event(ProcStartEvent {
            pid: 1,
            name: "a".into(),
        });
        stats.procs[0].tcp_cum = StatsBytes { sent: 10, recv: 20 };
        stats.procs[0].udp_cum = StatsBytes { sent: 30, recv: 40 };

        stats.reset();

        assert_eq!(stats.procs[0].tcp_cum, StatsBytes::default());
        assert_eq!(stats.procs[0].udp_cum, StatsBytes::default());

        assert_eq!(stats.procs.len(), 1);
    }

    #[test]
    fn stats_row_new_saturates_on_overflow() {
        let row = StatsRow::new(
            130,
            "firefox",
            StatsBytes {
                sent: u64::MAX,
                recv: 10,
            },
            StatsBytes { sent: 20, recv: 10 },
            StatsBytes::default(),
            StatsBytes::default(),
        );
        assert_eq!(row.total().sent, u64::MAX);
        assert_eq!(row.total().recv, 10 + 10);
    }

    #[test]
    fn stats_bytes_merge_saturates() {
        let a = StatsBytes {
            sent: u64::MAX,
            recv: 10,
        };

        let b = StatsBytes { sent: 5, recv: 20 };

        let merged = a.merge(b);

        assert_eq!(merged.sent, u64::MAX);
        assert_eq!(merged.recv, 30);
    }

    #[test]
    fn collect_rows_populates_cumulative() {
        let mut stats = StatsCollector::default();
        stats.apply_event(ProcStartEvent {
            pid: 7,
            name: "vim".into(),
        });

        let tcp1 = StatsBytes {
            sent: 100,
            recv: 200,
        };
        let udp1 = StatsBytes {
            sent: 400,
            recv: 1000,
        };

        let map = FakeStatsMap::new(7, tcp1, udp1);
        let mut rows = Vec::new();
        stats.collect_rows(&map, &mut rows);

        // After the first tick the cumulative counters equal the merged values.
        assert_eq!(rows[0].tcp_cum, tcp1);
        assert_eq!(rows[0].udp_cum, udp1);
        assert_eq!(rows[0].total_cum(), tcp1.merge(udp1));

        // Second tick: cumulative tracks the latest counters, delta is the diff.
        let tcp2 = StatsBytes {
            sent: 150,
            recv: 250,
        };
        let udp2 = StatsBytes {
            sent: 500,
            recv: 1200,
        };
        let map2 = FakeStatsMap::new(7, tcp2, udp2);
        stats.collect_rows(&map2, &mut rows);

        assert_eq!(rows[0].tcp_cum, tcp2);
        assert_eq!(rows[0].udp_cum, udp2);
        assert_eq!(rows[0].total_cum(), tcp2.merge(udp2));
    }

    #[test]
    fn apply_event_pushes_and_is_lowercase() {
        let mut stats = StatsCollector::default();
        stats.apply_event(ProcStartEvent {
            pid: 10,
            name: "GOOGLE".into(),
        });

        assert_eq!(stats.procs[0].name.as_ref(), "google");

        let bytes = StatsBytes::default();
        let map = FakeStatsMap::new(10, bytes, bytes);

        let mut rows: Vec<StatsRow> = Vec::new();

        stats.collect_rows(&map, &mut rows);

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name.as_ref(), "google");
    }

    #[test]
    fn apply_event_resets_existing_pid() {
        let mut stats = StatsCollector::default();
        stats.apply_event(ProcStartEvent {
            pid: 10,
            name: "first".into(),
        });
        stats.apply_event(ProcStartEvent {
            pid: 10,
            name: "second".into(),
        });

        assert_eq!(stats.procs.len(), 1);
        assert_eq!(stats.procs[0].name.as_ref(), "second");

        let zero_bytes = StatsBytes { sent: 0, recv: 0 };
        assert_eq!(stats.procs[0].tcp_cum, zero_bytes);
        assert_eq!(stats.procs[0].udp_cum, zero_bytes);
    }

    #[test]
    fn collect_rows_removes_exited_proc() {
        let event = ProcStartEvent {
            pid: 140,
            name: "librewolf".into(),
        };

        let mut stats = StatsCollector::default();
        stats.apply_event(event);

        assert_eq!(stats.procs.len(), 1);

        let mut map = FakeStatsMap::new(140, StatsBytes::default(), StatsBytes::default());
        let mut rows = Vec::<StatsRow>::with_capacity(1);

        stats.collect_rows(&map, &mut rows);

        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0],
            StatsRow::new(
                140,
                "librewolf",
                StatsBytes::default(),
                StatsBytes::default(),
                StatsBytes::default(),
                StatsBytes::default(),
            )
        );

        map.remove(140);

        stats.collect_rows(&map, &mut rows);

        assert!(rows.is_empty());
        assert!(stats.procs.is_empty());
        assert_eq!(stats.dead_procs().len(), 1);
        assert_eq!(stats.dead_procs()[0].pid, 140);

        // A second tick where the proc is still gone must not duplicate it.
        stats.collect_rows(&map, &mut rows);

        assert_eq!(stats.dead_procs().len(), 1);
    }

    #[test]
    fn dead_procs_capped_at_limit() {
        let mut stats = StatsCollector::default();
        let mut map = FakeStatsMap::default();

        for pid in 0..u32::try_from(DEAD_PROC_LIMIT).unwrap() + 5 {
            stats.apply_event(ProcStartEvent {
                pid,
                name: "p".into(),
            });
            map.data.insert(
                pid.to_ne_bytes().to_vec(),
                vec![proc_stats_bytes(
                    StatsBytes::default(),
                    StatsBytes::default(),
                )],
            );
        }

        let mut rows = Vec::<StatsRow>::new();

        // Kill one proc per tick by removing it from the live map first.
        for pid in 0..u32::try_from(DEAD_PROC_LIMIT).unwrap() + 5 {
            map.remove(pid);
            stats.collect_rows(&map, &mut rows);

            assert!(
                stats.dead_procs().len() <= DEAD_PROC_LIMIT,
                "tick {}: dead_procs.len() = {}",
                pid,
                stats.dead_procs().len()
            );
        }

        let kept: Vec<u32> = stats.dead_procs().iter().map(|p| p.pid).collect();
        assert_eq!(kept.len(), DEAD_PROC_LIMIT);
        // Most recent `DEAD_PROC_LIMIT` deaths are kept; oldest evicted first.
        // 25 deaths total (pids 0..=24), so the kept window is pids 5..=24.
        let total_dead = u32::try_from(DEAD_PROC_LIMIT).unwrap() + 5;
        assert_eq!(
            kept.first(),
            Some(&(total_dead - u32::try_from(DEAD_PROC_LIMIT).unwrap()))
        );
        assert_eq!(kept.last(), Some(&(total_dead - 1)));
        assert!(
            kept.windows(2).all(|w| w[0] < w[1]),
            "kept order is ascending"
        );
    }

    #[test]
    fn reset_clears_dead_procs() {
        let mut stats = StatsCollector::default();
        stats.apply_event(ProcStartEvent {
            pid: 1,
            name: "a".into(),
        });
        let map = FakeStatsMap::default();
        let mut rows = Vec::new();

        // pid 1 was never in the map, immediately counted as dead.
        stats.collect_rows(&map, &mut rows);
        assert_eq!(stats.dead_procs().len(), 1);

        stats.reset();

        assert!(stats.dead_procs().is_empty());
    }

    #[test]
    fn collect_rows_computes_delta() {
        let mut stats = StatsCollector::default();
        stats.apply_event(ProcStartEvent {
            pid: 7,
            name: "vim".into(),
        });

        let tcp_bytes = StatsBytes {
            sent: 100,
            recv: 200,
        };
        let udp_bytes = StatsBytes {
            sent: 400,
            recv: 1000,
        };

        let map = FakeStatsMap::new(7, tcp_bytes, udp_bytes);
        let mut rows: Vec<StatsRow> = Vec::new();

        stats.collect_rows(&map, &mut rows);
        assert_eq!(rows[0].tcp, tcp_bytes);
        assert_eq!(rows[0].udp, udp_bytes);

        let tcp_bytes2 = StatsBytes {
            sent: tcp_bytes.sent + 20,
            recv: tcp_bytes.recv + 10,
        };
        let udp_bytes2 = StatsBytes {
            sent: udp_bytes.sent + 20,
            recv: udp_bytes.recv + 10,
        };

        let map = FakeStatsMap::new(7, tcp_bytes2, udp_bytes2);

        let tcp_delta = StatsBytes {
            sent: tcp_bytes2.sent.saturating_sub(tcp_bytes.sent),
            recv: tcp_bytes2.recv.saturating_sub(tcp_bytes.recv),
        };
        let udp_delta = StatsBytes {
            sent: udp_bytes2.sent.saturating_sub(udp_bytes.sent),
            recv: udp_bytes2.recv.saturating_sub(udp_bytes.recv),
        };

        stats.collect_rows(&map, &mut rows);
        assert_eq!(rows[0].tcp, tcp_delta);
        assert_eq!(rows[0].udp, udp_delta);
    }

    #[test]
    fn merge_values_for_pid_success() {
        let tcp_bytes = StatsBytes {
            sent: 50,
            recv: 110,
        };
        let udp_bytes = StatsBytes {
            sent: 10,
            recv: 500,
        };

        let map = FakeStatsMap::new_two_values(3, tcp_bytes, udp_bytes);

        let merged = merge_values_for_pid(&map, 3).unwrap();

        assert_eq!(
            merged,
            ProcStats {
                tcp: StatsBytes {
                    sent: tcp_bytes.sent + 10,
                    recv: tcp_bytes.recv + 10
                },
                udp: StatsBytes {
                    sent: udp_bytes.sent + 10,
                    recv: udp_bytes.recv + 10
                },
            }
        );
    }

    #[test]
    fn proc_stats_from_bytes_valid_length() {
        let mut raw: Vec<u8> = Vec::with_capacity(size_of::<ProcStats>());
        let bytes = StatsBytes { sent: 10, recv: 20 };

        // TCP and UDP
        raw.extend_from_slice(&bytes.sent.to_ne_bytes());
        raw.extend_from_slice(&bytes.recv.to_ne_bytes());
        raw.extend_from_slice(&bytes.sent.to_ne_bytes());
        raw.extend_from_slice(&bytes.recv.to_ne_bytes());

        let result = proc_stats_from_bytes(&raw).unwrap();

        assert_eq!(
            result,
            ProcStats {
                tcp: bytes,
                udp: bytes
            }
        );

        assert_eq!(proc_stats_from_bytes(&[0u8; 3]), None);
    }
}
