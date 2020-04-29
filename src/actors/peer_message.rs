// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use ip_packet::IpPacket;
use smoltcp::wire::TcpPacket;
use std::net::SocketAddr;

#[derive(Debug, Clone, Copy, PartialEq)]
/// Packet character describing where which tun-device it came from and which one it is destined for
pub enum PacketCharacter {
    /// Packet is in InnerIncoming state, iff it is already processed and should be relayed to node
    InnerIncoming,
    /// Packet is in InnerOutgoing state, iff it needs processing and then forwarded to internet
    InnerOutgoing,
    /// Packet is in OuterIncoming state, iff it needs processing and then forwarded to node
    OuterIncoming,
    /// Packet is in OuterOutgoing state, iff it is already processed and should be relayed to internet
    OuterOutgoing,
}

impl From<(bool, bool)> for PacketCharacter {
    fn from((is_inner, is_incoming): (bool, bool)) -> Self {
        match (is_inner, is_incoming) {
            (true, true) => PacketCharacter::InnerIncoming,
            (true, false) => PacketCharacter::InnerOutgoing,
            (false, true) => PacketCharacter::OuterIncoming,
            (false, false) => PacketCharacter::OuterOutgoing,
        }
    }
}

#[derive(Debug, Clone)]
/// Enum describing different states of packet as it is seen in the debugger
pub enum SenderMessage {
    /// Packet needs to be processed by either RPC processor or P2P processor
    Process(RawPacketMessage),
    /// Packet is already processed and need to be relay to internet/node
    Relay(RawPacketMessage),
    /// Packet is not meant to be processed by proxy, just forward it.
    Forward(bool, Vec<u8>),
}


#[derive(Debug)]
/// Semi-deserialized packet meant to be processed further by debugger
pub struct RawPacketMessage {
    is_incoming: bool,
    is_inner: bool,
    packet: IpPacket,
}

impl RawPacketMessage {
    /// Create new Packet with InnerIncoming character from raw buffer
    pub fn inner_incoming<T: AsRef<[u8]>>(packet: T) -> Option<Self> {
        Self::new(packet.as_ref(), true, true)
    }

    /// Create new Packet with InnerOutgoing character from raw buffer
    pub fn inner_outgoing<T: AsRef<[u8]>>(packet: T) -> Option<Self> {
        Self::new(packet.as_ref(), false, true)
    }

    /// Create new Packet with OuterIncoming character from raw buffer
    pub fn outer_incoming<T: AsRef<[u8]>>(packet: T) -> Option<Self> {
        Self::new(packet.as_ref(), true, false)
    }

    /// Create new Packet with InnerOutgoing character from raw buffer
    pub fn outer_outgoing<T: AsRef<[u8]>>(packet: T) -> Option<Self> {
        Self::new(packet.as_ref(), false, false)
    }

    /// Create new characterless packet from raw buffer
    pub fn partial<T: AsRef<[u8]>>(packet: T) -> Option<Self> {
        Self::outer_outgoing(packet)
    }

    /// Get raw buffer of packet (with headers)
    pub fn buffer(&self) -> &[u8] {
        self.packet.buffer()
    }

    #[inline]
    /// Clone buffer of packet (with headers)
    pub fn clone_packet(&self) -> Vec<u8> {
        self.buffer().to_vec()
    }

    #[inline]
    /// Get tcp part of buffer wrapped in convenience structure (without IP headers)
    pub fn tcp_packet(&self) -> TcpPacket<&[u8]> {
        self.packet.tcp_packet()
    }

    #[inline]
    /// Get ip part of buffer wrapped in convenience structure (raw buffer wrapped in IpPacket)
    pub fn ip_packet(&self) -> &IpPacket {
        &self.packet
    }


    #[inline]
    /// Get Socket address (IP address + TCP port number) of source generating this packet
    pub fn source_addr(&self) -> SocketAddr {
        let port = self.tcp_packet().src_port();
        match self.packet {
            IpPacket::V4(ref packet) => SocketAddr::new(packet.src_addr().0.into(), port),
            IpPacket::V6(ref packet) => SocketAddr::new(packet.src_addr().0.into(), port),
        }
    }

    #[inline]
    /// Get Socket address (IP address + TCP port number) for destination of this packet
    pub fn destination_addr(&self) -> SocketAddr {
        let port = self.tcp_packet().dst_port();
        match self.packet {
            IpPacket::V4(ref packet) => SocketAddr::new(packet.dst_addr().0.into(), port),
            IpPacket::V6(ref packet) => SocketAddr::new(packet.dst_addr().0.into(), port),
        }
    }

    #[inline]
    /// Get Socket address (IP address + TCP port number) for the endpoint remote of this node
    /// * Source address for incoming packets
    /// * Destination address for outgoing packets
    pub fn remote_addr(&self) -> SocketAddr {
        if self.is_incoming {
            self.source_addr()
        } else {
            self.destination_addr()
        }
    }

