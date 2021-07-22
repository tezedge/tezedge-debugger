use std::collections::HashMap;
use bpf_memprof_common::{Stack, Hex64};
use super::func_path::FuncPath;

#[derive(Default)]
pub struct VmAggregator {
    counter: u32,
    paths: HashMap<FuncPath, u32>,
    //pages: HashMap<PageAddress, PageInfo>,
    //groups: HashMap<FuncPathIndex, Usage>,
}

impl VmAggregator {
    pub fn track_alloc(&mut self, stack: &Stack, ptr: u64, len: u64) {
        let &mut VmAggregator { ref mut counter, .. } = self;
        let path = FuncPath::new(stack);
        let index = *self.paths
            .entry(path.clone())
            .or_insert_with(|| {
                let index = *counter;
                *counter += 1;
                index
            });

        // TODO:
        let _ = (index, ptr, len);
    }

    pub fn report(&self) -> impl Iterator<Item = (u64, u64, &[Hex64])> {
        // TODO:
        std::iter::empty()
    }
}
