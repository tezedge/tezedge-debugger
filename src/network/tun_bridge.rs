use tun::{
    self, Configuration,
    platform::posix::{Reader, Writer},
};
use std::{
    io::{Read, Write},
    process::Command,
    net::IpAddr,
};
use failure::{Error, Fail};
use flume::{Receiver, Sender, unbounded};
use crate::actors::prelude::*;

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

pub fn make_bridge(in_addr_space: &str, out_addr_space: &str, local_addr: IpAddr, remote_addr: IpAddr) -> Result<((Sender<RawPacketMessage>, Receiver<RawPacketMessage>), BridgeWriter), Error> {
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

    std::thread::spawn(process_packets(in_reader, in_send, true, local_addr, remote_addr));
    std::thread::spawn(process_packets(out_reader, out_send, false, local_addr, remote_addr));

    Ok(((tx, rx), BridgeWriter::new(in_writer, out_writer)))
}

fn process_packets(mut dev: Reader, sender: Sender<RawPacketMessage>, inner: bool, local_addr: IpAddr, tun_addr: IpAddr) -> impl FnMut() + 'static {
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
                        if let Err(err) = sender.send(msg) {
                            log::error!("failed to forward message: {:?}", err);
                        }
                    }
                    Err(err) => {
                        log::trace!("dropping invalid packet {:?}: {}", data, err);
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