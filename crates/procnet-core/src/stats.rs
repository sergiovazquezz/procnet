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
    pub const fn new(sent: u64, recv: u64) -> Self {
        Self { sent, recv }
    }

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
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
/// The IP's are in Big Endian representation.
pub struct StatsAddr {
    pub dst_port: u16,
    pub src_port: u16,
    /// Big Endian.
    pub dst_ipv4: u32,
    /// Big Endian.
    pub dst_ipv6: [u8; 16],
}

impl StatsAddr {
    #[must_use]
    /// Fields `dst_ipv4` and `dst_ipv6` are expected to be in Big Endian
    /// representation.
    pub const fn new(dst_port: u16, src_port: u16, dst_ipv4: u32, dst_ipv6: [u8; 16]) -> Self {
        Self {
            dst_port,
            src_port,
            dst_ipv4,
            dst_ipv6,
        }
    }
}

#[repr(C)]
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolStats {
    pub bytes: StatsBytes,
    pub addr: StatsAddr,
}

impl ProtocolStats {
    #[must_use]
    pub const fn new(bytes: StatsBytes, addr: StatsAddr) -> Self {
        Self { bytes, addr }
    }
}

#[repr(C)]
#[derive(Clone, Debug, Default, PartialEq)]
struct ProcStats {
    pub tcp: ProtocolStats,
    pub udp: ProtocolStats,
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
    pub tcp: ProtocolStats,
    pub udp: ProtocolStats,
    tcp_cum: StatsBytes,
    udp_cum: StatsBytes,
}

