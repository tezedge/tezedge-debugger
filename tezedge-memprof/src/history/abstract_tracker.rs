use std::ops::Deref;
use bpf_memprof::{Hex32, Stack};
use super::{page::Page, report::FrameReport, stack::StackResolver};

pub trait Tracker {
    fn track_alloc(&mut self, page: Page, stack: &Stack, flags: Hex32, pid: u32);
    fn track_free(&mut self, page: Page, pid: u32);
    fn mark_page_cache(&mut self, page: Page, b: bool);
}

pub trait Reporter {
    fn short_report(&self) -> (u64, u64);

    fn tree_report<R>(
        &self,
        resolver: R,
        threshold: u64,
        reverse: bool,
    ) -> FrameReport<R>
    where
        R: Deref<Target = StackResolver>;
}
