#![allow(dead_code)]

mod remote_client;
mod network_message;
mod identity;
mod msg_decoder;

use std::collections::HashMap;

use log::info;
use failure::Error;
use pnet::{
    packet::{
        Packet,
        udp::UdpPacket,
        ipv4::Ipv4Packet,
        ethernet::{EthernetPacket, EtherTypes},
    },
    datalink,
};
use std::sync::{Arc, RwLock};
use lazy_static::lazy_static;

use crate::{
    remote_client::RemoteClient,
    identity::{load_identity, Identity},
};
use crate::network_message::NetworkMessage;

type Remotes = HashMap<u16, Arc<RwLock<RemoteClient>>>;

lazy_static! {
    pub static ref IDENTITY: Identity = load_identity("./identity/identity.json").expect("failed to load identity");
}

fn main() -> Result<(), Error> {
    simple_logger::init().expect("failed to initialize logger");
    let interface = datalink::interfaces().into_iter()
        .filter(|x| x.is_up() && x.is_broadcast() && x.is_multicast())
        .next()
        .unwrap();

    let local_port = 9732; // Add as CLI command.
    let mut remotes: Remotes = Default::default();

    let (_tx, mut rx) = datalink::channel(&interface, Default::default())
        .map(|chan| match chan {
            datalink::Channel::Ethernet(tx, rx) => (tx, rx),
            _ => panic!("Unsupported channel type"),
        })?;

    info!("Started sniffing on port {}", local_port);

    loop {
        // Ethernet Packet == MAC Address
        let packet = EthernetPacket::new(rx.next()?).unwrap();

        if packet.get_ethertype() == EtherTypes::Ipv4 {
            // IPv4 == IP Address
            let ipv4 = Ipv4Packet::new(packet.payload()).unwrap();
            let udp = UdpPacket::new(ipv4.payload()).unwrap();
            let source_port = udp.get_source();
            let dest_port = udp.get_destination();
            if local_port == source_port {
                process_msg(dest_port, &mut remotes, udp.payload(), true);
            } else if local_port == dest_port {
                process_msg(source_port, &mut remotes, udp.payload(), false);
            } else {
                continue;
            }
        }
    }
}

#[inline]
fn process_msg(remote: u16, remotes: &mut Remotes, payload: &[u8], incoming: bool) {
    if !remotes.contains_key(&remote) {
        // Spawn new remote client handler
        remotes.insert(remote, RemoteClient::spawn(remote));
    }
    let val = remotes.get(&remote).expect("Client dropped prematurely");
    let mut lock = val.write().expect("Lock poisoning");
    lock.send_message(if incoming {
        NetworkMessage::incoming(payload)
    } else {
        NetworkMessage::outgoing(payload)
    });
}


#[inline]
/// Try to create a nonce-pair from *guessed* nonce messages.
fn process_nonces(remote: u16, out_msg: &[u8], inc_msg: &[u8], incoming: bool) {
    use crypto::nonce::generate_nonces;
    info!("Received nonces for {}: {:?} | {:?}", remote, out_msg, inc_msg);
    let nonces = generate_nonces(out_msg, inc_msg, incoming);
    info!("Local nonce: {:?} | Remote nonce: {:?}", nonces.local, nonces.remote);
}