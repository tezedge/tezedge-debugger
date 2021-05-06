use std::{collections::HashMap, slice::Iter, vec::IntoIter};
use serde::{Serialize, Deserialize};
use bpf_memprof::{Event, EventKind, Hex64, Stack};

#[derive(Serialize, Deserialize, PartialEq, Eq)]
pub struct PageEvent {
    pub pfn: Hex64,
    pub pages: i32,
    pub stack: Stack,
    pub flavour: u32,
}

impl PageEvent {
    pub fn try_from(e: Event) -> Option<Self> {
        let Event { event, stack, .. } = e;
        match event {
            EventKind::PageAlloc(v) if v.pfn.0 != 0 => Some(PageEvent {
                pfn: v.pfn,
                pages: (1 << v.order) as i32,
                stack,
                flavour: 0,
            }),
            EventKind::PageFree(v) if v.pfn.0 != 0 => Some(PageEvent {
                pfn: v.pfn,
                pages: -((1 << v.order) as i32),
                stack,
                flavour: 3,
            }),
            _ => None,
        }
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct History {
    v: Vec<PageEvent>,
}

impl History {
    pub fn len(&self) -> usize {
        self.v.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn push(&mut self, event: PageEvent) {
        self.v.push(event)
    }

    pub fn reorder(&mut self, distance: usize) {
        if self.v.is_empty() || distance == 0 {
            return;
        }

        let mut k = 0;
        let l = self.v.len();
        for i in 0..(l - 1) {
            for j in (i + 1)..(i + 1 + distance).min(l) {
                if self.v[i].pfn == self.v[j].pfn && self.v[i].pages < 0 && self.v[i].pages + self.v[j].pages == 0 {
                    self.v.swap(i, j);
                    k += 1;
                }
            }
        }

        log::info!("reorder: {}", k);
    }

    pub fn iter(&self) -> Iter<'_, PageEvent> {
        self.v.iter()
    }
}

impl IntoIterator for History {
    type IntoIter = IntoIter<PageEvent>;
    type Item = PageEvent;

    fn into_iter(self) -> Self::IntoIter {
        self.v.into_iter()
    }
}

#[derive(Serialize)]
pub struct Frame {
    value: i64,
    frames: HashMap<Hex64, Frame>,
    #[serde(skip)]
    allocations: Vec<Allocation>,
}

#[allow(dead_code)]
#[derive(Serialize)]
pub struct Allocation {
    pfn: Hex64,
    pages: i32,
    flavour: u32,
}

impl Allocation {
    fn new(event: &PageEvent) -> Self {
        Allocation {
            pfn: event.pfn,
            pages: event.pages,
            flavour: event.flavour,
        }
    }
}

impl Frame {
    pub fn empty() -> Self {
        Frame {
            frames: HashMap::new(),
            allocations: Vec::new(),
            value: 0,
        }
    }

    pub fn insert(&mut self, event: &PageEvent) {
        let mut node = self;
        for stack_frame in event.stack.ips() {
            node = node.frames.entry(*stack_frame).or_insert(Frame::empty());
        }
        node.allocations.push(Allocation::new(event));
    }

    fn sum(&self) -> i64 {
        self.allocations.iter().map(|a| a.pages as i64).sum::<i64>()
    }

    fn total_sum(&self) -> (i64, i64) {
        let s = self.sum();
        (s, s + self.frames.iter().map(|(_, v)| v.total_sum().1).sum::<i64>())
    }

    pub fn strip(&mut self) {
        self.frames.values_mut().for_each(Frame::strip);
        self.frames.retain(|_, v| {
            let (s, t) = v.total_sum();
            v.value = t;
            !(s == 0 && v.frames.is_empty())
        });
    }
}
