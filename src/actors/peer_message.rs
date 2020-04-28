use ip_packet::IpPacket;
use smoltcp::wire::TcpPacket;
use std::net::SocketAddr;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PacketCharacter {
    InnerIncoming,
    InnerOutgoing,
    OuterIncoming,
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
pub enum SenderMessage {
    Process(RawPacketMessage),
    Relay(RawPacketMessage),
    Forward(bool, Vec<u8>),
}


#[derive(Debug)]
pub struct RawPacketMessage {
    is_incoming: bool,
    is_inner: bool,
    packet: IpPacket,
}

impl RawPacketMessage {
    pub fn inner_incoming<T: AsRef<[u8]>>(packet: T) -> Option<Self> {
        Self::new(packet.as_ref(), true, true)
    }

    pub fn inner_outgoing<T: AsRef<[u8]>>(packet: T) -> Option<Self> {
        Self::new(packet.as_ref(), false, true)
    }

    pub fn outer_incoming<T: AsRef<[u8]>>(packet: T) -> Option<Self> {
        Self::new(packet.as_ref(), true, false)
    }

    pub fn outer_outgoing<T: AsRef<[u8]>>(packet: T) -> Option<Self> {
        Self::new(packet.as_ref(), false, false)
    }

    pub fn partial<T: AsRef<[u8]>>(packet: T) -> Option<Self> {
        Self::outer_outgoing(packet)
    }

    pub fn buffer(&self) -> &[u8] {
        self.packet.buffer()
    }

    #[inline]
    pub fn clone_packet(&self) -> Vec<u8> {
        self.buffer().to_vec()
    }

    #[inline]
    pub fn tcp_packet(&self) -> TcpPacket<&[u8]> {
        self.packet.tcp_packet()
    }

    #[inline]
    pub fn ip_packet(&self) -> &IpPacket {
        &self.packet
    }


    #[inline]
    pub fn source_addr(&self) -> SocketAddr {
        let port = self.tcp_packet().src_port();
        match self.packet {
            IpPacket::V4(ref packet) => SocketAddr::new(packet.src_addr().0.into(), port),
            IpPacket::V6(ref packet) => SocketAddr::new(packet.src_addr().0.into(), port),
        }
    }

    #[inline]
    pub fn destination_addr(&self) -> SocketAddr {
        let port = self.tcp_packet().dst_port();
        match self.packet {
            IpPacket::V4(ref packet) => SocketAddr::new(packet.dst_addr().0.into(), port),
            IpPacket::V6(ref packet) => SocketAddr::new(packet.dst_addr().0.into(), port),
        }
    }

    #[inline]
    pub fn remote_addr(&self) -> SocketAddr {
        if self.is_incoming {
            self.source_addr()
        } else {
            self.destination_addr()
        }
    }

    #[inline]
    pub fn payload(&self) -> &[u8] {
        self.packet.tcp_packet().payload()
    }

    #[inline]
    pub fn has_payload(&self) -> bool {
        self.payload().len() > 0
    }

    #[inline]
    pub fn is_push(&self) -> bool {
        self.tcp_packet().psh()
    }

    #[inline]
    pub fn is_incoming(&self) -> bool {
        self.is_incoming
    }

    #[inline]
    pub fn is_outgoing(&self) -> bool {
        !self.is_incoming
    }

    #[inline]
    pub fn flip_direction(&mut self) {
        self.set_is_incoming(!self.is_incoming);
    }

    #[inline]
    pub fn set_is_incoming(&mut self, value: bool) {
        self.is_incoming = value;
    }

    #[inline]
    pub fn is_inner(&self) -> bool {
        self.is_inner
    }

    #[inline]
    pub fn is_outer(&self) -> bool {
        !self.is_inner
    }

    #[inline]
    pub fn flip_side(&mut self) {
        self.set_is_inner(!self.is_inner);
    }

    #[inline]
    pub fn set_is_inner(&mut self, value: bool) {
        self.is_inner = value;
    }

    #[inline]
    pub fn is_ipv4(&self) -> bool {
        if let IpPacket::V4(_) = self.packet {
            true
        } else {
            false
        }
    }

    #[inline]
    pub fn is_ipv6(&self) -> bool {
        !self.is_ipv4()
    }

    #[inline]
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
    pub enum IpPacket {
        V4(Ipv4Packet<Vec<u8>>),
        V6(Ipv6Packet<Vec<u8>>),
    }

    impl IpPacket {
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

        pub fn payload(&self) -> &[u8] {
            match self {
                Self::V4(ref packet) => Ipv4Packet::new_unchecked(packet.as_ref()).payload(),
                Self::V6(ref packet) => Ipv6Packet::new_unchecked(packet.as_ref()).payload(),
            }
        }

        pub fn buffer(&self) -> &[u8] {
            match self {
                Self::V4(ref packet) => packet.as_ref(),
                Self::V6(ref packet) => packet.as_ref(),
            }
        }

        pub fn tcp_packet(&self) -> TcpPacket<&[u8]> {
            TcpPacket::new_unchecked(self.payload())
        }
    }
}