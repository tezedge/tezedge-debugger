use std::fmt;
use bpf_memprof::Hex64;
use serde::{Serialize, ser};

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub struct Page {
    pfn: Hex64,
    order: u32,
}

impl fmt::Display for Page {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}-{}", self.pfn, self.order)
    }
}

impl Serialize for Page {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl Page {
    pub fn new(pfn: Hex64, order: u32) -> Self {
        Page { pfn, order }
    }

    pub fn size_kib(&self) -> u64 {
        1u64 << (self.order + 2)
    }
}
