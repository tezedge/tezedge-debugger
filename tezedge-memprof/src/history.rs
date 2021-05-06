use std::{collections::{HashMap, HashSet}, fmt, ops::Range, time::{SystemTime, Duration}};
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

#[derive(Serialize)]
pub enum PageError {
    DoubleFree(Hex64),
    FreeWithoutAlloc(Hex64),
    DoubleAlloc(Hex64),
}

#[derive(Default, Serialize)]
pub struct PageHistory {
    errors: Vec<PageError>,
    inner: HashMap<Page, Vec<Range<u64>>>,
    frame: Frame,
}

impl PageHistory {
    pub fn process(&mut self, page: Page, stack: Option<&Stack>) {
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or(Duration::default())
            .as_millis() as u64;

        let pfn = page.pfn;
        let e = self.inner.entry(page).or_default();
        let new = match (e.last_mut(), stack.is_some()) {
            (Some(range), false) if range.end == u64::MAX => {
                range.end = timestamp;
                None
            },
            (Some(range), false) => {
                self.errors.push(PageError::DoubleFree(pfn));
                range.end = timestamp;
                None
            },
            (None, false) => {
                self.errors.push(PageError::FreeWithoutAlloc(pfn));
                Some(0..timestamp)
            },
            (Some(range), true) if range.end == u64::MAX => {
                self.errors.push(PageError::DoubleAlloc(pfn));
                None
            },
            (_, true) => {
                Some(timestamp..u64::MAX)
            },
        };
        if let Some(new) = new {
            e.push(new);
        }

        if let Some(stack) = stack {
            self.frame.insert(stack, page);
        }
    }

    pub fn report<F>(&self, filter: &F) -> FrameReport
    where
        F: Fn(&[Range<u64>]) -> bool,
    {
        self.frame.report(&self.inner, filter)
    }
}

#[derive(Default, Serialize)]
struct Frame {
    pages: HashSet<Page>,
    frames: HashMap<Hex64, Frame>,
}

impl Frame {
    pub fn insert(&mut self, stack: &Stack, page: Page) {
        let mut node = self;
        for stack_frame in stack.ips() {
            node = node.frames.entry(*stack_frame).or_insert(Frame::default());
        }
        node.pages.insert(page);
    }

    pub fn report<F>(&self, history: &HashMap<Page, Vec<Range<u64>>>, filter: &F) -> FrameReport
    where
        F: Fn(&[Range<u64>]) -> bool,
    {
        let mut allocated_here = 0; // kilobytes
        for page in &self.pages {
            let ranges = history.get(page).unwrap();
            if filter(ranges) {
                allocated_here += 1 << (page.order + 2);
            }
        }

        let mut frames = HashMap::new();
        for (k, v) in self.frames.iter() {
            let r = v.report(history, filter);
            if r.value != 0 {
                allocated_here += r.value;
                frames.insert(format!("{:?}", k), r);
            }
        }

        FrameReport {
            value: allocated_here,
            frames,
        }
    }
}

#[derive(Default, Serialize)]
pub struct FrameReport {
    value: u64,
    frames: HashMap<String, FrameReport>,
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
