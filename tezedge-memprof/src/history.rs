use std::vec::IntoIter;
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
            EventKind::PageAlloc(v) => Some(PageEvent {
                pfn: v.pfn,
                pages: (1 << v.order) as i32,
                stack,
                flavour: 0,
            }),
            EventKind::PageAllocZoneLocked(v) => Some(PageEvent {
                pfn: v.pfn,
                pages: (1 << v.order) as i32,
                stack,
                flavour: 1,
            }),
            EventKind::PageAllocExtFrag(v) => Some(PageEvent {
                pfn: v.pfn,
                pages: 1,
                stack,
                flavour: 2,
            }),
            EventKind::PageFree(v) => Some(PageEvent {
                pfn: v.pfn,
                pages: -((1 << v.order) as i32),
                stack,
                flavour: 3,
            }),
            EventKind::PageFreeBatched(v) => Some(PageEvent {
                pfn: v.pfn,
                pages: -1,
                stack,
                flavour: 4,
            }),
            EventKind::PagePcpuDrain(v) => Some(PageEvent {
                pfn: v.pfn,
                pages: -((1 << v.order) as i32),
                stack,
                flavour: 5,
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
}

impl IntoIterator for History {
    type IntoIter = IntoIter<PageEvent>;
    type Item = PageEvent;

    fn into_iter(self) -> Self::IntoIter {
        self.v.into_iter()
    }
}
