use std::{collections::HashMap, fmt, ops::Range, time::{SystemTime, Duration}};
use serde::{Serialize, ser};
use bpf_memprof::{Hex64, Stack};

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub struct Page {
    pfn: Hex64,
    order: u32,
}

impl fmt::Display for Page {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}-{}", self.pfn, self.order)
    }
}

impl Serialize for Page {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl Page {
    pub fn new(pfn: Hex64, order: u32) -> Self {
        Page { pfn, order }
    }
}

#[derive(Default, Serialize)]
struct PageHistory {
    ranges: Vec<Range<u64>>,
    stack: Vec<Hex64>,
}

#[derive(Default, Serialize)]
pub struct History {
    double_free: Vec<Page>,
    free_without_alloc: Vec<Page>,
    double_alloc: Vec<Page>,
    inner: HashMap<Page, PageHistory>,
}

impl History {
    pub fn process(&mut self, page: Page, stack: Option<&Stack>) {
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or(Duration::default())
            .as_millis() as u64;

        let e = self.inner.entry(page.clone()).or_default();
        let new = match (e.ranges.last_mut(), stack.is_some()) {
            (Some(range), false) if range.end == u64::MAX => {
                range.end = timestamp;
                None
            },
            (Some(range), false) => {
                self.double_free.push(page);
                range.end = timestamp;
                None
            },
            (None, false) => {
                self.free_without_alloc.push(page);
                Some(0..timestamp)
            },
            (Some(range), true) if range.end == u64::MAX => {
                self.double_alloc.push(page);
                None
            },
            (_, true) => {
                Some(timestamp..u64::MAX)
            },
        };
        if let Some(new) = new {
            e.ranges.push(new);
        }

        if let Some(stack) = stack {
            e.stack = stack.ips().to_vec();
        }
    }

    pub fn tree_report<F>(&self, filter: &F) -> FrameReport
    where
        F: Fn(&[Range<u64>]) -> bool,
    {
        let mut report = FrameReport::default();
        // TODO: optimize it, group pages in the same stack frame, and insert batch
        for (page, history) in &self.inner {
            if filter(&history.ranges) {
                report.insert(&history.stack, 1 << (page.order + 2));
            }
        }

        report
    }

    pub fn short_report<F>(&self, filter: &F) -> ShortReport
    where
        F: Fn(&[Range<u64>]) -> bool,
    {
        let mut report = ShortReport::default();
        for (page, history) in &self.inner {
            if filter(&history.ranges) {
                report.kilobytes += 1 << (page.order + 2);
            }
            report.alloc_count += history.ranges.len() as u64;
            report.free_count += history.ranges.len() as u64 -
                history.ranges
                    .last()
                    .map(|r| if r.end == u64::MAX { 1 } else { 0 })
                    .unwrap_or(0);
        }

        report.double_free_count = self.double_free.len() as _;
        report.without_alloc_count = self.free_without_alloc.len() as _;
        report.double_alloc_count = self.double_alloc.len() as _;

        report
    }
}

pub fn default_filter() -> impl Fn(&[Range<u64>]) -> bool {
    |ranges| ranges.last().unwrap_or(&(0..0)).end == u64::MAX
}

#[derive(Default, Serialize)]
pub struct FrameReport {
    value: u64,
    frames: HashMap<String, FrameReport>,
}

impl FrameReport {
    fn insert(&mut self, stack: &[Hex64], value: u64) {
        let mut node = self;
        for stack_frame in stack {
            let key = format!("{:?}", stack_frame);
            node.value += value;
            node = node.frames.entry(key).or_default();
        }
        node.value += value;
    }
}

#[derive(Default, Serialize)]
pub struct ShortReport {
    kilobytes: u64,
    alloc_count: u64,
    free_count: u64,
    #[serde(skip_serializing)]
    double_free_count: u64,
    #[serde(skip_serializing)]
    without_alloc_count: u64,
    #[serde(skip_serializing)]
    double_alloc_count: u64,
}

impl fmt::Display for ShortReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "kib: {}, alloc: {}, free: {}, double free: {}, without alloc: {}, double alloc: {}",
            self.kilobytes,
            self.alloc_count,
            self.free_count,
            self.double_free_count,
            self.without_alloc_count,
            self.double_alloc_count,
        )
    }
}

// filters

pub struct Filter {
    pub time_range: Range<u64>,
}

impl Filter {
    pub fn not_deallocated_in(&self, history: &[Range<u64>]) -> bool {
        history.iter()
            .find(|r| self.time_range.contains(&r.start) && r.end > self.time_range.end)
            .is_some()
    }
}
