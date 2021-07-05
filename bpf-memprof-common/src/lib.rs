// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

#![cfg_attr(not(feature = "client"), no_std)]

mod event;
pub use self::event::{Pod, Hex32, Hex64, CommonHeader};
pub use self::event::{
    KFree, KMAlloc, KMAllocNode, CacheAlloc, CacheAllocNode, CacheFree, PageAlloc, PageFree,
    PageFreeBatched,
};
pub use self::event::{RssStat, PercpuAlloc, PercpuFree, AddToPageCache, RemoveFromPageCache};

#[cfg(feature = "client")]
mod client;
#[cfg(feature = "client")]
pub use self::client::{Client, ClientCallback, EventKind, Event, Stack};

pub const STACK_MAX_DEPTH: usize = 127;
