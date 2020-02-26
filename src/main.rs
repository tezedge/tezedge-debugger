#![allow(dead_code)]

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

fn main() -> Result<(), Error> {
    simple_logger::init().expect("failed to initialize logger");
    let interface = datalink::interfaces().into_iter()
        .filter(|x| x.is_up() && x.is_broadcast() && x.is_multicast())
        .next()
        .unwrap();

    let local_port = 9732; // Add as CLI command.
    let mut node_messages: HashMap<u16, (bool, bool)> = Default::default();

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
                process_outgoing(dest_port, &mut node_messages, udp.payload());
            } else if local_port == dest_port {
                process_incoming(source_port, &mut node_messages, udp.payload());
            } else {
                continue;
            }
        }
    }
}

// We need to catch first messages between two nodes, to reconstruct nonces and precomputed key

fn process_outgoing(recipient: u16, msgs: &mut HashMap<u16, (bool, bool)>, payload: &[u8]) {
    match msgs.get(&recipient) {
        None | Some((false, _)) => {
            info!("Sending message({})\n{:?}\n", recipient, payload);
            msgs.insert(recipient, (true, false));
        }
        _ => return,
    }
}

fn process_incoming(sender: u16, msgs: &mut HashMap<u16, (bool, bool)>, payload: &[u8]) {
    match msgs.get(&sender) {
        None | Some((_, false)) => {
            info!("Got first message from {}\n{:?}\n", sender, payload);
            msgs.insert(sender, (false, true));
        }
        _ => return,
    }
}