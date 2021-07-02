use std::{sync::Arc, collections::HashMap};
use serde::{Serialize, Deserialize};
use bpf_memprof_common::{Hex64, Stack};

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

#[derive(Serialize, Deserialize, Debug)]
pub enum RawEvent {
    Alloc {
        page: u32,
        order: u8,
    },
    Free {
        page: u32,
    },
    Cache {
        page: u32,
    },
    UnCache {
        page: u32,
    },
    RssAnon(u32),
}

#[derive(Default)]
pub struct Aggregator {
    counter: u32,
    paths: HashMap<FuncPath, FuncPathIndex>,
    pages: HashMap<PageAddress, PageInfo>,
    groups: HashMap<FuncPathIndex, Usage>,
    dump: Option<Vec<RawEvent>>,
}

impl Aggregator {
    pub fn turn_on_dump(&mut self) {
        self.dump = Some(Vec::new());
    }

    pub fn store_dump(&mut self) {
        if let Some(dump) = &self.dump {
            log::info!("writing dump...");
            bincode::serialize_into(std::fs::File::create("target/dump").unwrap(), dump).unwrap();
            log::info!("done dump");
        }
    }

    pub fn track_alloc(&mut self, page: u32, order: u8, stack: &Stack) {
        if let Some(dump) = &mut self.dump {
            dump.push(RawEvent::Alloc { page, order });
        }
        let &mut Aggregator { ref mut counter, .. } = self;
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
        if let Some(dump) = &mut self.dump {
            dump.push(RawEvent::Free { page });
        }

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
        if let Some(dump) = &mut self.dump {
            if b {
                dump.push(RawEvent::Cache { page });
            } else {
                dump.push(RawEvent::UnCache { page });
            }
        }

        let address = PageAddress(page);
        if let Some(info) = self.pages.get_mut(&address) {
            let pages_count = 1 << info.order;
            let usage = self.groups.get_mut(&info.func_path_index).unwrap();

            if !info.is_allocated {
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

    pub fn track_rss_anon(&mut self, value: u32) {
        if let Some(dump) = &mut self.dump {
            dump.push(RawEvent::RssAnon(value));
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
