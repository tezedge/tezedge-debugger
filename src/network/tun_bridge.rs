use tun::{
    self, Configuration, Device as _,
    platform::Device,
};
use std::io::Read as _;
use failure::{Error, Fail};
use flume::{Receiver, Sender, unbounded};
use pnet::packet::ipv4::Ipv4Packet;

pub fn make_bridge(
    (in_addr, in_mask): (IpAddrTuple, IpAddrTuple),
    (out_addr, out_mask): (IpAddrTuple, IpAddrTuple),
) -> Result<(Sender<Packet>, Receiver<Packet>), Error> {
    let mut in_conf = Configuration::default();
    let mut out_conf = Configuration::default();

    in_conf.address(in_addr)
        .platform(|config| {
            config.packet_information(false);
        })
        .netmask(in_mask)
        .name("tintun")
        .up();

    out_conf.address(out_addr)
        .platform(|config| {
            config.packet_information(false);
        })
        .netmask(out_mask)
        .name("toutun")
        .up();

    let in_tun = tun::platform::create(&in_conf)
        .map_err(BridgeError::from)?;
    let out_tun = tun::platform::create(&out_conf)
        .map_err(BridgeError::from)?;

    let (tx, rx) = unbounded();
    let in_send = tx.clone();
    let out_send = tx.clone();

    std::thread::spawn(process_packets(in_tun, in_send));
    std::thread::spawn(process_packets(out_tun, out_send));

    Ok((tx, rx))
}

fn process_packets(mut dev: Device, sender: Sender<Packet>) -> impl FnMut() + 'static {
    move || {
        let mut buf = Vec::with_capacity(65535);
        loop {
            let count = dev.read(&mut buf).unwrap();
            if let Err(err) = sender.send(Ipv4Packet::owned((&buf[0..count]).to_vec()).unwrap()) {
                log::error!("Failed to send message from on TUN {}: {:?}", dev.name(), err);
            }
        }
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
pub type Packet = Ipv4Packet<'static>;