    #[inline]
    /// Get raw payload buffer of this packet (Without TCP or IP headers)
    pub fn payload(&self) -> &[u8] {
        self.packet.tcp_packet().payload()
    }

    #[inline]
    /// Check if packet has any (non-header related) payload
    pub fn has_payload(&self) -> bool {
        self.payload().len() > 0
    }

    #[inline]
    /// Check if is push (PSH) flag set in TCP header
    pub fn is_push(&self) -> bool {
        self.tcp_packet().psh()
    }

    #[inline]
    /// Check if this packet is incoming to the local node
    pub fn is_incoming(&self) -> bool {
        self.is_incoming
    }

    #[inline]
    /// Check if this packet is outgoing from local note
    pub fn is_outgoing(&self) -> bool {
        !self.is_incoming
    }

    #[inline]
    /// Flip direction of packet from incoming to outgoing and vice-versa
    pub fn flip_direction(&mut self) {
        self.set_is_incoming(!self.is_incoming);
    }

    #[inline]
    /// Set specific direction for this packet
    pub fn set_is_incoming(&mut self, value: bool) {
        self.is_incoming = value;
    }

    #[inline]
    /// Check if this packet came from inner tun device
    pub fn is_inner(&self) -> bool {
        self.is_inner
    }

    #[inline]
    /// Check if this packet came from outer tun device
    pub fn is_outer(&self) -> bool {
        !self.is_inner
    }

    #[inline]
    /// Flip side of which this packet came from from inner to outer and vice versa
    pub fn flip_side(&mut self) {
        self.set_is_inner(!self.is_inner);
    }

    #[inline]
    /// Set origin of this packet
    pub fn set_is_inner(&mut self, value: bool) {
        self.is_inner = value;
    }

    #[inline]
    /// Check if this packet has IPv4 header
    pub fn is_ipv4(&self) -> bool {
        if let IpPacket::V4(_) = self.packet {
            true
        } else {
            false
        }
    }

    #[inline]
    /// Check if this packet has IPv6 header
    pub fn is_ipv6(&self) -> bool {
        !self.is_ipv4()
    }

    #[inline]
    /// Get character of this packet
    pub fn character(&self) -> PacketCharacter {
        (self.is_incoming, self.is_inner).into()
    }

    fn new(buffer: &[u8], is_incoming: bool, is_inner: bool) -> Option<Self> {
        Some(Self {
            packet: IpPacket::new(buffer)?,
            is_inner,
            is_incoming,
        })
    }
}

impl Clone for RawPacketMessage {
    fn clone(&self) -> Self {
        Self {
            is_incoming: self.is_incoming,
            is_inner: self.is_inner,
            packet: IpPacket::new(self.packet.buffer())
                .expect("failed to clone valid packet from valid packet"),
        }
    }
}

mod ip_packet {
    use smoltcp::{
        wire::{
            Ipv4Packet, Ipv6Packet, TcpPacket, IpProtocol as Protocol,
        },
    };

    #[derive(Debug, Clone)]
    /// Convenience wrapper around IPv4/IPv6 packet as single unit
    pub enum IpPacket {
        V4(Ipv4Packet<Vec<u8>>),
        V6(Ipv6Packet<Vec<u8>>),
    }

    impl IpPacket {
        /// Build new (semi-universal) packet from raw buffer, from *correct* IPv(4/6) + TCP packet
        /// No other protocols are supported
        pub fn new(buf: &[u8]) -> Option<Self> {
            if buf.len() == 0 {
                return None;
            }
            let ver = buf[0] >> 4;

            if ver == 4 {
                let packet = Ipv4Packet::new_checked(buf).ok()?;
                if packet.protocol() != Protocol::Tcp {
                    return None;
                }
                Some(Self::V4(Ipv4Packet::new_unchecked(buf.to_vec())))
            } else if ver == 6 {
                let packet = Ipv6Packet::new_checked(buf).ok()?;
                if packet.next_header() != Protocol::Tcp {
                    return None;
                }
                Some(Self::V6(Ipv6Packet::new_unchecked(buf.to_vec())))
            } else {
                None
            }
        }

        /// Get payload of this IPv(4/6) packet (including next protocol header)
        pub fn payload(&self) -> &[u8] {
            match self {
                Self::V4(ref packet) => Ipv4Packet::new_unchecked(packet.as_ref()).payload(),
                Self::V6(ref packet) => Ipv6Packet::new_unchecked(packet.as_ref()).payload(),
            }
        }

        /// Get raw buffer of this packet
        pub fn buffer(&self) -> &[u8] {
            match self {
                Self::V4(ref packet) => packet.as_ref(),
                Self::V6(ref packet) => packet.as_ref(),
            }
        }

        /// Get Tcp packet (without IP headers) from this IP packet.
        pub fn tcp_packet(&self) -> TcpPacket<&[u8]> {
            TcpPacket::new_unchecked(self.payload())
        }
    }
}