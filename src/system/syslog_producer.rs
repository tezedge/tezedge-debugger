// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use tracing::{error, info};
use tokio::{
    io,
    net::UdpSocket,
};
use crate::{
    storage_::{LogStore, StoreCollector, log, indices::NodeName},
    system::NodeConfig,
};

/// Spawn new Syslog UDP server, for processing syslogs.
pub async fn syslog_producer(storage: &LogStore, node: &NodeConfig) -> io::Result<()> {
    // Create the server
    let syslog_port = node.syslog_port;
    let name = node.p2p_port.clone();
    let storage = storage.clone();
    let socket = UdpSocket::bind(("0.0.0.0", syslog_port)).await?;
    info!(port = syslog_port, "started listening for syslog");
    tokio::spawn(async move {
        // Local packet buffer
        let mut buffer = [0u8; 64 * 1024];
        loop {
            // Read data from UDP server
            let read = socket.recv(&mut buffer)
                .await.unwrap(); // This unwrap is safe, as socket was bound before reading
            let datagram = &buffer[..read];
            // Syslog are textual format, all received datagrams must be valid strings.
            if let Ok(log) = std::str::from_utf8(&datagram) {
                let msg = syslog_loose::parse_message(log);
                let mut log_msg = log::Message::from(msg);
                log_msg.node_name = NodeName(name.clone());
                if let Err(err) = storage.store_message(log_msg) {
                    error!(error = tracing::field::display(&err), "failed to store log");
                }
            }
        }
    });
    Ok(())
}
