// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

#![forbid(unsafe_code)]

pub mod tables;

mod system;
pub use self::system::System;

mod connection;
pub mod main_loop;

pub mod database;

mod server;
