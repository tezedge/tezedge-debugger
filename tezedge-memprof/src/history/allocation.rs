use std::{collections::{HashMap, HashSet}, ops::Deref};
use serde::Serialize;
use bpf_memprof::{Hex32, Stack};
use super::{
    page::Page,
    report::FrameReport,
    stack::StackResolver,
    history::StackShort,
    abstract_tracker::Tracker,
};

#[derive(Serialize, Hash, PartialEq, Eq, Clone)]
pub struct StackHash(u32);

impl StackHash {
    pub fn new(stack: &StackShort) -> Self {
        let mut hasher = crc32fast::Hasher::new();
        for frame in &stack.0 {
            hasher.update(&frame.0.to_ne_bytes());
        }
        StackHash(hasher.finalize())
    }
}

#[derive(Serialize)]
pub struct Usage {
    node: u32,
    cache: u32,
    stack: StackShort,
}

impl Usage {
    pub fn new(stack: StackShort) -> Self {
        Usage {
            node: 0,
            cache: 0,
            stack,
        }
    }

    pub fn decrease(&mut self, page: &Page) {
        if self.node < page.number() {
            panic!();
        }
        self.node -= page.number();
    }

    pub fn increase(&mut self, page: &Page) {
        self.node += page.number();
    }

    pub fn cache(&mut self, page: &Page, b: bool) {
        if b {
            self.cache += page.number();
        } else {
            if self.cache < page.number() {
                self.cache = 0;
                log::debug!("page {} was not marked as cache by mistake", page);
            } else {
                self.cache -= page.number();
            }
        }
    }
}

#[derive(Serialize)]
pub struct PageState {
    stack_hash: StackHash,
    for_cache: bool,
}

impl PageState {
    pub fn new(stack_hash: StackHash, for_cache: bool) -> Self {
        PageState {
            stack_hash,
            for_cache,
        }
    }
}

#[derive(Default, Serialize)]
pub struct Group {
    last_stack: HashMap<Page, PageState>,
    group: HashMap<StackHash, Usage>,
    collision_detector: HashSet<StackShort>,
}

impl Group {
    pub fn insert(&mut self, page: Page, stack: StackShort) {
        let stack_hash = StackHash::new(&stack);

        let mut for_cache = false;
        // if `self.last_stack` contains state for some page
        // then `self.group` contains `usage` for the stack
        if let Some(state) = self.last_stack.get(&page) {
            log::debug!("double alloc {}", page);
            if state.stack_hash.eq(&stack_hash) {
                // double alloc in the same stack, do nothing
                return;
            } else {
                // double alloc in different stack, free in this stack and proceed
                for_cache = state.for_cache;
                let usage = self.group.get_mut(&state.stack_hash).unwrap();
                usage.decrease(&page);
                self.last_stack.remove(&page);
            }
        }

        // ensure `self.group` contains usage, and insert the state into `self.last_stack`
        if let Some(usage) = self.group.get_mut(&stack_hash) {
            assert!(self.collision_detector.contains(&stack));
            usage.increase(&page);
        } else {
            assert!(self.collision_detector.insert(stack.clone()));
            let mut usage = Usage::new(stack);
            usage.increase(&page);
            self.group.insert(stack_hash.clone(), usage);
        }
        self.last_stack.insert(page, PageState::new(stack_hash, for_cache));
    }

    pub fn remove(&mut self, page: &Page) {
        // if `self.last_stack` contains state for some page
        // then `self.group` contains `usage` for the stack
        if let Some(state) = self.last_stack.remove(page) {
            let usage = self.group.get_mut(&state.stack_hash).unwrap();
            if state.for_cache {
                usage.cache(page, false);
            }
            usage.decrease(page);
        } else {
            log::debug!("double free, or free without alloc {}", page);
        }
    }

    pub fn mark_cache(&mut self, page: Page, b: bool) {
        // if `self.last_stack` contains state for some page
        // then `self.group` contains `usage` for the stack
        if let Some(state) = self.last_stack.get_mut(&page) {
            if state.for_cache != b {
                let usage = self.group.get_mut(&state.stack_hash).unwrap();
                usage.cache(&page, b);
                state.for_cache = b;

                if !b {
                    usage.decrease(&page);
                    self.last_stack.remove(&page);
                }
            }
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &Usage> {
        self.group.iter().map(|(_, usage)| usage)
    }
}

#[derive(Default, Serialize)]
pub struct AllocationState {
    pid: Option<u32>,
    group: Group,
}

impl Tracker for AllocationState {
    fn track_alloc(&mut self, page: Page, stack: &Stack, _flags: Hex32, pid: u32) {
        self.pid = Some(pid);
        let stack = StackShort(stack.ips().to_vec());
        self.group.insert(page, stack);
    }

    fn track_free(&mut self, page: Page, pid: u32) {
        if self.pid != Some(pid) {
            return;
        }
        self.group.remove(&page);
    }

    fn mark_page_cache(&mut self, page: Page, b: bool) {
        self.group.mark_cache(page, b);
    }

    fn short_report(&self) -> (u64, u64) {
        let (mut node, mut cache) = (0, 0);
        for usage in self.group.iter() {
            node += (usage.node * 4) as u64;
            cache += (usage.cache * 4) as u64;
        }

        (node, cache)
    }

    fn tree_report<R>(
        &self,
        resolver: R,
        threshold: u64,
        reverse: bool,
    ) -> FrameReport<R>
    where
        R: Deref<Target = StackResolver>,
    {
        let mut report = FrameReport::new(resolver);
        for usage in self.group.iter() {
            let value = (usage.node as u64) * 4;
            let cache_value = (usage.cache as u64) * 4;

            if reverse {
                report.inner.insert(usage.stack.0.iter().rev(), value, cache_value);
            } else {
                report.inner.insert(usage.stack.0.iter(), value, cache_value);
            }
        }
        report.inner.strip(threshold);

        report
    }
}
