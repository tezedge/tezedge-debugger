use tun::{
    self, Configuration,
    platform::posix::{Reader, Writer},
};
use std::{
    io::{Read, Write},
    process::Command,
    net::Ipv4Addr,
};
use failure::{Error, Fail};
use flume::{Receiver, Sender, unbounded};
use pnet::packet::{
    Packet as PacketTrait,
    MutablePacket as MutablePacketTrait,
    ipv4::MutableIpv4Packet,
};
use crate::actors::packet_orchestrator::{OrchestratorMessage, Packet};
use packet::PacketMut;

fn create_tun_device(device: &str) {
    Command::new("ip")
        .args(&["tuntap", "add", "mode", "tun", "name", device])
        .output().unwrap();
}

fn setup_tun_device(device: &str, ip: &str) {
    Command::new("ip")
        .args(&["link", "set", device, "up"])
        .output().unwrap();
    Command::new("ip")
        .args(&["addr", "add", ip, "dev", device])
        .output().unwrap();
}

pub fn make_bridge(in_addr_space: &str, out_addr_space: &str, in_addr: &str, out_addr: &str) -> Result<((Sender<OrchestratorMessage>, Receiver<OrchestratorMessage>), BridgeWriter), Error> {
    create_tun_device("tun0");
    create_tun_device("tun1");

    setup_tun_device("tun0", in_addr_space);
    setup_tun_device("tun1", out_addr_space);

    let mut in_conf = Configuration::default();
    let mut out_conf = Configuration::default();
    in_conf.name("tun0");
    out_conf.name("tun1");

    let (tx, rx) = unbounded();
    let in_send = tx.clone();
    let out_send = tx.clone();

    let (in_reader, in_writer) = tun::platform::create(&in_conf)
        .map_err(BridgeError::from)?
        .split();
    let (out_reader, out_writer) = tun::platform::create(&out_conf)
        .map_err(BridgeError::from)?
        .split();

    std::thread::spawn(process_packets(in_reader, in_send, true, in_addr.parse()?));
    std::thread::spawn(process_packets(out_reader, out_send, false, out_addr.parse()?));

    Ok(((tx, rx), BridgeWriter::new(in_writer, out_writer)))
}

fn process_packets(mut dev: Reader, sender: Sender<OrchestratorMessage>, inner: bool, addr: Ipv4Addr) -> impl FnMut() + 'static {
    move || {
        let mut buf = [0u8; 65535];
        loop {
            let count = dev.read(&mut buf).unwrap();
            let data = &buf[0..count];
            let header = data[0];
            let version = header >> 4;
            if version == 4 {
                if let Some(ip_header) = MutableIpv4Packet::owned((&buf[0..count]).to_vec()) {
                    if let Err(err) = sender.send(if inner {
                        OrchestratorMessage::Inner(
                            if ip_header.get_source() == addr {
                                Packet::outgoing
                            } else {
                                Packet::incoming
                            }(ip_header)
                        )
                    } else {
                        OrchestratorMessage::Outer(
                            if ip_header.get_destination() == addr {
                                Packet::incoming
                            } else {
                                Packet::outgoing
                            }(ip_header)
                        )
                    }) {
                        log::error!("failed to forward message: {:?}", err);
                    }
                } else {
                    log::warn!("Got invalid ip message: {:?}", &buf[0..count]);
                }
            }
        }
    }
}

/// Writing part of the bridge, to send forward captured packets
pub struct BridgeWriter {
    /// "Inwards" writer, for forwarding to local clients
    in_writer: Writer,
    /// "Outwards" writer, for forwarding to remote clients
    out_writer: Writer,
}

impl BridgeWriter {
    pub fn new(in_writer: Writer, out_writer: Writer) -> Self {
        Self { in_writer, out_writer }
    }

    pub fn send_packet_to_internet(&mut self, packet: &mut MutableIpv4Packet, addr: &str) -> Result<(), Error> {
        packet.set_source(addr.parse()?);
        Self::recalculate_checksums(packet);
        Ok(self.out_writer.write_all(packet.packet())?)
    }

    pub fn send_packet_to_local(&mut self, packet: &mut MutableIpv4Packet, addr: &str) -> Result<(), Error> {
        packet.set_destination(addr.parse()?);
        Self::recalculate_checksums(packet);
        Ok(self.in_writer.write_all(packet.packet())?)
    }

    fn recalculate_checksums(packet: &mut MutableIpv4Packet) {
        use packet::{
            tcp::Packet as TcpPacket,
            ip::{
                Packet as IPPacket,
                v4::Packet as IPv4Packet,
            },
        };


        let packet: &mut [u8] = packet.packet_mut();
        let mut ip_packet = IPv4Packet::new(packet).unwrap();
        let _ = ip_packet.update_checksum();
        let (header, payload) = ip_packet.split_mut();
        let ip_header = IPPacket::V4(IPv4Packet::no_payload(header).unwrap());
        let mut tcp_packet = TcpPacket::new(payload).unwrap();
        let _ = tcp_packet.update_checksum(&ip_header);
    }
}


#[derive(Debug, Fail)]
pub enum BridgeError {
    #[fail(display = "failed to create bridge device: {}", _0)]
    CreateDeviceError(tun::ErrorKind),
}

impl From<tun::Error> for BridgeError {
    fn from(err: tun::Error) -> Self {
        Self::CreateDeviceError(err.0)
    }
}

pub type IpAddrTuple = (u8, u8, u8, u8);