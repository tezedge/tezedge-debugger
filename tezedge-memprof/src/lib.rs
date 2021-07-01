// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

#![forbid(unsafe_code)]

mod memory_map;

mod state;
pub use self::state::{AtomicState, Reporter};

mod history;
pub use self::history::{Page, History, AllocationState, FrameReport, EventLast, Tracker};

mod stack;
pub use self::stack::StackResolver;

mod table;

pub mod server;
