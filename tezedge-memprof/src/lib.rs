// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

#![forbid(unsafe_code)]

mod memory_map;

mod state;
pub use self::state::{AtomicState, Reporter};

mod history;
pub use self::history::{Page, History, FrameReport, EventLast};

mod stack;
pub use self::stack::StackResolver;
