// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::messages::tcp_packet::Packet;
use tracing::{trace, error, field::{display, debug}};
use std::{io, env, os::unix::io::AsRawFd};
use crate::system::orchestrator::spawn_packet_orchestrator;
use crate::system::SystemSettings;
use smoltcp::{
    time::Instant,
    wire::{EthernetFrame},
    phy::{RawSocket, Device, RxToken, wait},
};
use std::net::SocketAddr;

/// Spawn new packet producer, which is driven by the raw socket
pub fn raw_socket_producer(settings: SystemSettings) -> io::Result<()> {
    // Firstly, spawn the orchestrator for incoming messages
    let orchestrator = spawn_packet_orchestrator(settings.clone());
    // Get the interface name
    let ifname = env::args().nth(1)
        .unwrap_or("eth0".to_string());
    std::thread::spawn(move || {
        // Local packet buffer, to reduce allocations
        let mut packet_buf = [0u8; 64 * 1024];
        // Spawn new Raw Socket
        let mut socket = RawSocket::new(&ifname)
            .unwrap();
        loop {
            // Wait until the file descriptor is readable
            let _ = wait(socket.as_raw_fd(), None);
            // Read new packet
            if let Some((rx_token, _)) = socket.receive() {
                let packet_frame = rx_token.consume(Instant::now(), |buffer| {
                    // And copy it into local buffer
                    (packet_buf[..buffer.len()]).clone_from_slice(buffer);
                    let data = &packet_buf[..buffer.len()];
                    let frame = EthernetFrame::new_unchecked(data);
                    Ok(frame.payload())
                }).unwrap();

                // Check if the received packet is valid TCP packet
                if let Some(packet) = Packet::new(packet_frame) {
                    let local_rpc_addr = SocketAddr::new(settings.local_address, settings.rpc_port);
                    let local_syslog_addr = SocketAddr::new(settings.local_address, settings.syslog_port);

                    if packet.source_address() == local_rpc_addr || packet.destination_address() == local_rpc_addr ||
                        packet.source_address() == local_syslog_addr || packet.destination_address() == local_syslog_addr {
                        // this is for local server, ignore
                        continue;
                    }

                    // If so, send it to the orchestrator for further processing
                    match orchestrator.send(packet) {
                        Ok(_) => {
                            trace!("sent packet for processing");
                        }
                        Err(_) => {
                            error!("orchestrator channel closed abruptly");
                        }
                    }
                }
            }
        }
    });

    Ok(())
}