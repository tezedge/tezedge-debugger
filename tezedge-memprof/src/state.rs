use std::{fmt, sync::{Arc, atomic::{Ordering, AtomicBool, AtomicU64}}, time::Duration};

#[derive(Clone, Default, Debug)]
struct Counters<T> {
    slab_unknown_bytes: T,
    slab_unknown_alloc_count: T,
    slab_unknown_free_count: T,
    slab_unknown_bad_free_count: T,
    slab_known_bytes: T,
    slab_known_alloc_count: T,
    slab_known_free_count: T,
    slab_known_bad_free_count: T,
    page_bytes: T,
    page_alloc_count: T,
    page_free_count: T,
    page_fault_count: T,
    rss_stat_count: T,
    rss_stat_file_bytes: T,
    rss_stat_anon_bytes: T,
    rss_stat_swap_bytes: T,
    rss_stat_shared_bytes: T,
}

impl Counters<AtomicU64> {
    pub fn load(&self) -> Counters<u64> {
        Counters {
            slab_unknown_bytes: self.slab_unknown_bytes.load(Ordering::SeqCst),
            slab_unknown_alloc_count: self.slab_unknown_alloc_count.load(Ordering::SeqCst),
            slab_unknown_free_count: self.slab_unknown_free_count.load(Ordering::SeqCst),
            slab_unknown_bad_free_count: self.slab_unknown_bad_free_count.load(Ordering::SeqCst),
            slab_known_bytes: self.slab_known_bytes.load(Ordering::SeqCst),
            slab_known_alloc_count: self.slab_known_alloc_count.load(Ordering::SeqCst),
            slab_known_free_count: self.slab_known_free_count.load(Ordering::SeqCst),
            slab_known_bad_free_count: self.slab_known_bad_free_count.load(Ordering::SeqCst),
            page_bytes: self.page_bytes.load(Ordering::SeqCst),
            page_alloc_count: self.page_alloc_count.load(Ordering::SeqCst),
            page_free_count: self.page_free_count.load(Ordering::SeqCst),
            page_fault_count: self.page_fault_count.load(Ordering::SeqCst),
            rss_stat_count: self.rss_stat_count.load(Ordering::SeqCst),
            rss_stat_file_bytes: self.rss_stat_file_bytes.load(Ordering::SeqCst),
            rss_stat_anon_bytes: self.rss_stat_anon_bytes.load(Ordering::SeqCst),
            rss_stat_swap_bytes: self.rss_stat_swap_bytes.load(Ordering::SeqCst),
            rss_stat_shared_bytes: self.rss_stat_shared_bytes.load(Ordering::SeqCst),
        }
    }
}

impl Counters<u64> {
    pub fn diff(&self, other: &Self, elapsed_time: Duration) -> Counters<f64> {
        let d = |a: u64, b: u64| (a as f64 - b as f64) / elapsed_time.as_secs_f64();
        Counters {
            slab_unknown_bytes: d(self.slab_unknown_bytes, other.slab_unknown_bytes),
            slab_unknown_alloc_count: d(self.slab_unknown_alloc_count, other.slab_unknown_alloc_count),
            slab_unknown_free_count: d(self.slab_unknown_free_count, other.slab_unknown_free_count),
            slab_unknown_bad_free_count: d(self.slab_unknown_bad_free_count, other.slab_unknown_bad_free_count),
            slab_known_bytes: d(self.slab_known_bytes, other.slab_known_bytes),
            slab_known_alloc_count: d(self.slab_known_alloc_count, other.slab_known_alloc_count),
            slab_known_free_count: d(self.slab_known_free_count, other.slab_known_free_count),
            slab_known_bad_free_count: d(self.slab_known_bad_free_count, other.slab_known_bad_free_count),
            page_bytes: d(self.page_bytes, other.page_bytes),
            page_alloc_count: d(self.page_alloc_count, other.page_alloc_count),
            page_free_count: d(self.page_free_count, other.page_free_count),
            page_fault_count: d(self.page_fault_count, other.page_fault_count),
            rss_stat_count: d(self.rss_stat_count, self.rss_stat_count),
            rss_stat_file_bytes: d(self.rss_stat_file_bytes, self.rss_stat_file_bytes),
            rss_stat_anon_bytes: d(self.rss_stat_anon_bytes, self.rss_stat_anon_bytes),
            rss_stat_swap_bytes: d(self.rss_stat_swap_bytes, self.rss_stat_swap_bytes),
            rss_stat_shared_bytes: d(self.rss_stat_shared_bytes, self.rss_stat_shared_bytes),
        }
    }
}

pub struct AtomicState {
    running: AtomicBool,
    counters: Counters<AtomicU64>,
}

pub struct Reporter {
    atomic_state: Arc<AtomicState>,
    counters: Counters<u64>,
}

impl Reporter {
    pub fn new(atomic_state: Arc<AtomicState>) -> Self {
        Reporter {
            atomic_state,
            counters: Counters::default(),
        }
    }

    pub fn running(&self) -> bool {
        self.atomic_state.running.load(Ordering::Relaxed)
    }

    pub fn report(&mut self, elapsed_time: Duration) -> Report {
        let last_counters = self.counters.clone();
        let current_counters = self.atomic_state.counters.load();
        self.counters = current_counters.clone();
        Report { last_counters, current_counters, elapsed_time }
    }
}

