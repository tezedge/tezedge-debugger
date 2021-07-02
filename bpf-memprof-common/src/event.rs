// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use core::{convert::TryFrom, fmt};

#[cfg(feature = "client")]
use serde::{Serialize, Deserialize};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Hex64(pub u64);

impl fmt::Debug for Hex64 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016x}", &self.0)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Hex32(pub u32);

impl fmt::Debug for Hex32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:08x}", &self.0)
    }
}

pub trait Pod
where
    Self: Sized,
{
    const DISCRIMINANT: Option<u32>;
    const SIZE: usize;

    fn from_slice(s: &[u8]) -> Option<Self>;
}

#[cfg_attr(feature = "client", derive(Serialize, Deserialize))]
#[cfg_attr(not(feature = "client"), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonHeader {
    ty: u16,
    flags: u8,
    preempt_count: u8,
    pid: u32,
}

impl Pod for CommonHeader {
    const DISCRIMINANT: Option<u32> = None;
    const SIZE: usize = 0x08;

    #[inline(always)]
    fn from_slice(s: &[u8]) -> Option<Self> {
        if s.len() < Self::SIZE {
            return None;
        }
        Some(CommonHeader {
            ty: u16::from_ne_bytes(TryFrom::try_from(&s[0x00..0x02]).unwrap()),
            flags: s[0x02],
            preempt_count: s[0x03],
            pid: u32::from_ne_bytes(TryFrom::try_from(&s[0x04..0x08]).unwrap()),
        })
    }
}

#[cfg_attr(feature = "client", derive(Serialize, Deserialize))]
#[cfg_attr(not(feature = "client"), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KFree {
    call_site: Hex64,
    pub ptr: Hex64,
}

impl Pod for KFree {
    const DISCRIMINANT: Option<u32> = Some(1);
    const SIZE: usize = 0x10;

    #[inline(always)]
    fn from_slice(s: &[u8]) -> Option<Self> {
        if s.len() < Self::SIZE {
            return None;
        }
        Some(KFree {
            call_site: Hex64(u64::from_ne_bytes(TryFrom::try_from(&s[0x00..0x08]).unwrap())),
            ptr: Hex64(u64::from_ne_bytes(TryFrom::try_from(&s[0x08..0x10]).unwrap())),
        })
    }
}

#[cfg_attr(feature = "client", derive(Serialize, Deserialize))]
#[cfg_attr(not(feature = "client"), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KMAlloc {
    call_site: Hex64,
    pub ptr: Hex64,
    bytes_req: Hex64,
    pub bytes_alloc: Hex64,
    gfp_flags: Hex32,
}

impl Pod for KMAlloc {
    const DISCRIMINANT: Option<u32> = Some(2);
    const SIZE: usize = 0x24;

    #[inline(always)]
    fn from_slice(s: &[u8]) -> Option<Self> {
        if s.len() < Self::SIZE {
            return None;
        }
        Some(KMAlloc {
            call_site: Hex64(u64::from_ne_bytes(TryFrom::try_from(&s[0x00..0x08]).unwrap())),
            ptr: Hex64(u64::from_ne_bytes(TryFrom::try_from(&s[0x08..0x10]).unwrap())),
            bytes_req: Hex64(u64::from_ne_bytes(TryFrom::try_from(&s[0x10..0x18]).unwrap())),
            bytes_alloc: Hex64(u64::from_ne_bytes(TryFrom::try_from(&s[0x18..0x20]).unwrap())),
            gfp_flags: Hex32(u32::from_ne_bytes(TryFrom::try_from(&s[0x20..0x24]).unwrap())),
        })
    }
}

#[cfg_attr(feature = "client", derive(Serialize, Deserialize))]
#[cfg_attr(not(feature = "client"), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KMAllocNode {
    call_site: Hex64,
    pub ptr: Hex64,
    bytes_req: Hex64,
    pub bytes_alloc: Hex64,
    gfp_flags: Hex32,
    node: Hex32,
}

impl Pod for KMAllocNode {
    const DISCRIMINANT: Option<u32> = Some(3);
    const SIZE: usize = 0x28;

