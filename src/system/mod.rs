// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

pub mod syslog_producer;
//pub mod rpc_parser;
//pub mod replayer;

// new socket capturing system
mod parser;
mod reporter;
mod p2p;

pub use self::{
    parser::Parser,
    reporter::Reporter,
    p2p::Report as P2pReport,
};

mod processor;

mod system_settings {
    use crate::storage::MessageStore;

    #[derive(Clone)]
    /// System settings describing the running system
    pub struct SystemSettings {
        pub storage: MessageStore,
        pub namespace: String,
        pub syslog_port: u16,
        pub rpc_port: u16,
        pub node_p2p_port: u16,
        pub node_rpc_port: u16,
        pub max_message_number: u64,
    }
}
pub use self::system_settings::SystemSettings;
