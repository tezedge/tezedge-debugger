// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::net::{IpAddr, SocketAddr};
use chrono::Duration;
use crate::utility::identity::Identity;
use crate::storage::MessageStore;
use crate::utility::stats::{AlertConfig, ChannelConfig};

pub mod orchestrator;
pub mod p2p_parser;
pub mod raw_socket_producer;
pub mod processor;
pub mod syslog_producer;
pub mod rpc_parser;
pub mod replayer;
pub mod metric_collector;

pub mod prelude {
    pub use super::p2p_parser::spawn_p2p_parser;
    pub use super::raw_socket_producer::raw_socket_producer;
    pub use super::orchestrator::spawn_packet_orchestrator;
    pub use super::SystemSettings;
}

/// Create whole new system consisting of packet producer, packet orchestrator, parsers and final processor
pub fn build_raw_socket_system(settings: SystemSettings) -> std::io::Result<()> {
    raw_socket_producer::raw_socket_producer(settings)
}

#[derive(Clone)]
/// System settings describing the running system
pub struct SystemSettings {
    pub identity: Identity,
    pub local_address: IpAddr,
    pub storage: MessageStore,
    pub syslog_port: u16,
    pub rpc_port: u16,
    pub node_rpc_port: u16,
    pub metrics_fetch_interval: Duration,
    pub notification_cfg: NotificationConfig,
    pub docker_daemon_address: Option<SocketAddr>,
}

#[derive(Clone)]
pub struct NotificationConfig {
    /// minimal interval between notifications
    pub minimal_interval: Duration,
    pub channel: Option<ChannelConfig>,
    pub alert_config: AlertConfig,
}
