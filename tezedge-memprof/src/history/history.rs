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

#[derive(Hash, PartialEq, Eq)]
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
    histories: HashMap<Page, H>,
    group: HashMap<StackShort, Vec<Page>>,
}

impl<H> History<H>
where
    H: PageHistory + Default,
{
    pub fn track_alloc(&mut self, page: Page, stack: &Stack, flags: Hex32) {
        let entry = self.histories.entry(page.clone()).or_default();
        if let Err(AllocError) = entry.track_alloc(flags) {
            self.error_report.double_alloc(&page);
        }
        let pages_group = self.group.entry(StackShort(stack.ips().to_vec())).or_default();
        pages_group.push(page);
    }

    pub fn track_free(&mut self, page: Page) {
        let entry = self.histories.entry(page.clone()).or_default();
        match entry.track_free() {
            Ok(()) => (),
            Err(FreeError::DoubleFree) => self.error_report.double_free(&page),
            Err(FreeError::WithoutAlloc) => self.error_report.without_alloc(&page),
        }
    }

    pub fn short_report(&self) -> u64 {
        let mut value_kib = 0;
        for (page, history) in &self.histories {
            if history.is_allocated(None) {
                value_kib += page.size_kib();
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
            for page in group {
                let history = self.histories.get(page).unwrap();
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
