// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT


pub mod syslog_producer;
//pub mod rpc_parser;
//pub mod replayer;

// new capturing system
mod reporter;
pub use self::reporter::Reporter;

#[cfg(target_os = "linux")]
mod utils;
#[cfg(target_os = "linux")]
mod parser;
#[cfg(target_os = "linux")]
mod p2p;

mod ring_buffer_analyzer;

mod settings {
    use serde::Deserialize;

    #[derive(Clone, Deserialize)]
    pub struct NodeConfig {
        pub name: String,
        pub identity_path: String,
        pub syslog_port: u16,
        pub p2p_port: u16,
    }

    #[derive(Clone, Deserialize)]
    pub struct DebuggerConfig {
        pub db_path: String,
        pub rpc_port: u16,
        pub p2p_message_limit: u64,
        pub log_message_limit: u64,
        pub bpf_sniffer: String,
        pub keep_db: bool,
        pub nodes: Vec<NodeConfig>,
    }
}
pub use self::settings::{NodeConfig, DebuggerConfig};
