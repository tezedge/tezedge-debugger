// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
mod facade;
#[cfg(feature = "std")]
pub use self::facade::{Command, SnifferEvent, SnifferError, SnifferErrorCode, BpfModuleClient};
#[cfg(feature = "std")]
pub use bpf_ring_buffer::{RingBuffer, RingBufferSync, RingBufferObserver};

mod data_descriptor;
pub use self::data_descriptor::{SocketId, EventId, DataDescriptor, DataTag};
