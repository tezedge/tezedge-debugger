// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use tracing::{error, info};
use tokio::{
    io,
    net::UdpSocket,
};
use crate::system::SystemSettings;
use crate::messages::log_message::LogMessage;

/// Spawn new Syslog UDP server, for processing syslogs.
pub async fn syslog_producer(settings: SystemSettings) -> io::Result<()> {
    // Create the server
    let socket = UdpSocket::bind(("0.0.0.0", settings.syslog_port)).await?;
    info!(port = 13131, "started listening for syslog");
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
                let mut log_msg = LogMessage::from(msg);
                if let Err(err) = settings.storage.log().store_message(&mut log_msg) {
                    error!(error = tracing::field::display(&err), "failed to store log");
                }
            }
        }
    });
    Ok(())
}