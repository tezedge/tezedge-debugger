use std::{collections::HashMap, ops::Deref};
use serde::{Serialize, ser};
use bpf_memprof::{Hex32, Hex64, Stack};
use super::{
    page::Page,
    error::ErrorReport,
    allocation::{PageHistory, AllocError, FreeError},
    report::FrameReport,
    StackResolver,
};

#[derive(Clone, Hash, PartialEq, Eq)]
pub struct StackShort(Vec<Hex64>);

impl Serialize for StackShort {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        let stack = self.0.iter().fold(String::new(), |s, f| s + &format!("{:?}/", f));
        serializer.serialize_str(&stack)
    }
}

#[derive(Default, Serialize)]
pub struct History<H> {
    error_report: ErrorReport,
    group: HashMap<StackShort, HashMap<Page, H>>,
    last_stack: HashMap<Page, StackShort>,
}

impl<H> History<H>
where
    H: PageHistory + Default,
{
    pub fn track_alloc(&mut self, page: Page, stack: &Stack, flags: Hex32) {
        let stack = StackShort(stack.ips().to_vec());

        if let Some(last_stack) = self.last_stack.get(&page) {
            if last_stack.eq(&stack) {
                if let Err(AllocError) = self.group.get_mut(&stack).unwrap().get_mut(&page).unwrap().track_alloc(flags) {
                    self.error_report.double_alloc(&page);
                }
            } else {
                let mut history = self.group.get_mut(&last_stack).unwrap().remove(&page).unwrap();
                if let Err(AllocError) = history.track_alloc(flags) {
                    self.error_report.double_alloc(&page);
                }
                self.group.entry(stack.clone()).or_default().insert(page.clone(), history);
                self.last_stack.insert(page, stack);
            }
        } else {
            let history = self.group.entry(stack.clone()).or_default().entry(page.clone()).or_default();
            if let Err(AllocError) = history.track_alloc(flags) {
                self.error_report.double_alloc(&page);
            }
            self.last_stack.insert(page, stack);
        }
    }

    pub fn track_free(&mut self, page: Page) {
        if let Some(stack) = self.last_stack.get(&page) {
            let history = self.group.entry(stack.clone()).or_default().entry(page.clone()).or_default();
            match history.track_free() {
                Ok(()) => (),
                Err(FreeError::DoubleFree) => self.error_report.double_free(&page),
                Err(FreeError::WithoutAlloc) => self.error_report.without_alloc(&page),
            }
        }
    }

    pub fn short_report(&self) -> u64 {
        let mut value_kib = 0;
        for (_, group) in &self.group {
            for (page, history) in group {
                if history.is_allocated(None) {
                    value_kib += page.size_kib();
                }
            }
        }

        value_kib
    }

    pub fn tree_report<R>(
        &self,
        resolver: R,
        threshold: u64,
    ) -> FrameReport<R>
    where
        R: Deref<Target = StackResolver>,
    {
        let mut report = FrameReport::new(resolver);
        for (stack, group) in &self.group {
            let mut value = 0;
            for (page, history) in group {
                if history.is_allocated(None) {
                    value += page.size_kib();
                }
            }
            report.inner.insert(&stack.0, value);
        }
        report.inner.strip(threshold);

        report
    }
}
