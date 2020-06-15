use tokio::sync::mpsc::{
    UnboundedSender, unbounded_channel,
};
use tracing::{trace, info, error};
use std::{
    process::exit,
    collections::{HashMap, hash_map::Entry},
};
use crate::{
    system::prelude::*,
    messages::tcp_packet::Packet,
};

pub fn spawn_packet_orchestrator() -> UnboundedSender<Packet> {
    let (sender, mut receiver) = unbounded_channel::<Packet>();

    tokio::spawn(async move {
        let mut packet_processors = HashMap::new();
        loop {
            if let Some(packet) = receiver.recv().await {
                let entry = packet_processors.entry(packet.identification_pair());
                let mut occupied_entry;
                let processor;

                // Packet is closing connection
                if packet.is_closing() {
                    if let Entry::Occupied(entry) = entry {
                        // There is still running processor, this packet will notify it to shut down
                        occupied_entry = entry.remove();
                        processor = &mut occupied_entry;
                    } else {
                        // Processor is already shut down, ignore the packet
                        continue;
                    }
                } else {
                    // Is packet for processing
                    let addr = packet.source_addr();
                    processor = entry.or_insert_with(move || {
                        // If processor does not exists, create new one
                        info!(addr = display(addr), "spawning p2p parser");
                        spawn_p2p_parser(addr)
                    });
                };

                match processor.send(packet) {
                    Ok(()) => {
                        trace!("sent packet to p2p");
                    }
                    Err(_) => {
                        error!("p2p parser channel closed abruptly");
                    }
                }
            } else {
                error!("packet consuming channel closed unexpectedly");
                exit(-1);
            }
        }
    });

    return sender;
}