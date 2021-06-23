use std::{collections::HashMap, ops::{AddAssign, Deref}};
use serde::Serialize;
use bpf_memprof::{Hex32, Stack};
use super::{
    page::Page,
    report::FrameReport,
    stack::StackResolver,
    history::StackShort,
};

#[derive(Default, Serialize)]
pub struct AllocationState {
    group: HashMap<StackShort, Usage>,
    last_stack: HashMap<Page, StackShort>,
}

#[derive(Default, Serialize)]
struct Usage {
    node: u32,
    cache: u32,
}

impl Usage {
    pub fn decrease(&mut self, page: &Page) {
        self.node = self.node.checked_sub(page.number()).unwrap_or(0);
    }

    pub fn increase(&mut self, page: &Page) {
        self.node += page.number();
    }

    pub fn cache(&mut self, page: &Page, b: bool) {
        if b {
            self.cache += page.number();
        } else {
            self.cache -= page.number();
        }
    }

    pub fn is_empty(&self) -> bool {
        self.node == 0 && self.cache == 0
    }

    pub fn short_report(&self) -> (u64, u64) {
        ((self.node as u64) * 4, (self.cache as u64) * 4)
    }
}

impl<'a> AddAssign<&'a Self> for Usage {
    fn add_assign(&mut self, rhs: &'a Self) {
        self.node += rhs.node;
        self.cache += rhs.cache;
    }
}

impl AllocationState {
    pub fn mark_page_cache(&mut self, page: Page, b: bool) {
        if let Some(stack) = self.last_stack.get(&page) {
            self.group.get_mut(stack).unwrap().cache(&page, b);
        }
    }

    pub fn track_alloc(&mut self, page: Page, stack: &Stack, _flags: Hex32) {
        let stack = StackShort(stack.ips().to_vec());

        // if we have a last_stack for some page then `self.group` contains entry for this stack
        // and the entry contains history for the page, so unwrap here is ok
        if let Some(last_stack) = self.last_stack.get(&page) {
            if last_stack.eq(&stack) {
                self.group.get_mut(last_stack).unwrap().increase(&page);
            } else {
                // fix it to track precise history, do not remove it in previous stack
                //self.group.get_mut(last_stack).unwrap().decrease(&page);
                self.group.entry(stack.clone()).or_default().increase(&page);
                self.last_stack.insert(page, stack);
            }
        } else {
            self.group.entry(stack.clone()).or_default().increase(&page);
            self.last_stack.insert(page, stack);
        }
    }

    pub fn track_free(&mut self, page: Page, pid: u32) {
        let _ = pid; // TODO:

        if let Some(stack) = self.last_stack.get(&page).cloned() {
            let usage = self.group.entry(stack.clone()).or_default();
            usage.decrease(&page);

            if usage.is_empty() {
                self.group.remove(&stack);
            }
        }
        // WARNING: might violate invariant if double free
        self.last_stack.remove(&page);
    }

    pub fn short_report(&self) -> (u64, u64) {
        let mut total = Usage::default();
        for (_, usage) in &self.group {
            total += usage;
        }

        total.short_report()
    }

    pub fn tree_report<R>(
        &self,
        resolver: R,
        threshold: u64,
        reverse: bool,
    ) -> FrameReport<R>
    where
        R: Deref<Target = StackResolver>,
    {
        let mut report = FrameReport::new(resolver);
        for (stack, usage) in &self.group {
            let (value, cache_value) = usage.short_report();
            
            if reverse {
                report.inner.insert(stack.0.iter().rev(), value, cache_value);
            } else {
                report.inner.insert(stack.0.iter(), value, cache_value);
            }
        }
        report.inner.strip(threshold);

        report
    }
}
