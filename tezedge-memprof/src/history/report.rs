use std::{collections::{HashMap, BTreeMap}, ops::Deref};
use bpf_memprof::Hex64;
use serde::ser;
use super::StackResolver;

#[derive(Default)]
pub struct FrameReportInner {
    value: u64,
    frames: HashMap<Hex64, FrameReportInner>,
}

pub struct FrameReportSorted {
    name: String,
    value: u64,
    frames: BTreeMap<u64, FrameReportSorted>,
}

impl FrameReportInner {
    pub fn insert<'a, StackIter>(&mut self, stack: StackIter, value: u64)
    where
        StackIter: Iterator<Item = &'a Hex64>,
    {
        let mut node = self;
        for stack_frame in stack {
            node.value += value;
            node = node.frames.entry(*stack_frame).or_default();
        }
        node.value += value;
    }

    pub fn strip(&mut self, threshold: u64) {
        self.frames.retain(|_, frame| {
            frame.strip(threshold);
            frame.value >= threshold
        })
    }

    pub fn sorted(&self, resolver: &StackResolver, name: String) -> FrameReportSorted {
        let mut frames = BTreeMap::new();
        for (key, value) in &self.frames {
            let name = resolver.resolve(key.0);
            frames.insert(!value.value, value.sorted(resolver, name));
        }

        FrameReportSorted { name, value: self.value, frames }
    }
}

pub struct FrameReport<R> {
    resolver: R,
    pub(super) inner: FrameReportInner,
}

impl<R> FrameReport<R> {
    pub fn new(resolver: R) -> Self {
        FrameReport { resolver, inner: FrameReportInner::default() }
    }
}

impl ser::Serialize for FrameReportSorted {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        use self::ser::SerializeMap;

        struct Helper<'a>(&'a BTreeMap<u64, FrameReportSorted>);

        impl<'a> ser::Serialize for Helper<'a> {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: ser::Serializer,
            {
                let mut map = serializer.serialize_map(Some(self.0.len()))?;
                for (_, inner_frame) in self.0 {
                    map.serialize_entry(&inner_frame.name, inner_frame)?
                }
                map.end()
            }
        }        

        let mut map = serializer.serialize_map(Some(2))?;
        map.serialize_entry("value", &self.value)?;
        map.serialize_entry("frames", &Helper(&self.frames))?;
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
        let sorted = self.inner.sorted(&self.resolver, String::new());
        sorted.serialize(serializer)
    }
}
