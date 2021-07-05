use std::{collections::HashMap, ops::Deref, sync::Arc};
use serde::{Serialize, ser};
use bpf_memprof_common::{Hex32, Hex64, Stack};
use super::{
    page::Page,
    error::ErrorReport,
    page_history::{PageHistory, AllocError, FreeError},
    report::FrameReport,
    stack::StackResolver,
    abstract_tracker::{Tracker, Reporter},
};

#[derive(Clone, Hash, PartialEq, Eq)]
pub struct StackShort(pub Arc<Vec<Hex64>>);

impl StackShort {
    pub fn new(stack: &Stack) -> Self {
        StackShort(Arc::new(stack.ips().to_vec()))
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

impl<H> Tracker for History<H>
where
    H: PageHistory + Default,
{
    fn track_alloc(&mut self, page: Page, stack: &Stack, flags: Hex32, pid: u32) {
        let _ = pid;
        let stack = StackShort::new(stack);

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

    fn track_free(&mut self, page: Page, pid: u32) {
        let _ = pid; // TODO:
        if let Some(stack) = self.last_stack.get(&page).cloned() {
            let history = self.group.entry(stack.clone()).or_default().entry(page.clone()).or_default();
            Self::track_free_error(&mut self.error_report, history, &page);

            if history.is_empty() {
                let group = self.group.get_mut(&stack).unwrap();
                group.remove(&page);
                if group.is_empty() {
                    self.group.remove(&stack);
                }
                self.last_stack.remove(&page);
            }
        } else {
            // self.error_report.without_alloc(&page);
        }
    }

    fn mark_page_cache(&mut self, page: Page, b: bool) {
        if let Some(stack) = self.last_stack.get(&page) {
            self.group.get_mut(stack).unwrap().get_mut(&page).unwrap().mark_page_cache(b);
        }
    }
}

impl<H> Reporter for History<H>
where
    H: PageHistory,
{
    fn short_report(&self) -> (u64, u64) {
        let mut value_kib = 0;
        let mut cache_value_kib = 0;
        for (_, group) in &self.group {
            for (page, history) in group {
                if history.is_allocated(None) {
                    value_kib += page.size_kib();
                    if history.page_cache() {
                        cache_value_kib += page.size_kib();
                    }
                }
            }
        }

        (value_kib, cache_value_kib)
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
        for (stack, group) in &self.group {
            let mut value = 0;
            let mut cache_value = 0;
            for (page, history) in group {
                if history.is_allocated(None) {
                    value += page.size_kib();
                    if history.page_cache() {
                        cache_value += page.size_kib();
                    }
                }
            }
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

impl<H> History<H>
where
    H: PageHistory + Default,
{
    fn track_alloc_error(error_report: &mut ErrorReport, history: &mut H, page: &Page, flags: Hex32) {
        if let Err(AllocError) = history.track_alloc(flags) {
            error_report.double_alloc(page);
        }
    }

    #[allow(dead_code)]
    fn track_free_error(error_report: &mut ErrorReport, history: &mut H, page: &Page) {
        match history.track_free() {
            Ok(()) => (),
            Err(FreeError::DoubleFree) => error_report.double_free(&page),
            Err(FreeError::WithoutAlloc) => {
                error_report.without_alloc(&page);
                debug_assert!(false);
            },
        }
    }

    #[cfg(test)]
    fn is_empty(&self) -> bool {
        self.last_stack.is_empty() && self.group.is_empty()
    }
}

#[cfg(test)]
mod test {
    use bpf_memprof_common::{Hex64, Hex32, Stack};
    use crate::{History, EventLast, Page, Tracker, Reporter};

    #[test]
    fn overflow() {
        let mut h = History::<EventLast>::default();
        for _ in 0..0x100 {
            for i in 1..100 {
                h.track_alloc(Page::new(Hex64(i), 0), &Stack::from_frames(&[i / 3]), Hex32(0), 0);
            }
            for i in 1..100 {
                h.track_free(Page::new(Hex64(i), 0), 0);
            }
        }

        assert_eq!(h.short_report(), (0, 0));
        assert!(h.is_empty());
    }
}
