use std::{cell::RefCell, rc::Rc};

use anyhow::Result;
use libbpf_rs::{MapMut, RingBuffer, RingBufferBuilder};
use procnet_core::events::{ProcEvent, parse_proc_event};

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
    pub fn drain_available(&mut self) -> Result<Vec<ProcEvent>> {
        self.ringbuf.consume()?;

        Ok(self.queue.borrow_mut().drain(..).collect())
    }
}
