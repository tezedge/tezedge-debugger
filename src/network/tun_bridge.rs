use tun::{
    Device,
    self, Configuration,
    platform::posix::{Reader, Writer},
};
use std::{
    io::{Read, Write},
    // process::Command,
    net::IpAddr,
};
use failure::{Error, Fail};
use flume::{Receiver, Sender, unbounded};
use crate::actors::prelude::*;
use crate::network::health_checks::{device_address_check, internet_accessibility_check};
use failure::_core::time::Duration;

pub fn make_bridge(_in_addr_space: &str, _out_addr_space: &str,
                   in_addr: &str, out_addr: &str,
                   local_addr: IpAddr, remote_addr: IpAddr) -> Result<((Sender<SenderMessage>, Receiver<SenderMessage>), BridgeWriter), Error>
{
    let mut in_conf = Configuration::default();
    let mut out_conf = Configuration::default();
    in_conf.name("tun0");
    out_conf.name("tun1");

    let (tx, rx) = unbounded();
    let in_send = tx.clone();
    let out_send = tx.clone();

    let mut in_dev = tun::platform::create(&in_conf)
        .map_err(BridgeError::from)?;
    let mut out_dev = tun::platform::create(&out_conf)
        .map_err(BridgeError::from)?;

    // Run health-checks -- retrying
    loop {
        match internet_accessibility_check(&mut out_dev, out_addr) {
            Ok(_) => {
                log::info!("Internet access set correctly");
            }
            Err(err) => {
                log::info!("Internet unreachable: {}", err);
                std::thread::sleep(Duration::from_secs(3));
                continue;
            }
        };

        break;
    }

    let (in_reader, in_writer) = in_dev.split();
    let (out_reader, out_writer) = out_dev.split();

    std::thread::spawn(process_packets(in_reader, in_send, true, local_addr, remote_addr));
    std::thread::spawn(process_packets(out_reader, out_send, false, local_addr, remote_addr));

    Ok(((tx, rx), BridgeWriter::new(in_writer, out_writer)))
}

fn process_packets(mut dev: Reader, sender: Sender<SenderMessage>, inner: bool, local_addr: IpAddr, tun_addr: IpAddr) -> impl FnMut() + 'static {
    move || {
        let mut buf = [0u8; 65535];
        loop {
            let count = dev.read(&mut buf).unwrap();
            let data = &buf[0..count];
            let header = data[0];
            let version = header >> 4;
            if version == 4 {
                match RawPacketMessage::partial(data) {
                    Ok(mut msg) => {
                        msg.set_is_inner(inner);
                        if inner {
                            // if message is inner, it is incoming, iff dest addr == local_addr
                            msg.set_is_incoming(msg.destination_addr() == local_addr);
                        } else {
                            // message is incoming, iff source addr != tun_addr
                            msg.set_is_incoming(msg.destination_addr() == tun_addr);
                        }
                        if let Err(err) = sender.send(SenderMessage::Process(msg)) {
                            log::error!("failed to process message: {:?}", err);
                        }
                    }
                    Err(_) => {
                        if let Err(err) = sender.send(SenderMessage::Forward(inner, data.to_vec())) {
                            log::error!("failed to forward message: {:?}", err);
                        }
                    }
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

    pub fn send_packet_to_internet(&mut self, mut packet: RawPacketMessage, addr: IpAddr) -> Result<(), Error> {
        // TODO: CREATE ERROR FOR THIS
        packet.set_source_addr(addr)
            .expect("failed to set source address");
        self.out_writer.write_all(&packet.clone_packet())
            .expect("failed to write data");
        Ok(())
    }

    pub fn send_packet_to_local(&mut self, mut packet: RawPacketMessage, addr: IpAddr) -> Result<(), Error> {
        // TODO: CREATE ERROR FOR THIS
        packet.set_destination_addr(addr)
            .expect("failed to set destination address");
        self.in_writer.write_all(&packet.clone_packet())
            .expect("failed to write data");
        Ok(())
    }

    pub fn forward_to_internet(&mut self, packet: &[u8]) -> Result<(), Error> {
        self.out_writer.write_all(packet)
            .expect("failed to write data");
        Ok(())
    }

    pub fn forward_to_local(&mut self, packet: &[u8]) -> Result<(), Error> {
        self.in_writer.write_all(packet)
            .expect("failed to write data");
        Ok(())
    }
}


#[derive(Debug, Fail)]
pub enum BridgeError {
    #[fail(display = "failed to create bridge device: {}", _0)]
    CreateDeviceError(tun::Error),
}

impl From<tun::Error> for BridgeError {
    fn from(err: tun::Error) -> Self {
        Self::CreateDeviceError(err)
    }
}

pub type IpAddrTuple = (u8, u8, u8, u8);