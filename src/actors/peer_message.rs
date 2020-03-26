use packet::{
    Error as PacketError,
    PacketMut as _,
    ip::Packet as IpPacket,
    tcp::Packet as TcpPacket,
    Packet,
};
use std::net::{
    Ipv6Addr, IpAddr,
};
use packet::ip::Protocol;

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

#[derive(Debug)]
pub struct RawPacketMessage {
    is_incoming: bool,
    is_inner: bool,
    packet: IpPacket<Vec<u8>>,
}

impl RawPacketMessage {
    pub fn inner_incoming<T: Into<Vec<u8>>>(packet: T) -> Result<Self, PacketError> {
        Self::new(packet.into(), true, true)
    }

    pub fn inner_outgoing<T: Into<Vec<u8>>>(packet: T) -> Result<Self, PacketError> {
        Self::new(packet.into(), false, true)
    }

    pub fn outer_incoming<T: Into<Vec<u8>>>(packet: T) -> Result<Self, PacketError> {
        Self::new(packet.into(), true, false)
    }

    pub fn outer_outgoing<T: Into<Vec<u8>>>(packet: T) -> Result<Self, PacketError> {
        Self::new(packet.into(), false, false)
    }

    pub fn partial<T: Into<Vec<u8>>>(packet: T) -> Result<Self, PacketError> {
        Self::outer_outgoing(packet)
    }

    #[inline]
    pub fn clone_packet(&self) -> Vec<u8> {
        let ip_packet = self.ip_packet();
        let (header, payload) = ip_packet.split();
        let mut packet = Vec::with_capacity(header.len() + payload.len());
        packet.extend_from_slice(header);
        packet.extend_from_slice(payload);
        packet
    }

    #[inline]
    pub fn tcp_packet(&self) -> TcpPacket<&[u8]> {
        TcpPacket::new(self.packet.payload())
            .expect("Non-tcp packet found, even though all packet are checked on creation")
    }

    #[inline]
    pub fn tcp_packet_mut(&mut self) -> TcpPacket<&mut [u8]> {
        TcpPacket::new(self.packet.payload_mut())
            .expect("Non-tcp packet found, even though all packet are checked on creation")
    }

    #[inline]
    pub fn update_tcp_packet_checksum(&mut self) -> Result<(), PacketError> {
        if let IpPacket::V4(ref mut packet) = self.ip_packet_mut() {
            let checksum: u16 = {
                use pnet::packet::tcp::{TcpPacket as PnetTcpPacket, ipv4_checksum};
                let src = packet.source();
                let dst = packet.destination();
                let tcp_packet = packet.payload();
                let tcp_packet = PnetTcpPacket::new(tcp_packet).unwrap();
                ipv4_checksum(&tcp_packet, &src, &dst)
            };

            self.tcp_packet_mut().set_checksum(checksum)?;

            // use packet::ip::v4::Packet as IPv4Packet;
            // let (ip_header, tcp_packet) = packet.split_mut();
            // let ip_header = IpPacket::V4(IPv4Packet::no_payload(ip_header)?);
            // let mut tcp_packet = TcpPacket::new(tcp_packet)?;
            // tcp_packet.update_checksum(&ip_header)?;
            Ok(())
        } else {
            // TODO: Add IPv6 support
            Ok(())
        }
    }

    #[inline]
    pub fn ip_packet(&self) -> &IpPacket<Vec<u8>> {
        &self.packet
    }

    #[inline]
    pub fn ip_packet_mut(&mut self) -> &mut IpPacket<Vec<u8>> {
        &mut self.packet
    }

    #[inline]
    pub fn update_ip_packet_checksum(&mut self) -> Result<(), PacketError> {
        let raw = self.clone_packet();
        if let IpPacket::V4(ref mut packet) = self.ip_packet_mut() {
            use pnet::packet::{
                ipv4::{Ipv4Packet as PnetIpv4Packet, checksum}
            };
            let ppacket = PnetIpv4Packet::new(&raw).unwrap();
            let checksum = checksum(&ppacket);
            packet.set_checksum(checksum)?;
            Ok(())
        } else {
            // TODO: Add IPv6 support
            Ok(())
        }
    }

    #[inline]
    pub fn update_checksums(&mut self) -> Result<(), PacketError> {
        self.update_ip_packet_checksum()?;
        self.update_tcp_packet_checksum()
    }

    #[inline]
    pub fn source_addr(&self) -> IpAddr {
        if let IpPacket::V4(ref packet) = self.packet {
            packet.source().into()
        } else {
            // TODO: Write actual IPv6 support
            Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0).into()
        }
    }

    #[inline]
    pub fn set_source_addr(&mut self, addr: IpAddr) -> Result<(), PacketError> {
        // TODO: Add IPv6 support
        if let IpPacket::V4(ref mut packet) = self.ip_packet_mut() {
            if let IpAddr::V4(addr) = addr {
                packet.set_source(addr)?;
            }
        }
        self.update_checksums()?;
        Ok(())
    }

    #[inline]
    pub fn destination_addr(&self) -> IpAddr {
        if let IpPacket::V4(ref packet) = self.packet {
            packet.destination().into()
        } else {
            // TODO: Write actual IPv6 support
            Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0).into()
        }
    }

    #[inline]
    pub fn set_destination_addr(&mut self, addr: IpAddr) -> Result<(), PacketError> {
        // TODO: Add IPv6 support
        let mut was_updated = false;
        if let IpPacket::V4(ref mut packet) = self.ip_packet_mut() {
            if let IpAddr::V4(addr) = addr {
                packet.set_destination(addr)?;
                was_updated = true;
            }
        }
        if was_updated {
            self.update_checksums()?;
        }
        Ok(())
    }

    #[inline]
    pub fn remote_addr(&self) -> IpAddr {
        if self.is_incoming {
            self.source_addr()
        } else {
            self.destination_addr()
        }
    }

    #[inline]
    pub fn payload(&self) -> &[u8] {
        let raw_pl_len = self.tcp_packet().payload().len();
        let tcp_pl_len = self.packet.payload().len();
        let tcp_h_len = tcp_pl_len - raw_pl_len;
        &self.packet.payload()[tcp_h_len..]
    }

    #[inline]
    pub fn has_payload(&self) -> bool {
        self.payload().len() > 0
    }

    #[inline]
    pub fn is_push(&self) -> bool {
        use packet::tcp::flag::PSH;
        self.tcp_packet().flags().intersects(PSH)
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

    fn new(buffer: Vec<u8>, is_incoming: bool, is_inner: bool) -> Result<Self, PacketError> {
        use packet::ErrorKind;
        let packet = IpPacket::new(buffer)?;
        if let IpPacket::V4(ref packet) = packet {
            if packet.protocol() != Protocol::Tcp {
                let err: PacketError = ErrorKind::Msg("received non-TCP packet".to_string()).into();
                Err(err)?;
            }
        }
        TcpPacket::new(packet.payload())?;
        Ok(Self {
            packet,
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
            packet: IpPacket::new(self.clone_packet())
                .expect("failed to clone valid packet from valid packet"),
        }
    }
}