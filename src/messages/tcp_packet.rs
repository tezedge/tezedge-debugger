use std::net::SocketAddr;
use smoltcp::{
    wire::{
        Ipv4Packet, Ipv6Packet, TcpPacket, IpProtocol as Protocol,
    },
};
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;

pub type IdAddrs = (SocketAddr, SocketAddr);

#[derive(Debug, Clone)]
/// Convenience wrapper around IPv4/IPv6 packet as single unit
pub enum Packet {
    V4(Ipv4Packet<Vec<u8>>),
    V6(Ipv6Packet<Vec<u8>>),
}

impl Packet {
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

    /// Get raw buffer of this packet
    pub fn ip_buffer(&self) -> &[u8] {
        match self {
            Self::V4(ref packet) => packet.as_ref(),
            Self::V6(ref packet) => packet.as_ref(),
        }
    }

    pub fn tcp_buffer(&self) -> &[u8] {
        match self {
            Self::V4(_) => Ipv4Packet::new_unchecked(self.ip_buffer()).payload(),
            Self::V6(_) => Ipv6Packet::new_unchecked(self.ip_buffer()).payload(),
        }
    }

    /// Get Tcp packet (without IP headers) from this IP packet.
    pub fn tcp_packet(&self) -> TcpPacket<&[u8]> {
        TcpPacket::new_unchecked(self.tcp_buffer())
    }

    #[inline]
    /// Get Socket address (IP address + TCP port number) of source generating this packet
    pub fn source_addr(&self) -> SocketAddr {
        let port = self.tcp_packet().src_port();
        match self {
            Self::V4(ref packet) => SocketAddr::new(packet.src_addr().0.into(), port),
            Self::V6(ref packet) => SocketAddr::new(packet.src_addr().0.into(), port),
        }
    }

    #[inline]
    /// Get Socket address (IP address + TCP port number) of source generating this packet
    pub fn destination_address(&self) -> SocketAddr {
        let port = self.tcp_packet().src_port();
        match self {
            Self::V4(ref packet) => SocketAddr::new(packet.dst_addr().0.into(), port),
            Self::V6(ref packet) => SocketAddr::new(packet.dst_addr().0.into(), port),
        }
    }

    #[inline]
    /// Socket Address identifying specific packet chain
    pub fn identification_pair(&self) -> IdAddrs {
        let mut hasher = DefaultHasher::new();
        self.source_addr().hash(&mut hasher);
        let sh = hasher.finish();
        let mut hasher = DefaultHasher::new();
        self.destination_address().hash(&mut hasher);
        let dh = hasher.finish();
        if sh < dh {
            (self.source_addr(), self.destination_address())
        } else {
            (self.destination_address(), self.source_addr())
        }
    }

    #[inline]
    /// Get raw payload buffer of this packet (Without TCP or IP headers)
    pub fn payload(&self) -> &[u8] {
        self.tcp_packet().payload()
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
    /// Check if is reset (RST) flag set in TCP header
    pub fn is_reset(&self) -> bool {
        self.tcp_packet().rst()
    }

    #[inline]
    /// Check if is finish (FIN) flag set in TCP header
    pub fn is_finish(&self) -> bool {
        self.tcp_packet().fin()
    }

    #[inline]
    /// Check if this packet closes connection
    pub fn is_closing(&self) -> bool {
        self.is_reset() || self.is_finish()
    }

    #[inline]
    /// Check if this packet has IPv4 header
    pub fn is_ipv4(&self) -> bool {
        if let Self::V4(_) = self {
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
}