// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

#![cfg_attr(feature = "probes", no_std)]

#[cfg(feature = "facade")]
mod facade;
#[cfg(feature = "facade")]
pub use self::facade::{BpfModule, SnifferError, SnifferErrorCode, SnifferEvent, Command, BpfModuleClient};
#[cfg(feature = "facade")]
pub use redbpf::{ringbuf::{RingBufferData, RingBuffer, RingBufferObserver}, ringbuf_sync::RingBufferSync};

#[cfg(feature = "facade")]
mod bpf_code;

#[cfg(feature = "probes")]
mod syscall_context;
#[cfg(feature = "probes")]
pub use self::syscall_context::{SyscallContext, SyscallContextFull};

#[cfg(feature = "probes")]
pub mod send;

mod data_descriptor;
pub use self::data_descriptor::{SocketId, EventId, DataDescriptor, DataTag};

mod address;
#[cfg(feature = "probes")]
pub use self::address::Address;

#[cfg(feature = "probes")]
mod app;
#[cfg(feature = "probes")]
pub use self::app::*;
