use std::fs;

pub const EVENT_START: u32 = 1;
pub const EVENT_EXIT: u32 = 2;

/// The "proc_event" struct from the bpf.c code, since it isn't generated
/// automatically.
#[repr(C)]
pub struct ProcEventBpf {
    pub event_type: u32,
    pub pid: u32,
    pub comm: [u8; 16],
}

#[derive(Clone)]
pub enum ProcEvent {
    Start { pid: u32, name: String },
    Exit(u32),
}

pub fn parse_proc_event(data: &[u8]) -> Option<ProcEvent> {
    if data.len() != size_of::<ProcEventBpf>() {
        return None;
    }

    let raw = unsafe { data.as_ptr().cast::<ProcEventBpf>().read_unaligned() };

    match raw.event_type {
        EVENT_START => {
            let name = get_proc_name(raw.pid).unwrap_or_else(|| comm_to_string(&raw.comm));
            Some(ProcEvent::Start { pid: raw.pid, name })
        }
        EVENT_EXIT => Some(ProcEvent::Exit(raw.pid)),
        _ => None,
    }
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
