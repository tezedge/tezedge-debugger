// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

#![forbid(unsafe_code)]

mod common;
pub mod tables;
mod system;
mod log_client;
mod processor;
pub mod main_loop;
pub mod database;
mod server;

pub use self::system::System;