pub struct Report {
    last_counters: Counters<u64>,
    current_counters: Counters<u64>,
    elapsed_time: Duration,
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let usage = self.current_counters.page_bytes;
        let mib = usage as f64 / (0x400 as f64);
        let rss_file_mib = self.current_counters.rss_stat_file_bytes as f64 / (0x400 as f64);
        let rss_anon_mib = self.current_counters.rss_stat_anon_bytes as f64 / (0x400 as f64);
        let rss_swap_mib = self.current_counters.rss_stat_swap_bytes as f64 / (0x400 as f64);
        let rss_shared_mib = self.current_counters.rss_stat_shared_bytes as f64 / (0x400 as f64);
        let _diff = self.current_counters.diff(&self.last_counters, self.elapsed_time);
        write!(
            f,
            "usage: {:.2} kiB, rss: {:.2} (file: {:.2} + anon: {:.2} + swap: {:.2} + shared: {:.2}) kiB\n{:#?}\n",
            mib,
            rss_file_mib + rss_anon_mib + rss_swap_mib + rss_shared_mib,
            rss_file_mib,
            rss_anon_mib,
            rss_swap_mib,
            rss_shared_mib,
            &self.current_counters,
        )
    }
}

impl AtomicState {
    pub fn new() -> Arc<Self> {
        Arc::new(AtomicState {
            running: AtomicBool::new(true),
            counters: Counters {
                slab_unknown_bytes: AtomicU64::new(0),
                slab_unknown_alloc_count: AtomicU64::new(0),
                slab_unknown_free_count: AtomicU64::new(0),
                slab_unknown_bad_free_count: AtomicU64::new(0),
                slab_known_bytes: AtomicU64::new(0),
                slab_known_alloc_count: AtomicU64::new(0),
                slab_known_free_count: AtomicU64::new(0),
                slab_known_bad_free_count: AtomicU64::new(0),
                page_bytes: AtomicU64::new(0),
                page_alloc_count: AtomicU64::new(0),
                page_free_count: AtomicU64::new(0),
                page_fault_count: AtomicU64::new(0),
                rss_stat_count: AtomicU64::new(0),
                rss_stat_file_bytes: AtomicU64::new(0),
                rss_stat_anon_bytes: AtomicU64::new(0),
                rss_stat_swap_bytes: AtomicU64::new(0),
                rss_stat_shared_bytes: AtomicU64::new(0),
            },
        })
    }

    pub fn running_ref(&self) -> &AtomicBool {
        &self.running
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed)
    }

    pub fn running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    pub fn slab_unknown_alloc(&self, bytes: u64) {
        self.counters.slab_unknown_alloc_count.fetch_add(1, Ordering::SeqCst);
        self.counters.slab_unknown_bytes.fetch_add(bytes, Ordering::SeqCst);
    }

    pub fn slab_unknown_free(&self, bytes: u64, success: bool) {
        self.counters.slab_unknown_bytes.fetch_sub(bytes, Ordering::SeqCst);
        let ct = if success {
            &self.counters.slab_unknown_free_count
        } else {
            &self.counters.slab_unknown_bad_free_count
        };
        ct.fetch_add(1, Ordering::SeqCst);
    }

    pub fn slab_known_alloc(&self, bytes: u64) {
        self.counters.slab_known_alloc_count.fetch_add(1, Ordering::SeqCst);
        self.counters.slab_known_bytes.fetch_add(bytes, Ordering::SeqCst);
    }

    pub fn slab_known_free(&self, bytes: u64, success: bool) {
        self.counters.slab_known_bytes.fetch_sub(bytes, Ordering::SeqCst);
        let ct = if success {
            &self.counters.slab_known_free_count
        } else {
            &self.counters.slab_known_bad_free_count
        };
        ct.fetch_add(1, Ordering::SeqCst);
    }

    pub fn page_alloc(&self, bytes: u64) {
        self.counters.page_alloc_count.fetch_add(1, Ordering::SeqCst);
        self.counters.page_bytes.fetch_add(bytes, Ordering::SeqCst);
    }

    pub fn page_free(&self, bytes: u64) {
        self.counters.page_bytes.fetch_sub(bytes, Ordering::SeqCst);
        self.counters.page_free_count.fetch_add(1, Ordering::SeqCst);
    }

    pub fn page_fault(&self) {
        self.counters.page_fault_count.fetch_add(1, Ordering::SeqCst);
    }

    pub fn rss_stat(&self, bytes: i64, member: i32) {
        self.counters.rss_stat_count.fetch_add(1, Ordering::SeqCst);
        let ct = match member {
            0 => &self.counters.rss_stat_file_bytes,
            1 => &self.counters.rss_stat_anon_bytes,
            2 => &self.counters.rss_stat_swap_bytes,
            3 => &self.counters.rss_stat_shared_bytes,
            _ => {
                log::warn!("rss stat unknown member: {}", member);
                return;
            },
        };
        let bytes = if bytes < 0 {
            log::warn!("rss stat yields negative {}", bytes);
            0
        } else {
            bytes as u64
        };
        ct.store(bytes, Ordering::SeqCst);
    }
}
