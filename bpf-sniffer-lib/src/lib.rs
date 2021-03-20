// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

#![cfg_attr(feature = "probes", no_std)]

#[cfg(feature = "facade")]
mod module;
#[cfg(feature = "facade")]
pub use self::module::BpfModule;

#[cfg(feature = "probes")]
mod syscall_context;
#[cfg(feature = "probes")]
pub use self::syscall_context::{SyscallContext, SyscallContextFull};

#[cfg(feature = "probes")]
pub mod send;

#[cfg(feature = "probes")]
mod address;
#[cfg(feature = "probes")]
pub use self::address::Address;

#[cfg(feature = "probes")]
mod app;
#[cfg(feature = "probes")]
pub use self::app::*;
