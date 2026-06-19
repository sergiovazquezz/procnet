use std::rc::Rc;

use libbpf_rs::{MapCore, MapFlags, MapMut};

use crate::{events::ProcEvent, procnet::types::ProcStats};

#[derive(Debug)]
struct ProcInfo {
    pid: u32,
    name: Rc<str>,
    last_sent_cum: u64,
    last_recv_cum: u64,
    stale_counter: u8,
}

#[derive(Clone, Debug)]
pub struct StatsRow {
    pub pid: u32,
    pub name: Rc<str>,
    pub sent_bytes: u64,
    pub recv_bytes: u64,
    pub total_bytes: u64,
}

impl StatsRow {
    pub fn new(pid: u32, name: Rc<str>, sent_bytes: u64, recv_bytes: u64) -> Self {
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
    rows: Vec<StatsRow>,
}

impl StatsCollector {
    pub fn new() -> Self {
        Self {
            procs: Vec::with_capacity(20),
            rows: Vec::with_capacity(20),
        }
    }

    pub fn apply_event(&mut self, event: ProcEvent) {
        match event {
            ProcEvent::Start { pid, name } => {
                self.procs.push(ProcInfo {
                    pid,
                    name: Rc::<str>::from(name),
                    last_recv_cum: 0,
                    last_sent_cum: 0,
                    stale_counter: 0,
                });
            }
            ProcEvent::Exit(pid) => {
                if let Some(idx) = self.procs.iter().position(|x| x.pid == pid) {
                    self.procs.swap_remove(idx);
                }
            }
        }
    }

    pub fn collect_rows(&mut self, stats_map: &MapMut) -> &[StatsRow] {
        self.rows.clear();

        self.procs.retain_mut(|proc_info| {
            if proc_info.stale_counter >= 2 {
                return false;
            }

            match merge_values_for_pid(stats_map, proc_info.pid) {
                Some(new_stats) => {
                    let sent_delta = new_stats.sent_bytes.saturating_sub(proc_info.last_sent_cum);
                    let recv_delta = new_stats.recv_bytes.saturating_sub(proc_info.last_recv_cum);

                    proc_info.last_sent_cum = new_stats.sent_bytes;
                    proc_info.last_recv_cum = new_stats.recv_bytes;

                    proc_info.stale_counter = 0;

                    self.rows.push(StatsRow::new(
                        proc_info.pid,
                        Rc::clone(&proc_info.name),
                        sent_delta,
                        recv_delta,
                    ));

                    true
                }
                None => {
                    proc_info.stale_counter += 1;

                    true
                }
            }
        });

        self.rows.sort_by(|a, b| {
            b.total_bytes
                .cmp(&a.total_bytes)
                .then_with(|| a.pid.cmp(&b.pid))
        });

        &self.rows
    }
}

pub fn merge_values_for_pid(stats_map: &MapMut, pid: u32) -> Option<ProcStats> {
    let key = pid.to_ne_bytes();

    let per_cpu_values = stats_map.lookup_percpu(&key, MapFlags::ANY).ok()??;

    let mut merged = ProcStats::default();

    for value in per_cpu_values {
        if let Some(value) = proc_stats_from_bytes(&value) {
            merged.recv_bytes = merged.recv_bytes.saturating_add(value.recv_bytes);
            merged.sent_bytes = merged.sent_bytes.saturating_add(value.sent_bytes);
            if merged.comm == [0u8; 16] && value.comm != [0u8; 16] {
                merged.comm = value.comm;
            }
        }
    }

    Some(merged)
}

fn proc_stats_from_bytes(data: &[u8]) -> Option<ProcStats> {
    if data.len() != size_of::<ProcStats>() {
        return None;
    }

    let stats = unsafe { data.as_ptr().cast::<ProcStats>().read_unaligned() };

    Some(stats)
}
