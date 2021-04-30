// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

#![cfg_attr(feature = "kern", no_std)]

mod event;
pub use self::event::{Pod, Hex32, Hex64, CommonHeader};
pub use self::event::{
    KFree, KMAlloc, KMAllocNode, CacheAlloc, CacheAllocNode, CacheFree, PageAlloc, PageAllocExtFrag,
    PageAllocZoneLocked, PageFree, PageFreeBatched, PagePcpuDrain,
};
pub use self::event::{PageFaultUser, RssStat};

#[cfg(feature = "client")]
mod client;
#[cfg(feature = "client")]
pub use self::client::{Client, EventKind, Event, Stack};

pub const STACK_MAX_DEPTH: usize = 127;
