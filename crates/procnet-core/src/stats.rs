use serde::{Deserialize, Serialize};

use crate::events::ProcStartEvent;

pub const MAP_SIZE: usize = 512;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatsBytes {
    pub sent: u64,
    pub recv: u64,
}

impl StatsBytes {
    #[must_use]
    pub const fn combined(&self) -> u64 {
        self.sent.saturating_add(self.recv)
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

#[derive(Debug)]
struct ProcInfo {
    pid: u32,
    name: Box<str>,
    tcp_cum: StatsBytes,
    udp_cum: StatsBytes,
}

impl ProcInfo {
    #[must_use]
    fn new<T>(pid: u32, name: T, tcp_cum: StatsBytes, udp_cum: StatsBytes) -> Self
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
    total: StatsBytes,
}

#[expect(clippy::missing_const_for_fn)]
impl StatsRow {
    #[must_use]
    pub fn new<T>(pid: u32, name: T, tcp: StatsBytes, udp: StatsBytes) -> Self
    where
        T: Into<Box<str>>,
    {
        Self {
            pid,
            name: name.into(),
            tcp,
            udp,
            total: StatsBytes {
                sent: tcp.sent.saturating_add(udp.sent),
                recv: tcp.recv.saturating_add(udp.recv),
            },
        }
    }

    #[must_use]
    pub fn total(&self) -> &StatsBytes {
        &self.total
    }
}

pub struct StatsCollector {
    procs: Vec<ProcInfo>,
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
        }
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

        self.procs.retain_mut(|proc_info| {
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

                out.push(StatsRow::new(
                    proc_info.pid,
                    proc_info.name.clone(),
                    tcp_delta,
                    udp_delta,
                ));

                true
            } else {
                false
            }
        });
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
    fn stats_row_new_saturates_on_overflow() {
        let row = StatsRow::new(
            130,
            "firefox",
            StatsBytes {
                sent: u64::MAX,
                recv: 10,
            },
            StatsBytes { sent: 20, recv: 10 },
        );
        assert_eq!(row.total.sent, u64::MAX);
        assert_eq!(row.total.recv, 10 + 10);
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
                StatsBytes::default()
            )
        );

        map.remove(140);

        stats.collect_rows(&map, &mut rows);

        assert!(rows.is_empty());
        assert!(stats.procs.is_empty());
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
