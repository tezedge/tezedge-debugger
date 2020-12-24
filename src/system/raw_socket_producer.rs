// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{io, env, net::{SocketAddr, IpAddr}, fmt};
use crate::system::{
    orchestrator::spawn_packet_orchestrator,
    SystemSettings,
};
use crate::utility::pcap_facade;

use pnet::packet::{
    Packet,
    ethernet::*, // {EthernetPacket, EtherTypes}
    ip::IpNextHeaderProtocols,
    ipv4::*, // Ipv4Packet
    ipv6::*, // Ipv6Packet
    tcp::*, // {TcpPacket, TcpFlags}
};

pub struct P2pPacket {
    pub source_address: SocketAddr,
    pub destination_address: SocketAddr,
    pub is_closing: bool,
    pub is_opening: bool,
    pub payload: Vec<u8>,
    pub counter: u64,
}

impl fmt::Display for P2pPacket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("P2pPacket")
            .field("source_address", &self.source_address)
            .field("destination_address", &self.destination_address)
            .field("is_closing", &self.is_closing)
            .field("is_opening", &self.is_opening)
            .field("payload", &hex::encode(self.payload.as_slice()))
            .field("counter", &self.counter)
            .finish()
    }
}

fn handle_tcp(counter: u64, settings: &SystemSettings, source: IpAddr, destination: IpAddr, payload: &[u8]) -> Option<P2pPacket> {
    if let Some(tcp) = TcpPacket::new(payload) {
        let source_is_rcp_or_syslog = source == settings.local_address && (
            tcp.get_source() == settings.rpc_port ||
            tcp.get_source() == settings.syslog_port
        );
        let destination_is_rcp_or_syslog = destination == settings.local_address && (
            tcp.get_destination() == settings.rpc_port ||
            tcp.get_destination() == settings.syslog_port
        );
        if source_is_rcp_or_syslog || destination_is_rcp_or_syslog {
            // this is for local server, ignore
            return None;
        }
        let packet = P2pPacket {
            source_address: SocketAddr::new(source, tcp.get_source()),
            destination_address: SocketAddr::new(destination, tcp.get_destination()),
            is_closing: (tcp.get_flags() & TcpFlags::FIN) != 0 || (tcp.get_flags() & TcpFlags::RST) != 0,
            is_opening: (tcp.get_flags() & TcpFlags::SYN) != 0,
            payload: tcp.payload().to_vec(),
            counter,
        };
        if !packet.payload.is_empty() {
            tracing::trace!(
                number = counter,
                source = tracing::field::display(packet.source_address),
                destination = tracing::field::display(packet.destination_address),
                // payload = tracing::field::display(hex::encode(&packet.payload)),
                "intercepted",
            );
        }
        Some(packet)
    } else {
        tracing::warn!("bad Tcp header");
        None
    }
}

fn handle_ethernet(counter: u64, settings: &SystemSettings, packet: &[u8]) -> Option<P2pPacket> {
    let packet = EthernetPacket::new(packet).unwrap();
    match packet.get_ethertype() {
        EtherTypes::Ipv4 => {
            if let Some(header) = Ipv4Packet::new(packet.payload()) {
                match header.get_next_level_protocol() {
                    IpNextHeaderProtocols::Tcp => {
                        handle_tcp(
                            counter,
                            &settings,
                            IpAddr::V4(header.get_source()),
                            IpAddr::V4(header.get_destination()),
                            header.payload(),
                        )
                    },
                    _ => None, // silently ignore every not Tcp packet
                }
            } else {
                tracing::warn!("bad Ipv4 header");
                None
            }
        },
        EtherTypes::Ipv6 => {
            if let Some(header) = Ipv6Packet::new(packet.payload()) {
                match header.get_next_header() {
                    IpNextHeaderProtocols::Tcp => {
                        handle_tcp(
                            counter,
                            &settings,
                            IpAddr::V6(header.get_source()),
                            IpAddr::V6(header.get_destination()),
                            header.payload(),
                        )
                    },
                    _ => None, // silently ignore every not Tcp packet
                }
            } else {
                tracing::warn!("bad Ipv6 header");
                None
            }
        },
        _ => None, // silently ignore every not Ipv4 nor Ipv6 packet
    }
}

/// Spawn new packet producer, which is driven by the raw socket
pub fn raw_socket_producer(settings: SystemSettings) -> io::Result<()> {
    // Firstly, spawn the orchestrator for incoming messages
    let orchestrator = spawn_packet_orchestrator(settings.clone());
    // Get the interface name
    let ifname = env::args().nth(1);
    std::thread::spawn(move || {
        // the overall packet counter starting from 1, like wireshark
        let mut counter = 1;
        pcap_facade::with_capture(ifname, |mut cap| {
            loop {
                match cap.next() {
                    Ok(packet) => {
                        if let Some(packet) = handle_ethernet(counter, &settings, packet) {
                            counter += 1;
                            // If so, send it to the orchestrator for further processing
                            match orchestrator.send(packet) {
                                Ok(_) => {
                                    tracing::trace!("sent packet for processing");
                                }
                                Err(_) => {
                                    tracing::error!("orchestrator channel closed abruptly");
                                }
                            }
                        }
                    },
                    Err(error) => {
                        tracing::error!(
                            error = tracing::field::display(&error),
                            "pcap library error, failed to capture",
                        )
                    },
                }
            }
        })
    });

    Ok(())
}