    #[inline(always)]
    fn from_slice(s: &[u8]) -> Option<Self> {
        if s.len() < Self::SIZE {
            return None;
        }
        Some(KMAllocNode {
            call_site: Hex64(u64::from_ne_bytes(TryFrom::try_from(&s[0x00..0x08]).unwrap())),
            ptr: Hex64(u64::from_ne_bytes(TryFrom::try_from(&s[0x08..0x10]).unwrap())),
            bytes_req: Hex64(u64::from_ne_bytes(TryFrom::try_from(&s[0x10..0x18]).unwrap())),
            bytes_alloc: Hex64(u64::from_ne_bytes(TryFrom::try_from(&s[0x18..0x20]).unwrap())),
            gfp_flags: Hex32(u32::from_ne_bytes(TryFrom::try_from(&s[0x20..0x24]).unwrap())),
            node: Hex32(u32::from_ne_bytes(TryFrom::try_from(&s[0x24..0x28]).unwrap())),
        })
    }
}

#[cfg_attr(feature = "client", derive(Serialize, Deserialize))]
#[cfg_attr(not(feature = "client"), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheAlloc {
    call_site: Hex64,
    pub ptr: Hex64,
    bytes_req: Hex64,
    pub bytes_alloc: Hex64,
    gfp_flags: Hex32,
}

impl Pod for CacheAlloc {
    const DISCRIMINANT: Option<u32> = Some(4);
    const SIZE: usize = 0x24;

    #[inline(always)]
    fn from_slice(s: &[u8]) -> Option<Self> {
        if s.len() < Self::SIZE {
            return None;
        }
        Some(CacheAlloc {
            call_site: Hex64(u64::from_ne_bytes(TryFrom::try_from(&s[0x00..0x08]).unwrap())),
            ptr: Hex64(u64::from_ne_bytes(TryFrom::try_from(&s[0x08..0x10]).unwrap())),
            bytes_req: Hex64(u64::from_ne_bytes(TryFrom::try_from(&s[0x10..0x18]).unwrap())),
            bytes_alloc: Hex64(u64::from_ne_bytes(TryFrom::try_from(&s[0x18..0x20]).unwrap())),
            gfp_flags: Hex32(u32::from_ne_bytes(TryFrom::try_from(&s[0x20..0x24]).unwrap())),
        })
    }
}

#[cfg_attr(feature = "client", derive(Serialize, Deserialize))]
#[cfg_attr(not(feature = "client"), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheAllocNode {
    call_site: Hex64,
    pub ptr: Hex64,
    bytes_req: Hex64,
    pub bytes_alloc: Hex64,
    gfp_flags: Hex32,
    node: Hex32,
}

impl Pod for CacheAllocNode {
    const DISCRIMINANT: Option<u32> = Some(5);
    const SIZE: usize = 0x28;

    #[inline(always)]
    fn from_slice(s: &[u8]) -> Option<Self> {
        if s.len() < Self::SIZE {
            return None;
        }
        Some(CacheAllocNode {
            call_site: Hex64(u64::from_ne_bytes(TryFrom::try_from(&s[0x00..0x08]).unwrap())),
            ptr: Hex64(u64::from_ne_bytes(TryFrom::try_from(&s[0x08..0x10]).unwrap())),
            bytes_req: Hex64(u64::from_ne_bytes(TryFrom::try_from(&s[0x10..0x18]).unwrap())),
            bytes_alloc: Hex64(u64::from_ne_bytes(TryFrom::try_from(&s[0x18..0x20]).unwrap())),
            gfp_flags: Hex32(u32::from_ne_bytes(TryFrom::try_from(&s[0x20..0x24]).unwrap())),
            node: Hex32(u32::from_ne_bytes(TryFrom::try_from(&s[0x24..0x28]).unwrap())),
        })
    }
}

#[cfg_attr(feature = "client", derive(Serialize, Deserialize))]
#[cfg_attr(not(feature = "client"), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheFree {
    call_site: Hex64,
    pub ptr: Hex64,
}

impl Pod for CacheFree {
    const DISCRIMINANT: Option<u32> = Some(6);
    const SIZE: usize = 0x10;

    #[inline(always)]
    fn from_slice(s: &[u8]) -> Option<Self> {
        if s.len() < Self::SIZE {
            return None;
        }
        Some(CacheFree {
            call_site: Hex64(u64::from_ne_bytes(TryFrom::try_from(&s[0x00..0x08]).unwrap())),
            ptr: Hex64(u64::from_ne_bytes(TryFrom::try_from(&s[0x08..0x10]).unwrap())),
        })
    }
}

#[cfg_attr(feature = "client", derive(Serialize, Deserialize))]
#[cfg_attr(not(feature = "client"), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageAlloc {
    pub pfn: Hex64,
    pub order: u32,
    pub gfp_flags: Hex32,
    migrate_ty: i32,
}

impl Pod for PageAlloc {
    const DISCRIMINANT: Option<u32> = Some(7);
    const SIZE: usize = 0x14;

