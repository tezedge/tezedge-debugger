use std::collections::HashMap;
use bpf_memprof_common::{Stack, Hex64};
use super::func_path::FuncPath;

#[derive(Clone, Hash, PartialEq, Eq)]
struct FuncPathIndex(u32);

#[derive(Hash, PartialEq, Eq)]
struct AreaAddress(u64);

struct AreaInfo {
    func_path_index: FuncPathIndex,
    is_allocated: bool,
    length: u64,
}

struct Usage {
    func_path: FuncPath,
    value: u64,
}

#[derive(Default)]
pub struct VmAggregator {
    counter: u32,
    paths: HashMap<FuncPath, FuncPathIndex>,
    areas: HashMap<AreaAddress, AreaInfo>,
    groups: HashMap<FuncPathIndex, Usage>,
}

impl VmAggregator {
    pub fn track_alloc(&mut self, stack: &Stack, ptr: u64, length: u64) {
        if ptr == 0 || length == 0 {
            return;
        }

        let &mut VmAggregator { ref mut counter, .. } = self;
        let path = FuncPath::new(stack);
        let index = self.paths
            .entry(path.clone())
            .or_insert_with(|| {
                let index = FuncPathIndex(*counter);
                *counter += 1;
                index
            })
            .clone();

        let address = AreaAddress(ptr);
        let info = self.areas
            .entry(address)
            .or_insert_with(|| AreaInfo {
                func_path_index: index.clone(),
                is_allocated: false,
                length,
            });

        if index.ne(&info.func_path_index) {
            log::error!("double alloc: {:016x} {}", ptr, length);
        }

        let usage = self.groups
            .entry(index.clone())
            .or_insert_with(|| Usage {
                func_path: path.clone(),
                value: 0,
            });

        if info.is_allocated {
            log::warn!("seems events in wrong order, double alloc: {:016x} {}", ptr, length);
        } else {
            info.is_allocated = true;
            usage.value += length;
        }

        if usage.func_path != path {
            log::warn!("inconsistent path for area: {:016x}", ptr);
        }
    }

    pub fn track_free(&mut self, ptr: u64, length: u64) {
        if ptr == 0 || length == 0 {
            return;
        }

        let address = AreaAddress(ptr);
        if let Some(info) = self.areas.remove(&address) {
            let usage = self.groups.get_mut(&info.func_path_index).unwrap();
            if info.length != length {
                log::warn!(
                    "free different length, area: {:016x}, length: alloc {} free {}",
                    ptr,
                    info.length,
                    length,
                );
            }
            if info.is_allocated {
                if usage.value < info.length {
                    log::warn!("alloc underflow, area: {:016x} {}", ptr, length);
                }
                usage.value -= info.length;
            } else {
                log::warn!("double free, area: {:016x} {}", ptr, length);
            }
        }
    }

    pub fn report(&self) -> impl Iterator<Item = (u64, u64, &[Hex64])> {
        self.groups.iter().map(|(_, usage)| (
            usage.value / 1024,
            0,
            usage.func_path.as_ref(),
        ))
    }
}
