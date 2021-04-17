// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

#![cfg_attr(not(feature = "client"), no_std)]

#[cfg(feature = "client")]
mod client;
#[cfg(feature = "client")]
pub use self::client::Client;

mod event;
pub use self::event::{Event, EventKind};