    #[inline(always)]
    fn from_slice(s: &[u8]) -> Option<Self> {
        if s.len() < Self::SIZE {
            return None;
        }
        Some(PageAlloc {
            pfn: Hex64(u64::from_ne_bytes(TryFrom::try_from(&s[0x00..0x08]).unwrap())),
            order: u32::from_ne_bytes(TryFrom::try_from(&s[0x08..0x0c]).unwrap()),
            gfp_flags: Hex32(u32::from_ne_bytes(TryFrom::try_from(&s[0x0c..0x10]).unwrap())),
            migrate_ty: i32::from_ne_bytes(TryFrom::try_from(&s[0x10..0x14]).unwrap()),
        })
    }
}

#[cfg_attr(feature = "client", derive(Serialize, Deserialize))]
#[cfg_attr(not(feature = "client"), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageFree {
    pub pfn: Hex64,
    pub order: u32,
}

impl Pod for PageFree {
    const DISCRIMINANT: Option<u32> = Some(10);
    const SIZE: usize = 0x0c;

    #[inline(always)]
    fn from_slice(s: &[u8]) -> Option<Self> {
        if s.len() < Self::SIZE {
            return None;
        }
        Some(PageFree {
            pfn: Hex64(u64::from_ne_bytes(TryFrom::try_from(&s[0x00..0x08]).unwrap())),
            order: u32::from_ne_bytes(TryFrom::try_from(&s[0x08..0x0c]).unwrap()),
        })
    }
}

#[cfg_attr(feature = "client", derive(Serialize, Deserialize))]
#[cfg_attr(not(feature = "client"), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageFreeBatched {
    pub pfn: Hex64,
}

impl Pod for PageFreeBatched {
    const DISCRIMINANT: Option<u32> = Some(11);
    const SIZE: usize = 0x08;

    #[inline(always)]
    fn from_slice(s: &[u8]) -> Option<Self> {
        if s.len() < Self::SIZE {
            return None;
        }
        Some(PageFreeBatched {
            pfn: Hex64(u64::from_ne_bytes(TryFrom::try_from(&s[0x00..0x08]).unwrap())),
        })
    }
}

#[cfg_attr(feature = "client", derive(Serialize, Deserialize))]
#[cfg_attr(not(feature = "client"), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RssStat {
    pub id: u32,
    curr: u32,
    pub member: i32,
    pub size: i64,
}

impl Pod for RssStat {
    const DISCRIMINANT: Option<u32> = Some(13);
    const SIZE: usize = 0x18;

    #[inline(always)]
    fn from_slice(s: &[u8]) -> Option<Self> {
        if s.len() < Self::SIZE {
            return None;
        }
        Some(RssStat {
            id: u32::from_ne_bytes(TryFrom::try_from(&s[0x00..0x04]).unwrap()),
            curr: u32::from_ne_bytes(TryFrom::try_from(&s[0x04..0x08]).unwrap()),
            member: i32::from_ne_bytes(TryFrom::try_from(&s[0x08..0x0c]).unwrap()),
            size: i64::from_ne_bytes(TryFrom::try_from(&s[0x10..0x18]).unwrap()),
        })
    }
}

#[cfg_attr(feature = "client", derive(Serialize, Deserialize))]
#[cfg_attr(not(feature = "client"), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PercpuAlloc {
    reserved: bool,
    is_atomic: bool,
    size: Hex64,
    align: Hex64,
    base_address: Hex64,
    off: i32,
    ptr: Hex64,
}

impl Pod for PercpuAlloc {
    const DISCRIMINANT: Option<u32> = Some(15);
    const SIZE: usize = 0x30;

    #[inline(always)]
    fn from_slice(s: &[u8]) -> Option<Self> {
        if s.len() < Self::SIZE {
            return None;
        }
        Some(PercpuAlloc {
            reserved: s[0x00] != 0,
            is_atomic: s[0x01] != 0,
            size: Hex64(u64::from_ne_bytes(TryFrom::try_from(&s[0x08..0x10]).unwrap())),
            align: Hex64(u64::from_ne_bytes(TryFrom::try_from(&s[0x10..0x18]).unwrap())),
            base_address: Hex64(u64::from_ne_bytes(TryFrom::try_from(&s[0x18..0x20]).unwrap())),
            off: i32::from_ne_bytes(TryFrom::try_from(&s[0x20..0x24]).unwrap()),
            ptr: Hex64(u64::from_ne_bytes(TryFrom::try_from(&s[0x28..0x30]).unwrap())),
        })
    }
}

