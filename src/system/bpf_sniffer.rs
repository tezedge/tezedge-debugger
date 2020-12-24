use std::{convert::TryFrom, net::{IpAddr, SocketAddr}, collections::HashMap};
use tokio::stream::StreamExt;
use sniffer::{EventId, facade::{Module, SnifferEvent}};

use super::{SystemSettings, orchestrator::spawn_packet_orchestrator, raw_socket_producer::P2pPacket};

struct Connection {
    remote_address: SocketAddr,
    is_opening: bool,
}

impl Connection {
    fn regular(&mut self, write: bool, data: &[u8], local: SocketAddr, counter: u64) -> P2pPacket {
        let remote = self.remote_address.clone();
        let is_opening = self.is_opening;
        self.is_opening = false;
        P2pPacket {
            source_address: if write { local } else { remote },
            destination_address: if write { remote } else { local },
            is_closing: false,
            is_opening: is_opening,
            payload: data.to_vec(),
            counter: counter,
        }
    }

    fn closing(self, local: SocketAddr, counter: u64) -> P2pPacket {
        P2pPacket {
            source_address: local,
            destination_address: self.remote_address,
            is_closing: true,
            is_opening: self.is_opening,
            payload: vec![],
            counter: counter,
        }
    }
}

fn fake(settings: &SystemSettings, id: &EventId) -> SocketAddr {
    let port = 3 << 14 | ((id.pid & 0x7f) << 7) as u16 | (id.fd & 0x7f) as u16;
    SocketAddr::new(settings.local_address.clone(), port)
}

pub fn build_bpf_sniffing_system(settings: SystemSettings) {
    let orchestrator = spawn_packet_orchestrator(settings.clone());
    tokio::spawn(async move {
        let (module, _events) = Module::load();
        let mut events = module.main_buffer();
        let mut connections = HashMap::<EventId, Connection>::new();
        let mut counter = 1u64;
        while let Some(event) = events.next().await {
            let packet = match SnifferEvent::try_from(event.as_ref()) {
                Err(e) => {
                    tracing::error!("{:?}", e);
                    None
                },
                Ok(SnifferEvent::Write { id, data }) => {
                    connections.get_mut(&id)
                        .map(|c| c.regular(true, data, fake(&settings, &id), counter))
                },
                Ok(SnifferEvent::Read { id, data }) => {
                    connections.get_mut(&id)
                        .map(|c| c.regular(false, data, fake(&settings, &id), counter))
                },
                Ok(SnifferEvent::Connect { id, address }) => {
                    tracing::info!("P2P Connect {{ id: {:?}, address: {} }}", id, address);
                    if should_ignore(&settings, &address) {
                        module.ignore(id.fd);
                        tracing::info!("P2P Ignore {{ id: {:?}, address: {} }}", id, address);
                        None
                    } else {
                        let connection = Connection {
                            remote_address: address,
                            is_opening: true,
                        };
                        connections.insert(id.clone(), connection)
                            .map(|connection| connection.closing(fake(&settings, &id), counter))
                    }
                },
                Ok(SnifferEvent::LocalAddress { .. }) => {
                    None
                },
                Ok(SnifferEvent::Close { id }) => {
                    tracing::info!("Close {{ id: {:?} }}", id);
                    connections.remove(&id)
                        .map(|connection| connection.closing(fake(&settings, &id), counter))
                },
            };
            if let Some(packet) = packet {
                tracing::trace!("packet: {}", packet);
                counter += 1;
                match orchestrator.send(packet) {
                    Ok(_) => {
                        tracing::trace!("sent packet for processing");
                    }
                    Err(_) => {
                        tracing::error!("orchestrator channel closed abruptly");
                    }
                }
            }
        }
    });
}

fn should_ignore(settings: &SystemSettings, address: &SocketAddr) -> bool {
    match address.port() {
        0 | 65535 => {
            return true;
        },
        // dns
        53 => {
            return true;
        },
        // our
        p if p == settings.syslog_port || p == settings.rpc_port => {
            return true;
        },
        _ => (),
    }
    // lo v6
    if address.ip() == "::1".parse::<IpAddr>().unwrap() {
        return true;
    }
    // lo v4
    if address.ip() == "127.0.0.1".parse::<IpAddr>().unwrap() {
        return true;
    }

    return false;
}
