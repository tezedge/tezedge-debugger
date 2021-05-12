use std::{collections::HashMap, fmt, ops::Range, time::{SystemTime, Duration}};
use serde::{Serialize, ser};
use bpf_memprof::{Hex32, Hex64, Stack};
use super::stack::StackResolver;

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
    ranges: Vec<(Range<u64>, Hex32)>,
}

pub trait HistoryFilter {
    fn keep(&self, history: impl Iterator<Item = Range<u64>>) -> bool;
}

#[derive(Default, Serialize)]
pub struct History {
    double_free: Vec<Page>,
    free_without_alloc: Vec<Page>,
    double_alloc: Vec<Page>,
    inner: HashMap<Page, PageHistory>,
    group: HashMap<Vec<Hex64>, Vec<Page>>,
}

impl History {
    pub fn process(&mut self, page: Page, misc: Option<(&Stack, Hex32)>) {
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or(Duration::default())
            .as_millis() as u64;

        let e = self.inner.entry(page.clone()).or_default();
        let new = match (e.ranges.last_mut(), &misc) {
            (Some((range, _)), &None) if range.end == u64::MAX => {
                range.end = timestamp;
                None
            },
            (Some((range, _)), &None) => {
                self.double_free.push(page.clone());
                range.end = timestamp;
                None
            },
            (None, &None) => {
                self.free_without_alloc.push(page.clone());
                Some((0..timestamp, Hex32(0)))
            },
            (Some((range, _)), &Some(_)) if range.end == u64::MAX => {
                self.double_alloc.push(page.clone());
                None
            },
            (_, &Some((_, ref flags))) => {
                Some((timestamp..u64::MAX, *flags))
            },
        };
        if let Some(new) = new {
            // keep only last record in the history, remove this line to preserve
            e.ranges.clear();
            e.ranges.push(new);
        }

        if let Some((stack, _)) = misc {
            let pages_group = self.group.entry(stack.ips().to_vec()).or_default();
            pages_group.push(page);
        }
    }

    pub fn tree_report<F>(
        &self,
        resolver: &StackResolver,
        filter: &F,
        threshold: u64,
    ) -> FrameReport
    where
        F: HistoryFilter,
    {
        let mut report = FrameReport::default();
        for (stack, group) in &self.group {
            let mut value = 0;
            for page in group {
                let history = self.inner.get(page).unwrap();
                if filter.keep(history.ranges.iter().map(|&(ref r, _)| r.clone())) {
                    value += 1u64 << (page.order + 2);
                }
            }
            if value >= threshold {
                report.insert(resolver, &stack, value);
            }
        }

        report
    }

    pub fn short_report<F>(&self, filter: &F) -> ShortReport
    where
        F: HistoryFilter,
    {
        let mut report = ShortReport::default();
        for (page, history) in &self.inner {
            if filter.keep(history.ranges.iter().map(|&(ref r, _)| r.clone())) {
                report.kilobytes += 1 << (page.order + 2);
            }
            report.alloc_count += history.ranges.len() as u64;
            report.free_count += history.ranges.len() as u64 -
                history.ranges
                    .last()
                    .map(|&(ref r, _)| if r.end == u64::MAX { 1 } else { 0 })
                    .unwrap_or(0);
        }

        report.double_free_count = self.double_free.len() as _;
        report.without_alloc_count = self.free_without_alloc.len() as _;
        report.double_alloc_count = self.double_alloc.len() as _;

        report
    }

    pub fn flags_report(&self) -> FlagsReport {
        let mut report = FlagsReport::default();
        for (_, history) in &self.inner {
            if let Some(&(ref r, flags)) = history.ranges.last() {
                if r.end == u64::MAX {
                    *report.leak.entry(flags).or_default() += 1;
                } else {
                    *report.ok.entry(flags).or_default() += 1;
                }
            } 
        }
        
        report
    }
}

pub struct DefaultFilter;

impl HistoryFilter for DefaultFilter {
    fn keep(&self, history: impl Iterator<Item = Range<u64>>) -> bool {
        history.last().unwrap_or(0..0).end == u64::MAX
    }
}

#[derive(Default, Serialize)]
pub struct FrameReport {
    value: u64,
    frames: HashMap<String, FrameReport>,
}

impl FrameReport {
    fn insert(&mut self, resolver: &StackResolver, stack: &[Hex64], value: u64) {
        let mut node = self;
        for stack_frame in stack {
            let key = resolver.resolve(stack_frame.0);
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

#[derive(Default, Serialize)]
pub struct FlagsReport {
    ok: HashMap<Hex32, u32>,
    leak: HashMap<Hex32, u32>,
}

// filters

pub struct Filter {
    time_range: Range<u64>,
}

impl Filter {
    pub fn new(time_range: Range<u64>) -> Self {
        Filter { time_range }
    }
}

impl HistoryFilter for Filter {
    fn keep(&self, mut history: impl Iterator<Item = Range<u64>>) -> bool {
        history
            .find(|r| self.time_range.contains(&r.start) && r.end > self.time_range.end)
            .is_some()
    }
}
