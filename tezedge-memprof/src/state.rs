use std::{fmt, sync::{Arc, atomic::{Ordering, AtomicBool, AtomicU64}}, time::Duration};

pub struct AtomicState {
    pub running: AtomicBool,
    physical: AtomicU64,
    free_events: AtomicU64,
    false_free_events: AtomicU64,
    last_events: AtomicU64,
    total_events: AtomicU64,
    page_faults: AtomicU64,
}

pub struct Report {
    usage: u64,
    events: f64,
    total_events: u64,
    free_events: u64,
    false_free_events: u64,
    page_faults: u64,
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mib = self.usage as f64 / (0x100000 as f64);
        let pf = (self.page_faults as f64) / (0x100 as f64);

        write!(
            f,
            "phys: {:.2} MiB, events per second: {:.2}, total events: {}, free events: {}, false free events: {}, page faults: {} ({:.2} MiB)",
            &mib,
            &self.events,
            &self.total_events,
            &self.free_events,
            &self.false_free_events,
            &self.page_faults,
            &pf,
        )
    }
}

impl AtomicState {
    pub fn new() -> Arc<Self> {
        Arc::new(AtomicState {
            running: AtomicBool::new(true),
            physical: AtomicU64::new(0),
            free_events: AtomicU64::new(0),
            false_free_events: AtomicU64::new(0),
            last_events: AtomicU64::new(0),
            total_events: AtomicU64::new(0),
            page_faults: AtomicU64::new(0),
        })
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed)
    }

    pub fn running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    pub fn count_physical_alloc(&self, bytes: u64) {
        self.physical.fetch_add(bytes, Ordering::SeqCst);
        self.total_events.fetch_add(1, Ordering::SeqCst);
    }

    pub fn count_free_event(&self, success: bool) {
        if success {
            self.free_events.fetch_add(1, Ordering::SeqCst);
        } else {
            self.false_free_events.fetch_add(1, Ordering::SeqCst);
        }
    }

    pub fn count_physical_free(&self, bytes: u64) {
        self.physical.fetch_sub(bytes, Ordering::SeqCst);
        self.total_events.fetch_add(1, Ordering::SeqCst);
    }

    pub fn count_page_fault(&self) {
        self.page_faults.fetch_add(1, Ordering::SeqCst);
    }

    pub fn observe(&self, elapsed_time: Duration) -> Report {
        let total_events = self.total_events.load(Ordering::SeqCst);
        let last_events = self.last_events
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |_| Some(total_events))
            .unwrap();
        let events = total_events - last_events;
        let events = (events as f64) / elapsed_time.as_secs_f64();
        Report {
            usage: self.physical.load(Ordering::SeqCst),
            events,
            total_events,
            free_events: self.free_events.load(Ordering::SeqCst),
            false_free_events: self.false_free_events.load(Ordering::SeqCst),
            page_faults: self.page_faults.load(Ordering::SeqCst),
        }
    }
}
