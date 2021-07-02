use std::{fmt, hash::{Hash, Hasher}};
use bpf_memprof_common::Hex64;
use serde::{Serialize, ser};

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Page {
    // last 4 bits is order, 0..28 bits are pfn
    inner: u32,
}

impl Hash for Page {
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        (self.inner & 0x0fffffff).hash::<H>(state)
    }
}

impl fmt::Display for Page {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let pfn = Hex64((self.inner & 0x0fffffff) as u64);
        let order = self.inner >> 28;
        write!(f, "{:?}-{}", pfn, order)
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
        let inner = ((pfn.0 & 0x0fffffff) as u32) + (order << 28);
        Page { inner }
    }

    pub fn size_kib(&self) -> u64 {
        4u64 << (self.inner >> 28)
    }

    pub fn number(&self) -> u32 {
        1 << (self.inner >> 28)
    }

    pub fn order(&self) -> u8 {
        (self.inner >> 28) as u8
    }

    pub fn set_order(&mut self, order: u8) {
        self.inner = (self.inner & 0x0fffffff) + ((order as u32) << 28)
    }
}
