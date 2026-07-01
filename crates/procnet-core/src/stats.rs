use serde::{Deserialize, Serialize};

use crate::events::ProcStartEvent;

#[repr(C)]
#[derive(Debug, Default, Clone, PartialEq)]
struct ProcStats {
    pub sent_bytes: u64,
    pub recv_bytes: u64,
}

/// Abstraction over a per-CPU BPF map lookup so the stats collector can be
/// tested and used without a direct `libbpf-rs` dependency.
pub trait StatsMap {
    fn lookup_percpu(&self, key: &[u8]) -> Option<Vec<Vec<u8>>>;
}

#[derive(Debug)]
struct ProcInfo {
    pid: u32,
    name: String,
    sent_cum: u64,
    recv_cum: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct StatsRow {
    pub pid: u32,
    pub name: String,
    pub sent_bytes: u64,
    pub recv_bytes: u64,
    pub total_bytes: u64,
}

impl StatsRow {
    #[must_use]
    pub const fn new(pid: u32, name: String, sent_bytes: u64, recv_bytes: u64) -> Self {
        Self {
            pid,
            name,
            sent_bytes,
            recv_bytes,
            total_bytes: sent_bytes.saturating_add(recv_bytes),
        }
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
            procs: Vec::with_capacity(20),
        }
    }

    pub fn apply_event(&mut self, mut event: ProcStartEvent) {
        event.name.make_ascii_lowercase();

        if let Some(p) = self.procs.iter_mut().find(|p| p.pid == event.pid) {
            p.name = event.name;
            p.sent_cum = 0;
            p.recv_cum = 0;
        } else {
            self.procs.push(ProcInfo {
                pid: event.pid,
                name: event.name,
                sent_cum: 0,
                recv_cum: 0,
            });
        }
    }

    pub fn collect_rows(&mut self, stats_map: &impl StatsMap, out: &mut Vec<StatsRow>) {
        out.clear();

        self.procs.retain_mut(|proc_info| {
            if let Some(new_stats) = merge_values_for_pid(stats_map, proc_info.pid) {
                let sent_delta = new_stats.sent_bytes.saturating_sub(proc_info.sent_cum);
                let recv_delta = new_stats.recv_bytes.saturating_sub(proc_info.recv_cum);

                proc_info.sent_cum = new_stats.sent_bytes;
                proc_info.recv_cum = new_stats.recv_bytes;

                out.push(StatsRow::new(
                    proc_info.pid,
                    proc_info.name.clone(),
                    sent_delta,
                    recv_delta,
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
            merged.recv_bytes = merged.recv_bytes.saturating_add(value.recv_bytes);
            merged.sent_bytes = merged.sent_bytes.saturating_add(value.sent_bytes);
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
        fn new(pid: u32, sent: u64, recv: u64) -> Self {
            let mut map = Self {
                data: HashMap::new(),
            };

            map.data.insert(
                pid.to_ne_bytes().to_vec(),
                vec![proc_stats_bytes(sent, recv)],
            );

            map
        }

        /// Creates two vectors for a given `pid`, the second having `10` for
        /// sent and recv.
        fn new_two_values(pid: u32, sent: u64, recv: u64) -> Self {
            let mut map = Self {
                data: HashMap::new(),
            };

            map.data.insert(
                pid.to_ne_bytes().to_vec(),
                vec![proc_stats_bytes(sent, recv), proc_stats_bytes(10, 10)],
            );

            map
        }

        fn remove(&mut self, pid: u32) {
            self.data.remove(pid.to_ne_bytes().as_slice());
        }
    }

    fn proc_stats_bytes(sent: u64, recv: u64) -> Vec<u8> {
        let mut v: Vec<u8> = Vec::with_capacity(size_of::<ProcStats>());

        v.extend_from_slice(&sent.to_ne_bytes());
        v.extend_from_slice(&recv.to_ne_bytes());

        v
    }

    #[test]
    fn stats_row_new_saturates_on_overflow() {
        let row = StatsRow::new(130, "firefox".into(), u64::MAX, 10);
        assert_eq!(row.total_bytes, u64::MAX);
    }

    #[test]
    fn apply_event_pushes_and_is_lowercase() {
        let mut stats = StatsCollector::default();
        stats.apply_event(ProcStartEvent {
            pid: 10,
            name: "GOOGLE".into(),
        });

        assert_eq!(stats.procs[0].name, "google".to_string());

        let (sent, recv) = (100, 200);
        let map = FakeStatsMap::new(10, sent, recv);

        let mut rows: Vec<StatsRow> = Vec::new();

        stats.collect_rows(&map, &mut rows);

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "google".to_string());
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
        assert_eq!(stats.procs[0].name, "second");
        assert_eq!(stats.procs[0].sent_cum, 0);
        assert_eq!(stats.procs[0].recv_cum, 0);
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

        let mut map = FakeStatsMap::new(140, 10, 20);
        let mut rows = Vec::<StatsRow>::with_capacity(1);

        stats.collect_rows(&map, &mut rows);

        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0],
            StatsRow {
                pid: 140,
                name: "librewolf".into(),
                sent_bytes: 10,
                recv_bytes: 20,
                total_bytes: 30
            }
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

        let (sent, recv) = (100, 200);
        let map = FakeStatsMap::new(7, sent, recv);
        let mut rows: Vec<StatsRow> = Vec::new();

        stats.collect_rows(&map, &mut rows);
        assert_eq!(rows[0].sent_bytes, sent);
        assert_eq!(rows[0].recv_bytes, recv);

        let (sent2, recv2) = (sent + 20, recv + 10);
        let map = FakeStatsMap::new(7, sent2, recv2);

        stats.collect_rows(&map, &mut rows);
        assert_eq!(rows[0].sent_bytes, sent2.saturating_sub(sent));
        assert_eq!(rows[0].recv_bytes, recv2.saturating_sub(recv));

        let (sent3, recv3) = (sent, recv + 50);
        let map = FakeStatsMap::new(7, sent3, recv3);

        stats.collect_rows(&map, &mut rows);
        assert_eq!(rows[0].sent_bytes, sent3.saturating_sub(sent2));
        assert_eq!(rows[0].recv_bytes, recv3.saturating_sub(recv2));
    }

    #[test]
    fn merge_values_for_pid_success() {
        let map = FakeStatsMap::new_two_values(3, 50, 110);

        let merged = merge_values_for_pid(&map, 3).unwrap();

        assert_eq!(
            merged,
            ProcStats {
                sent_bytes: 50 + 10,
                recv_bytes: 110 + 10
            }
        );
    }

    #[test]
    fn proc_stats_from_bytes_valid_length() {
        let mut bytes: Vec<u8> = Vec::with_capacity(size_of::<ProcStats>());
        let (sent_bytes, recv_bytes) = (10u64, 20u64);

        bytes.extend_from_slice(&sent_bytes.to_ne_bytes());
        bytes.extend_from_slice(&recv_bytes.to_ne_bytes());

        let result = proc_stats_from_bytes(&bytes).unwrap();

        assert_eq!(
            result,
            ProcStats {
                sent_bytes,
                recv_bytes
            }
        );

        assert_eq!(proc_stats_from_bytes(&[0u8; 3]), None);
    }
}
