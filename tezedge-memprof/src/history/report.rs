use std::{collections::{HashMap, BTreeMap}, ops::Deref, cmp::Ordering};
use bpf_memprof_common::Hex64;
use serde::ser::{self, SerializeSeq};
use super::stack::{SymbolInfo, StackResolver};

#[derive(Default)]
pub struct FrameReportInner {
    value: u64,
    cache_value: u64,
    frames: HashMap<Hex64, FrameReportInner>,
    under_threshold: u64,
    cache_under_threshold: u64,
}

struct SortKey {
    inv_value: u64,
    name: SymbolInfo,
}

impl PartialEq for SortKey {
    fn eq(&self, other: &Self) -> bool {
        self.inv_value.eq(&other.inv_value) && self.name.eq(&other.name)
    }
}

impl Eq for SortKey {}

impl PartialOrd for SortKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SortKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.inv_value.cmp(&other.inv_value)
            .then(self.name.cmp(&other.name))
    }
}

pub struct FrameReportSorted {
    name: Option<SymbolInfo>,
    value: u64,
    cache_value: u64,
    frames: BTreeMap<SortKey, FrameReportSorted>,
    under_threshold: u64,
    cache_under_threshold: u64,
    unknown: u64,
    cache_unknown: u64,
}

impl FrameReportInner {
    pub fn insert<'a, StackIter>(&mut self, stack: StackIter, value: u64, cache_value: u64)
    where
        StackIter: Iterator<Item = &'a Hex64>,
    {
        let mut node = self;
        for stack_frame in stack {
            node.value += value;
            node.cache_value += cache_value;
            node = node.frames.entry(*stack_frame).or_default();
        }
        node.value += value;
        node.cache_value += cache_value;
    }

    pub fn strip(&mut self, threshold: u64) {
        let mut under_threshold = 0;
        let mut cache_under_threshold = 0;
        self.frames.retain(|_, frame| {
            frame.strip(threshold);
            let retain = frame.value >= threshold;
            if !retain {
                under_threshold += frame.value;
                cache_under_threshold += frame.cache_value;
            }
            retain
        });
        self.under_threshold = under_threshold;
        self.cache_under_threshold = cache_under_threshold;
    }

    pub fn sorted(&self, resolver: &StackResolver, name: Option<SymbolInfo>) -> FrameReportSorted {
        let mut frames = BTreeMap::new();
        let mut unknown = self.value - self.under_threshold;
        let mut cache_unknown = self.cache_value - self.cache_under_threshold;
        for (key, value) in &self.frames {
            if let Some(name) = resolver.resolve(key.0) {
                let key = SortKey {
                    inv_value: !value.value,
                    name: name.clone(),
                };
                frames.insert(key, value.sorted(resolver, Some(name)));
                unknown -= value.value;
                cache_unknown -= value.cache_value;
            }
        }

        FrameReportSorted {
            name,
            value: self.value,
            cache_value: self.cache_value,
            frames,
            under_threshold: self.under_threshold,
            cache_under_threshold: self.cache_under_threshold,
            unknown,
            cache_unknown,
        }
    }
}

pub struct FrameReport<R> {
    resolver: R,
    pub(crate) inner: FrameReportInner,
}

impl<R> FrameReport<R> {
    pub fn new(resolver: R) -> Self {
        FrameReport { resolver, inner: FrameReportInner::default() }
    }

    pub fn value(&self) -> u64 {
        self.inner.value
    }

    pub fn cache_value(&self) -> u64 {
        self.inner.cache_value
    }
}

impl ser::Serialize for FrameReportSorted {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        use self::ser::SerializeMap;

        struct Helper<'a> {
            inner: &'a BTreeMap<SortKey, FrameReportSorted>,
            under_threshold: Option<FakeFrame>,
            unknown: Option<FakeFrame>,        
        }

        #[derive(serde::Serialize)]
        #[serde(rename_all = "camelCase")]
        struct FakeFrame {
            name: String,
            value: u64,
            cache_value: u64,
        }

        impl FakeFrame {
            pub fn under_threshold(value: u64, cache_value: u64) -> Option<Self> {
                if value != 0 {
                    Some(FakeFrame {
                        name: "underThreshold".to_string(),
                        value,
                        cache_value,
                    })
                } else {
                    None
                }
            }

            pub fn unknown(value: u64, cache_value: u64) -> Option<Self> {
                if value != 0 {
                    Some(FakeFrame {
                        name: "unknown".to_string(),
                        value,
                        cache_value,
                    })
                } else {
                    None
                }
            }
        }

        impl<'a> ser::Serialize for Helper<'a> {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: ser::Serializer,
            {
                if self.inner.is_empty() {
                    return serializer.serialize_seq(Some(0))?.end();
                }
                let l = self.inner.len()
                    + (self.under_threshold.is_some() as usize)
                    + (self.unknown.is_some() as usize);
                let mut map = serializer.serialize_seq(Some(l))?;
                for (_, inner_frame) in self.inner {
                    map.serialize_element(inner_frame)?;
                }
                if let &Some(ref f) = &self.under_threshold {
                    map.serialize_element(f)?;
                }
                if let &Some(ref f) = &self.unknown {
                    map.serialize_element(f)?;
                }
                map.end()
            }
        }        

        let helper = Helper {
            inner: &self.frames,
            under_threshold: FakeFrame::under_threshold(self.under_threshold, self.cache_under_threshold),
            unknown: FakeFrame::unknown(self.unknown, self.cache_unknown),
        };

        let l = 3 + (self.name.is_some() as usize);
        let mut map = serializer.serialize_map(Some(l))?;
        if let &Some(ref name) = &self.name {
            map.serialize_entry("name", name)?;
        }
        map.serialize_entry("value", &self.value)?;
        map.serialize_entry("cacheValue", &self.cache_value)?;
        map.serialize_entry("frames", &helper)?;
        map.end()
    }
}

impl<R> ser::Serialize for FrameReport<R>
where
    R: Deref<Target = StackResolver>,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        let sorted = self.inner.sorted(&self.resolver, None);
        sorted.serialize(serializer)
    }
}
