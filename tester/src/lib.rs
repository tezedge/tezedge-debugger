// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

#![forbid(unsafe_code)]

mod buffer;
pub mod handshake;

pub use self::buffer::{ChunkBuffer, Message};

pub const START_TIME: i64 = 1626264000;

//mod generator;
//pub use self::generator::{RandomState, Generator};
