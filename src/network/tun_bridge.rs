use tun::{
    self, Configuration,
    platform::posix::{Reader, Writer},
};
use std::io::{Read, Write};
use failure::{Error, Fail};
use flume::{Receiver, Sender, unbounded};
use pnet::packet::{
    Packet as PacketTrait,
    ipv4::MutableIpv4Packet,
};
use crate::actors::packet_orchestrator::{OrchestratorMessage, Packet};
use std::net::Ipv4Addr;

pub fn make_bridge(
    (in_addr, in_mask): (IpAddrTuple, IpAddrTuple),
    (out_addr, out_mask): (IpAddrTuple, IpAddrTuple),
) -> Result<((Sender<OrchestratorMessage>, Receiver<OrchestratorMessage>), BridgeWriter), Error> {
    let mut in_conf = Configuration::default();
    let mut out_conf = Configuration::default();

    in_conf.address(in_addr)
        .platform(|config| {
            config.packet_information(false);
        })
        .netmask(in_mask)
        .up();

    out_conf.address(out_addr)
        .platform(|config| {
            config.packet_information(false);
        })
        .netmask(out_mask)
        .up();

    let (tx, rx) = unbounded();
    let in_send = tx.clone();
    let out_send = tx.clone();

    let (in_reader, in_writer) = tun::platform::create(&in_conf)
        .map_err(BridgeError::from)?
        .split();
    let (out_reader, out_writer) = tun::platform::create(&out_conf)
        .map_err(BridgeError::from)?
        .split();

    std::thread::spawn(process_packets(in_reader, in_send, false));
    std::thread::spawn(process_packets(out_reader, out_send, true));

    Ok(((tx, rx), BridgeWriter::new(in_writer, out_writer)))
}

fn process_packets(mut dev: Reader, sender: Sender<OrchestratorMessage>, inner: bool) -> impl FnMut() + 'static {
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
                            if ip_header.get_source() == Ipv4Addr::new(10, 0, 0, 1) {
                                Packet::outgoing
                            } else {
                                Packet::incoming
                            }(ip_header)
                        )
                    } else {
                        OrchestratorMessage::Outer(
                            if ip_header.get_destination() == Ipv4Addr::new(10, 0, 1, 1) {
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

    pub fn send_packet_to_internet(&mut self, packet: &mut MutableIpv4Packet) -> Result<(), Error> {
        // TODO: Implement address handling
        packet.set_source([10, 0, 1, 1].into());
        Ok(self.out_writer.write_all(packet.packet())?)
    }

    pub fn send_packet_to_local(&mut self, packet: &mut MutableIpv4Packet) -> Result<(), Error> {
        // TODO: Implement address handling
        packet.set_destination([192, 168, 1, 199].into());
        Ok(self.in_writer.write_all(packet.packet())?)
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