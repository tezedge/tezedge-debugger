// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

pub mod syslog_producer;
//pub mod rpc_parser;
//pub mod replayer;

// new socket capturing system
mod bpf_sniffer;
mod p2p;
pub use self::bpf_sniffer::{BpfSniffer, BpfSnifferCommand, BpfSnifferResponse, BpfSnifferReport};
mod processor;

mod system_settings {
    use crate::storage::MessageStore;

    #[derive(Clone)]
    /// System settings describing the running system
    pub struct SystemSettings {
        pub storage: MessageStore,
        pub syslog_port: u16,
        pub rpc_port: u16,
        pub node_p2p_port: u16,
        pub node_rpc_port: u16,
    }
}
pub use self::system_settings::SystemSettings;