#[expect(clippy::missing_const_for_fn)]
impl StatsRow {
    #[must_use]
    pub fn new<T>(
        pid: u32,
        name: T,
        tcp: ProtocolStats,
        udp: ProtocolStats,
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
        self.tcp.bytes.merge(self.udp.bytes)
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
                let tcp_bytes = &new_stats.tcp.bytes;
                let udp_bytes = &new_stats.udp.bytes;

                let tcp_delta = StatsBytes {
                    sent: tcp_bytes.sent.saturating_sub(proc_info.tcp_cum.sent),
                    recv: tcp_bytes.recv.saturating_sub(proc_info.tcp_cum.recv),
                };

                let udp_delta = StatsBytes {
                    sent: udp_bytes.sent.saturating_sub(proc_info.udp_cum.sent),
                    recv: udp_bytes.recv.saturating_sub(proc_info.udp_cum.recv),
                };

                proc_info.tcp_cum = *tcp_bytes;
                proc_info.udp_cum = *udp_bytes;

                let tcp = ProtocolStats::new(tcp_delta, new_stats.tcp.addr);
                let udp = ProtocolStats::new(udp_delta, new_stats.udp.addr);

                let row = StatsRow::new(
                    proc_info.pid,
                    proc_info.name.clone(),
                    tcp,
                    udp,
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

#[inline]
fn merge_stats(merged: &mut ProtocolStats, stats: ProtocolStats) {
    merged.bytes.sent = merged.bytes.sent.saturating_add(stats.bytes.sent);
    merged.bytes.recv = merged.bytes.recv.saturating_add(stats.bytes.recv);

    if merged.addr == StatsAddr::default() && stats.addr != StatsAddr::default() {
        merged.addr = stats.addr;
    }
}

fn merge_values_for_pid(stats_map: &impl StatsMap, pid: u32) -> Option<ProcStats> {
    let key = pid.to_ne_bytes();

    let per_cpu_values = stats_map.lookup_percpu(&key)?;

    let mut merged = ProcStats::default();

    for value in per_cpu_values {
        if let Some(value) = proc_stats_from_bytes(&value) {
            merge_stats(&mut merged.tcp, value.tcp);
            merge_stats(&mut merged.udp, value.udp);
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
        fn new(pid: u32, tcp: ProtocolStats, udp: ProtocolStats) -> Self {
            let mut map = Self {
                data: HashMap::new(),
            };

            map.data.insert(
                pid.to_ne_bytes().to_vec(),
                vec![proc_stats_to_bytes(tcp, udp)],
            );

            map
        }

        /// Creates two vectors for a given `pid`. The second having `10` for
        /// sent and recv, and an empty `addr`.
        fn new_two_values(pid: u32, tcp: ProtocolStats, udp: ProtocolStats) -> Self {
            let mut map = Self {
                data: HashMap::new(),
            };

            let second_stats = ProtocolStats {
                bytes: StatsBytes { sent: 10, recv: 10 },
                addr: StatsAddr::default(),
            };

            map.data.insert(
                pid.to_ne_bytes().to_vec(),
                vec![
                    proc_stats_to_bytes(tcp, udp),
                    proc_stats_to_bytes(second_stats.clone(), second_stats),
                ],
            );

            map
        }

        fn remove(&mut self, pid: u32) {
            self.data.remove(pid.to_ne_bytes().as_slice());
        }
    }

    fn proc_stats_to_bytes(tcp: ProtocolStats, udp: ProtocolStats) -> Vec<u8> {
        let mut v: Vec<u8> = Vec::with_capacity(size_of::<ProcStats>());

        v.extend_from_slice(&tcp.bytes.sent.to_ne_bytes());
        v.extend_from_slice(&tcp.bytes.recv.to_ne_bytes());
        v.extend_from_slice(&tcp.addr.dst_port.to_ne_bytes());
        v.extend_from_slice(&tcp.addr.src_port.to_ne_bytes());
        v.extend_from_slice(&tcp.addr.dst_ipv4.to_ne_bytes());
        v.extend_from_slice(&tcp.addr.dst_ipv6);

        v.extend_from_slice(&udp.bytes.sent.to_ne_bytes());
        v.extend_from_slice(&udp.bytes.recv.to_ne_bytes());
        v.extend_from_slice(&udp.addr.dst_port.to_ne_bytes());
        v.extend_from_slice(&udp.addr.src_port.to_ne_bytes());
        v.extend_from_slice(&udp.addr.dst_ipv4.to_ne_bytes());
        v.extend_from_slice(&udp.addr.dst_ipv6);

        assert_eq!(v.len(), size_of::<ProcStats>());

        v
    }

    fn test_addr() -> StatsAddr {
        let ipv4 = u32::from_be_bytes([192, 168, 1, 10]).to_be();

        let ipv6: [u8; 16] = [
            0x20, 0x01, 0x0d, 0xb8, 0x85, 0xa3, 0x00, 0x00, 0x00, 0x00, 0x8a, 0x2e, 0x03, 0x70,
            0x73, 0x34,
        ];

        StatsAddr {
            dst_port: 443,
            src_port: 120,
            dst_ipv4: ipv4,
            dst_ipv6: ipv6,
        }
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
        let tcp = ProtocolStats::new(
            StatsBytes {
                sent: u64::MAX,
                recv: 10,
            },
            StatsAddr::default(),
        );

        let udp = ProtocolStats::new(StatsBytes { sent: 20, recv: 10 }, StatsAddr::default());

        let row = StatsRow::new(
            130,
            "firefox",
            tcp,
            udp,
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

        let tcp1 = ProtocolStats {
            bytes: StatsBytes {
                sent: 100,
                recv: 200,
            },
            addr: test_addr(),
        };
        let udp1 = ProtocolStats {
            bytes: StatsBytes {
                sent: 400,
                recv: 1000,
            },
            addr: test_addr(),
        };

        let map = FakeStatsMap::new(7, tcp1.clone(), udp1.clone());
        let mut rows = Vec::new();
        stats.collect_rows(&map, &mut rows);

        // After the first tick the cumulative counters equal the merged values.
        assert_eq!(rows[0].tcp_cum, tcp1.bytes);
        assert_eq!(rows[0].udp_cum, udp1.bytes);
        assert_eq!(rows[0].total_cum(), tcp1.bytes.merge(udp1.bytes));

        // Second tick: cumulative tracks the latest counters, delta is the diff.
        let tcp2 = ProtocolStats {
            bytes: StatsBytes {
                sent: 150,
                recv: 250,
            },
            addr: test_addr(),
        };
        let udp2 = ProtocolStats {
            bytes: StatsBytes {
                sent: 500,
                recv: 1200,
            },
            addr: test_addr(),
        };

        let map2 = FakeStatsMap::new(7, tcp2.clone(), udp2.clone());
        stats.collect_rows(&map2, &mut rows);

        assert_eq!(rows[0].tcp_cum, tcp2.bytes);
        assert_eq!(rows[0].udp_cum, udp2.bytes);
        assert_eq!(rows[0].total_cum(), tcp2.bytes.merge(udp2.bytes));
    }

    #[test]
    fn apply_event_pushes_and_is_lowercase() {
        let mut stats = StatsCollector::default();
        stats.apply_event(ProcStartEvent {
            pid: 10,
            name: "GOOGLE".into(),
        });

        assert_eq!(stats.procs[0].name.as_ref(), "google");

        let prot_stats = ProtocolStats::default();
        let map = FakeStatsMap::new(10, prot_stats.clone(), prot_stats);

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

        let mut map = FakeStatsMap::new(140, ProtocolStats::default(), ProtocolStats::default());
        let mut rows = Vec::<StatsRow>::with_capacity(1);

        stats.collect_rows(&map, &mut rows);

        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0],
            StatsRow::new(
                140,
                "librewolf",
                ProtocolStats::default(),
                ProtocolStats::default(),
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
                vec![proc_stats_to_bytes(
                    ProtocolStats::default(),
                    ProtocolStats::default(),
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

        let tcp = ProtocolStats {
            bytes: StatsBytes {
                sent: 100,
                recv: 200,
            },
            addr: test_addr(),
        };
        let udp = ProtocolStats {
            bytes: StatsBytes {
                sent: 400,
                recv: 1000,
            },
            addr: test_addr(),
        };

        let map = FakeStatsMap::new(7, tcp.clone(), udp.clone());
        let mut rows: Vec<StatsRow> = Vec::new();

        stats.collect_rows(&map, &mut rows);
        assert_eq!(rows[0].tcp, tcp);
        assert_eq!(rows[0].udp, udp);

        let tcp2 = ProtocolStats {
            bytes: StatsBytes {
                sent: tcp.bytes.sent + 20,
                recv: tcp.bytes.recv + 10,
            },
            addr: test_addr(),
        };
        let udp2 = ProtocolStats {
            bytes: StatsBytes {
                sent: udp.bytes.sent + 20,
                recv: udp.bytes.recv + 10,
            },
            addr: test_addr(),
        };

        let map = FakeStatsMap::new(7, tcp2.clone(), udp2.clone());

        let tcp_delta = StatsBytes {
            sent: tcp2.bytes.sent.saturating_sub(tcp.bytes.sent),
            recv: tcp2.bytes.recv.saturating_sub(tcp.bytes.recv),
        };
        let udp_delta = StatsBytes {
            sent: udp2.bytes.sent.saturating_sub(udp.bytes.sent),
            recv: udp2.bytes.recv.saturating_sub(udp.bytes.recv),
        };

        stats.collect_rows(&map, &mut rows);
        assert_eq!(rows[0].tcp.bytes, tcp_delta);
        assert_eq!(rows[0].udp.bytes, udp_delta);
    }

    #[test]
    fn merge_values_for_pid_success() {
        let tcp = ProtocolStats {
            bytes: StatsBytes {
                sent: 50,
                recv: 110,
            },
            addr: test_addr(),
        };
        let udp = ProtocolStats {
            bytes: StatsBytes {
                sent: 10,
                recv: 500,
            },
            addr: StatsAddr::default(),
        };

        let map = FakeStatsMap::new_two_values(3, tcp.clone(), udp.clone());

        let merged = merge_values_for_pid(&map, 3).unwrap();

        assert_eq!(
            merged,
            ProcStats {
                tcp: ProtocolStats {
                    bytes: StatsBytes {
                        sent: tcp.bytes.sent + 10,
                        recv: tcp.bytes.recv + 10
                    },
                    addr: test_addr()
                },
                udp: ProtocolStats {
                    bytes: StatsBytes {
                        sent: udp.bytes.sent + 10,
                        recv: udp.bytes.recv + 10
                    },
                    addr: StatsAddr::default()
                },
            }
        );
    }

    #[test]
    fn merge_values_for_pid_keeps_endpoint_snapshots_atomic() {
        let ipv4_addr = StatsAddr::new(
            443,
            49152,
            u32::from_be_bytes([192, 0, 2, 1]).to_be(),
            [0; 16],
        );
        let ipv6_addr = StatsAddr::new(
            53,
            49153,
            0,
            [0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
        );

        let first_tcp = ProtocolStats::new(StatsBytes::new(50, 110), ipv4_addr.clone());
        let first_udp = ProtocolStats::new(StatsBytes::new(10, 500), ipv6_addr.clone());
        let second_tcp = ProtocolStats::new(StatsBytes::new(20, 30), ipv6_addr);
        let second_udp = ProtocolStats::new(StatsBytes::new(40, 60), ipv4_addr);

        let mut map = FakeStatsMap {
            data: HashMap::new(),
        };
        map.data.insert(
            3_u32.to_ne_bytes().to_vec(),
            vec![
                proc_stats_to_bytes(first_tcp.clone(), first_udp.clone()),
                proc_stats_to_bytes(second_tcp, second_udp),
            ],
        );

        let merged = merge_values_for_pid(&map, 3).unwrap();

        assert_eq!(merged.tcp.bytes, StatsBytes::new(70, 140));
        assert_eq!(merged.udp.bytes, StatsBytes::new(50, 560));
        assert_eq!(merged.tcp.addr, first_tcp.addr);
        assert_eq!(merged.udp.addr, first_udp.addr);
    }

    #[test]
    fn proc_stats_from_bytes_valid_length() {
        let protocol_stats = ProtocolStats {
            bytes: StatsBytes { sent: 10, recv: 20 },
            addr: StatsAddr::default(),
        };

        let bytes = proc_stats_to_bytes(protocol_stats.clone(), protocol_stats.clone());

        let result = proc_stats_from_bytes(&bytes).unwrap();

        assert_eq!(
            result,
            ProcStats {
                tcp: protocol_stats.clone(),
                udp: protocol_stats,
            }
        );

        assert_eq!(proc_stats_from_bytes(&[0u8; 3]), None);
    }
}
