// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use tokio::{
    net::UdpSocket,
    task::JoinHandle,
};
use crate::{
    storage_::{StoreCollector, log, indices::NodeName},
    system::NodeConfig,
};

/// Spawn new Syslog UDP server, for processing syslogs.
pub fn spawn<S>(storage: &S, node: &NodeConfig, running: Arc<AtomicBool>) -> JoinHandle<()>
where
    S: StoreCollector<Message = log::Message> + Clone + Send + 'static,
{
    // Create the server
    let syslog_port = node.syslog_port;
    let name = node.p2p_port.clone();
    let storage = storage.clone();
    tokio::spawn(async move {
        let socket = match UdpSocket::bind(("0.0.0.0", syslog_port)).await {
            Ok(socket) => socket,
            Err(err) => {
                tracing::error!(error = tracing::field::display(&err), "failed to bin syslog socket");
                return;
            },
        };
        tracing::info!(port = syslog_port, "started listening for syslog");

        // Local packet buffer
        let mut buffer = [0u8; 64 * 1024];
        while running.load(Ordering::Relaxed) {
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
                    tracing::error!(error = tracing::field::display(&err), "failed to store log");
                }
            }
        }
    })
}