#[cfg_attr(feature = "client", derive(Serialize, Deserialize))]
#[cfg_attr(not(feature = "client"), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PercpuFree {
    base_address: Hex64,
    off: i32,
    ptr: Hex64,
}

impl Pod for PercpuFree {
    const DISCRIMINANT: Option<u32> = Some(15);
    const SIZE: usize = 0x18;

    #[inline(always)]
    fn from_slice(s: &[u8]) -> Option<Self> {
        if s.len() < Self::SIZE {
            return None;
        }
        Some(PercpuFree {
            base_address: Hex64(u64::from_ne_bytes(TryFrom::try_from(&s[0x00..0x08]).unwrap())),
            off: i32::from_ne_bytes(TryFrom::try_from(&s[0x08..0x0c]).unwrap()),
            ptr: Hex64(u64::from_ne_bytes(TryFrom::try_from(&s[0x10..0x18]).unwrap())),
        })
    }
}

#[cfg_attr(feature = "client", derive(Serialize, Deserialize))]
#[cfg_attr(not(feature = "client"), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddToPageCache {
    pub pfn: Hex64,
    i_ino: u64,
    index: u64,
    s_dev: u64,
}

impl Pod for AddToPageCache {
    const DISCRIMINANT: Option<u32> = Some(16);
    const SIZE: usize = 0x20;

    #[inline(always)]
    fn from_slice(s: &[u8]) -> Option<Self> {
        if s.len() < Self::SIZE {
            return None;
        }
        Some(AddToPageCache {
            pfn: Hex64(u64::from_ne_bytes(TryFrom::try_from(&s[0x00..0x08]).unwrap())),
            i_ino: u64::from_ne_bytes(TryFrom::try_from(&s[0x08..0x10]).unwrap()),
            index: u64::from_ne_bytes(TryFrom::try_from(&s[0x10..0x18]).unwrap()),
            s_dev: u64::from_ne_bytes(TryFrom::try_from(&s[0x18..0x20]).unwrap()),
        })
    }
}

#[cfg_attr(feature = "client", derive(Serialize, Deserialize))]
#[cfg_attr(not(feature = "client"), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoveFromPageCache {
    pub pfn: Hex64,
    i_ino: u64,
    index: u64,
    s_dev: u64,
}

impl Pod for RemoveFromPageCache {
    const DISCRIMINANT: Option<u32> = Some(17);
    const SIZE: usize = 0x20;

    #[inline(always)]
    fn from_slice(s: &[u8]) -> Option<Self> {
        if s.len() < Self::SIZE {
            return None;
        }
        Some(RemoveFromPageCache {
            pfn: Hex64(u64::from_ne_bytes(TryFrom::try_from(&s[0x00..0x08]).unwrap())),
            i_ino: u64::from_ne_bytes(TryFrom::try_from(&s[0x08..0x10]).unwrap()),
            index: u64::from_ne_bytes(TryFrom::try_from(&s[0x10..0x18]).unwrap()),
            s_dev: u64::from_ne_bytes(TryFrom::try_from(&s[0x18..0x20]).unwrap()),
        })
    }
}

#[cfg_attr(feature = "client", derive(Serialize, Deserialize))]
#[cfg_attr(not(feature = "client"), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigratePages {
    pub succeeded: u64,
    pub failed: u64,
    pub thp_succeeded: u64,
    pub thp_failed: u64,
    pub thp_split: u64,
    pub mode: u32,
    pub reason: u32,
}

impl Pod for MigratePages {
    const DISCRIMINANT: Option<u32> = Some(18);
    const SIZE: usize = 0x30;

    #[inline(always)]
    fn from_slice(s: &[u8]) -> Option<Self> {
        if s.len() < Self::SIZE {
            return None;
        }
        Some(MigratePages {
            succeeded: u64::from_ne_bytes(TryFrom::try_from(&s[0x00..0x08]).unwrap()),
            failed: u64::from_ne_bytes(TryFrom::try_from(&s[0x08..0x10]).unwrap()),
            thp_succeeded: u64::from_ne_bytes(TryFrom::try_from(&s[0x10..0x18]).unwrap()),
            thp_failed: u64::from_ne_bytes(TryFrom::try_from(&s[0x18..0x20]).unwrap()),
            thp_split: u64::from_ne_bytes(TryFrom::try_from(&s[0x20..0x28]).unwrap()),
            mode: u32::from_ne_bytes(TryFrom::try_from(&s[0x28..0x2c]).unwrap()),
            reason: u32::from_ne_bytes(TryFrom::try_from(&s[0x2c..0x30]).unwrap()),
        })
    }
}
