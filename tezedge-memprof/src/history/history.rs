use std::{collections::HashMap, ops::Deref};
use serde::{Serialize, ser};
use bpf_memprof::{Hex32, Hex64, Stack};
use super::{
    page::Page,
    error::ErrorReport,
    allocation::{PageHistory, AllocError, FreeError},
    report::FrameReport,
    stack::StackResolver,
};

#[derive(Clone, Hash, PartialEq, Eq)]
pub struct StackShort(Vec<Hex64>);

impl StackShort {
    pub fn unknown() -> Self {
        StackShort(vec![Hex64(0)])
    }
}

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

        // if we have a last_stack for some page then `self.group` contains entry for this stack
        // and the entry contains history for the page, so unwrap here is ok
        if let Some(last_stack) = self.last_stack.get(&page) {
            if last_stack.eq(&stack) {
                let history = self.group.get_mut(last_stack).unwrap().get_mut(&page).unwrap();
                Self::track_alloc_error(&mut self.error_report, history, &page, flags);
            } else {
                // fix it to track precise history, do not remove it in previous stack
                let mut history = self.group.get_mut(last_stack).unwrap().remove(&page).unwrap();
                Self::track_alloc_error(&mut self.error_report, &mut history, &page, flags);
                self.group.entry(stack.clone()).or_default().insert(page.clone(), history);
                self.last_stack.insert(page, stack);
            }
        } else {
            let group = self.group.entry(stack.clone()).or_default();
            let history = group.entry(page.clone()).or_default();
            Self::track_alloc_error(&mut self.error_report, history, &page, flags);
            self.last_stack.insert(page, stack);
        }
    }

    pub fn track_free(&mut self, page: Page) {
        let stack = self.last_stack.get(&page).cloned().unwrap_or(StackShort::unknown());
        let history = self.group.entry(stack).or_default().entry(page.clone()).or_default();
        Self::track_free_error(&mut self.error_report, history, &page);
    }

    fn track_alloc_error(error_report: &mut ErrorReport, history: &mut H, page: &Page, flags: Hex32) {
        if let Err(AllocError) = history.track_alloc(flags) {
            error_report.double_alloc(page);
        }
    }

    fn track_free_error(error_report: &mut ErrorReport, history: &mut H, page: &Page) {
        match history.track_free() {
            Ok(()) => (),
            Err(FreeError::DoubleFree) => error_report.double_free(&page),
            Err(FreeError::WithoutAlloc) => error_report.without_alloc(&page),
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
        reverse: bool,
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
            if reverse {
                report.inner.insert(stack.0.iter().rev(), value);
            } else {
                report.inner.insert(stack.0.iter(), value);
            }
        }
        report.inner.strip(threshold);

        report
    }
}
