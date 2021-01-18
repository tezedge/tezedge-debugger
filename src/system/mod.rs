// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

pub mod syslog_producer;
pub mod rpc_parser;
//pub mod replayer;

// new socket capturing system
mod bpf_sniffer;
mod p2p;
pub use self::bpf_sniffer::{BpfSniffer, BpfSnifferCommand, BpfSnifferResponse, BpfSnifferReport};
mod processor;

// old socket capturing system
//mod orchestrator;
//mod p2p_parser;
//mod raw_socket_producer;

// old socket capturing system
//pub use self::orchestrator::CONNECTIONS;

// old socket capturing system
/// Create whole new system consisting of packet producer, packet orchestrator, parsers and final processor
//pub fn build_raw_socket_system(settings: SystemSettings) -> std::io::Result<()> {
//    raw_socket_producer::raw_socket_producer(settings)
//}

mod system_settings {
    use std::net::IpAddr;
    use crate::storage::MessageStore;

    #[derive(Clone)]
    /// System settings describing the running system
    pub struct SystemSettings {
        pub local_address: IpAddr,
        pub storage: MessageStore,
        pub syslog_port: u16,
        pub rpc_port: u16,
        pub node_p2p_port: u16,
        pub node_rpc_port: u16,
    }
}
pub use self::system_settings::SystemSettings;
