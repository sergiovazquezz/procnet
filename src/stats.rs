use std::{fs, rc::Rc};

use libbpf_rs::{MapCore, MapFlags, MapMut};

use crate::{events::ProcEvent, procnet::types::proc_stats};

struct ProcInfo {
    pub pid: u32,
    pub name: Rc<str>,
}

#[derive(Clone)]
pub struct StatsRow {
    pub pid: u32,
    pub name: Rc<str>,
    pub sent_bytes: u64,
    pub recv_bytes: u64,
    pub total_bytes: u64,
}

impl StatsRow {
    pub fn new(pid: u32, stats: proc_stats, name: Rc<str>) -> Self {
        Self {
            pid,
            name,
            sent_bytes: stats.sent_bytes,
            recv_bytes: stats.recv_bytes,
            total_bytes: stats.sent_bytes.saturating_add(stats.recv_bytes),
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
        let name = get_proc_name(event.pid).unwrap_or_else(|| comm_to_string(&event.comm));

        self.procs.push(ProcInfo {
            pid: event.pid,
            name: Rc::<str>::from(name),
        });
    }

    pub fn collect_rows(&mut self, stats_map: &MapMut) -> &[StatsRow] {
        self.rows.clear();

        for proc in &self.procs {
            if let Some(stats) = merge_values_for_pid(stats_map, proc.pid) {
                let row = StatsRow::new(proc.pid, stats, Rc::clone(&proc.name));
                self.rows.push(row);
            }
        }

        self.rows.sort_by(|a, b| {
            b.total_bytes
                .cmp(&a.total_bytes)
                .then_with(|| a.pid.cmp(&b.pid))
        });

        &self.rows
    }
}

pub fn merge_values_for_pid(stats_map: &MapMut, pid: u32) -> Option<proc_stats> {
    let key = pid.to_ne_bytes();

    let per_cpu_values = stats_map.lookup_percpu(&key, MapFlags::ANY).ok()??;

    let mut merged = proc_stats::default();

    for value in per_cpu_values {
        if let Some(value) = proc_stats_from_bytes(&value) {
            merged.recv_bytes += value.recv_bytes;
            merged.sent_bytes += value.sent_bytes;
        }
    }

    Some(merged)
}

fn proc_stats_from_bytes(data: &[u8]) -> Option<proc_stats> {
    if data.len() != size_of::<proc_stats>() {
        return None;
    }

    let stats = unsafe { data.as_ptr().cast::<proc_stats>().read_unaligned() };

    Some(stats)
}

fn get_proc_name(pid: u32) -> Option<String> {
    let exe_path = format!("/proc/{pid}/exe");
    if let Ok(target) = fs::read_link(exe_path)
        && let Some(name) = target.file_name().and_then(|name| name.to_str())
        && !name.is_empty()
    {
        return Some(name.to_owned());
    }

    let comm_path = format!("/proc/{pid}/comm");
    if let Ok(comm) = fs::read_to_string(comm_path) {
        let trimmed = comm.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_owned());
        }
    }

    None
}

fn comm_to_string(comm: &[u8; 16]) -> String {
    let end = comm.iter().position(|b| *b == 0).unwrap_or(comm.len());
    String::from_utf8_lossy(&comm[..end]).into_owned()
}
