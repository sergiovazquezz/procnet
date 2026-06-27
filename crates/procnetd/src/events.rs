use std::{cell::RefCell, fs, rc::Rc};

use anyhow::Result;
use libbpf_rs::{MapMut, RingBuffer, RingBufferBuilder};
use procnet_core::events::ProcEvent;

pub const EVENT_START: u32 = 1;
pub const EVENT_EXIT: u32 = 2;

/// The `ProcEvent` struct from the bpf.c code, since it isn't generated
/// automatically.
#[repr(C)]
pub struct ProcEventBpf {
    pub event_type: u32,
    pub pid: u32,
    pub comm: [u8; 16],
}

pub struct EventReader<'a> {
    ringbuf: RingBuffer<'a>,
    queue: Rc<RefCell<Vec<ProcEvent>>>,
}

impl<'a> EventReader<'a> {
    pub fn new(events_map: &'a MapMut) -> Result<Self> {
        let queue = Rc::new(RefCell::new(Vec::<ProcEvent>::new()));
        let callback_queue = Rc::clone(&queue);

        let mut builder = RingBufferBuilder::new();

        builder.add(events_map, move |data: &[u8]| {
            if let Some(event) = parse_proc_event(data) {
                callback_queue.borrow_mut().push(event);
            }

            0i32
        })?;

        let ringbuf = builder.build()?;

        Ok(EventReader { ringbuf, queue })
    }

    /// Consume all pending events, then collect all the pids from the queue.
    pub fn drain_available(&self) -> Result<Vec<ProcEvent>> {
        self.ringbuf.consume()?;

        Ok(self.queue.borrow_mut().drain(..).collect())
    }
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
