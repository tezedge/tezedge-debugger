use std::{sync::Arc, collections::HashMap, ops::Deref};
use bpf_memprof::{Hex64, Stack};
use super::{Reporter, StackResolver, FrameReport};

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub struct FuncPath(Arc<Vec<Hex64>>);

impl FuncPath {
    pub fn new(stack: &Stack) -> Self {
        FuncPath(Arc::new(stack.ips().to_vec()))
    }
}

#[derive(Clone, Hash, PartialEq, Eq)]
pub struct FuncPathIndex(u32);

#[derive(Clone, Hash, PartialEq, Eq)]
pub struct PageAddress(u32);

pub struct PageInfo {
    func_path_index: FuncPathIndex,
    order: u8,
    is_allocated: bool,
    is_cache: bool,
}

pub struct Usage {
    func_path: FuncPath,
    value: u32,
    cache_value: u32,
}

#[derive(Default)]
pub struct Collector {
    counter: u32,
    paths: HashMap<FuncPath, FuncPathIndex>,
    pages: HashMap<PageAddress, PageInfo>,
    groups: HashMap<FuncPathIndex, Usage>,
}

impl Collector {
    pub fn track_alloc(&mut self, page: u32, order: u8, stack: &Stack) {
        let &mut Collector { ref mut counter, .. } = self;
        let path = FuncPath::new(stack);
        let index = self.paths
            .entry(path.clone())
            .or_insert_with(|| {
                let index = FuncPathIndex(*counter);
                *counter += 1;
                index
            }) as &FuncPathIndex;

        let address = PageAddress(page);
        let info = self.pages
            .entry(address)
            .or_insert_with(|| PageInfo {
                func_path_index: index.clone(),
                order,
                is_allocated: false,
                is_cache: false,
            });
        let pages_count = 1 << info.order;

        let old_index = &info.func_path_index;
        if *old_index != *index {
            let usage = self.groups.get_mut(old_index).unwrap();
            if info.is_allocated {
                if usage.value < pages_count {
                    log::warn!("alloc underflow, page: {:08x}-{}", page, info.order);
                }
                usage.value -= pages_count;
            } else {
                if info.is_cache {
                    log::warn!("not alloc, but cache, page: {:08x}-{}", page, info.order);
                }
            }
            if info.is_cache {
                if usage.cache_value < pages_count {
                    log::warn!("cache underflow, page: {:08x}-{}", page, info.order);
                }
                usage.cache_value -= pages_count;
            }
            info.func_path_index = index.clone();
        }

        info.is_allocated = true;
        let usage = self.groups
            .entry(index.clone())
            .or_insert_with(|| Usage {
                func_path: path.clone(),
                value: 0,
                cache_value: 0,
            });
        usage.value += pages_count;

        if usage.func_path != path {
            log::warn!("inconsistent path for page: {:08x}-{}", page, info.order);
        }
    }

    pub fn track_free(&mut self, page: u32) {
        let address = PageAddress(page);
        if let Some(info) = self.pages.remove(&address) {
            let pages_count = 1 << info.order;
            let usage = self.groups.get_mut(&info.func_path_index).unwrap();
            if info.is_allocated {
                if usage.value < pages_count {
                    log::warn!("alloc underflow, page: {:08x}-{}", page, info.order);
                }
                usage.value -= pages_count;
            } else {
                if info.is_cache {
                    log::warn!("not alloc, but cache, page: {:08x}-{}", page, info.order);
                }
            }
            if info.is_cache {
                if usage.cache_value < pages_count {
                    log::warn!("cache underflow, page: {:08x}-{}", page, info.order);
                }
                usage.cache_value -= pages_count;
            }
        }
    }

    pub fn mark_cache(&mut self, page: u32, b: bool) {
        let address = PageAddress(page);
        if let Some(info) = self.pages.get_mut(&address) {
            let pages_count = 1 << info.order;
            let usage = self.groups.get_mut(&info.func_path_index).unwrap();

            if info.is_allocated {
                log::warn!("not alloc, but cache {}, page: {:08x}-{}", b, page, info.order);
            }
            if info.is_cache != b {
                info.is_cache = b;
                if b {
                    usage.cache_value += pages_count;
                } else {
                    if usage.cache_value < pages_count {
                        log::warn!("cache underflow, page: {:08x}-{}", page, info.order);
                    }
                    usage.cache_value -= pages_count;
                }
            }
        }
    }

    pub fn report(&self) -> impl Iterator<Item = (u64, u64, &[Hex64])> {
        self.groups.iter().map(|(_, usage)| (
            (usage.value as u64) * 4,
            (usage.cache_value as u64) * 4,
            usage.func_path.0.as_ref().as_ref(),
        ))
    }
}

impl Reporter for Collector {
    fn short_report(&self) -> (u64, u64) {
        let (mut value, mut cache_value) = (0, 0);
        for (v, c, _) in self.report() {
            value += v;
            cache_value += c;
        }

        (value, cache_value)
    }

    fn tree_report<R>(&self, resolver: R, threshold: u64, reverse: bool) -> FrameReport<R>
    where
        R: Deref<Target = StackResolver>,
    {
        let mut report = FrameReport::new(resolver);
        for (value, cache_value, stack) in self.report() {
            if reverse {
                report.inner.insert(stack.iter().rev(), value, cache_value);
            } else {
                report.inner.insert(stack.iter(), value, cache_value);
            }
        }
        report.inner.strip(threshold);

        report

    }
}
