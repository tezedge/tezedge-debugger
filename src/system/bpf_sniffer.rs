// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{convert::TryFrom, net::{IpAddr, SocketAddr}, collections::HashMap};
use tokio::{stream::StreamExt, sync::mpsc};
use sniffer::{SocketId, EventId, Module, SnifferEvent};

use super::{SystemSettings, p2p, processor};
use crate::messages::p2p_message::{P2pMessage, SourceType};

pub struct BpfSniffer {
    module: Module,
    settings: SystemSettings,
    connections: HashMap<SocketId, mpsc::UnboundedSender<p2p::Message>>,
    counter: u64,
}

impl BpfSniffer {
    pub fn new(settings: &SystemSettings) -> Self {
        BpfSniffer {
            module: Module::load(),
            settings: settings.clone(),
            connections: HashMap::new(),
            counter: 1,
        }
    }

    pub async fn run(self) {
        let db = processor::spawn_processor(self.settings.clone());
        let mut events = self.module.main_buffer();
        let mut s = self;
        while let Some(slice) = events.next().await {
            match SnifferEvent::try_from(slice.as_ref()) {
                Err(error) => tracing::error!("{:?}", error),
                Ok(SnifferEvent::Connect { id, address }) => s.on_connect(id, address, db.clone(), false),
                // TODO:
                // Ok(SnifferEvent::Accept { id, address }) => s.on_connect(id, address, db.clone(), true),
                Ok(SnifferEvent::Close { id }) => s.on_close(id),
                Ok(SnifferEvent::Read { id, data }) => s.on_data(id, data.to_vec(), true),
                Ok(SnifferEvent::Write { id, data }) => s.on_data(id, data.to_vec(), false),
                // does not work, and not needed
                Ok(SnifferEvent::LocalAddress { .. }) => (),
                Ok(SnifferEvent::Debug { id, msg }) => tracing::warn!("{} {}", id, msg),
            }
        }
    }

    fn on_connect(&mut self, id: EventId, address: SocketAddr, db: mpsc::UnboundedSender<P2pMessage>, incoming: bool) {
        let should_ignore = ignore(&self.settings, &address);
        tracing::info!(
            address = tracing::field::debug(&address),
            id = tracing::field::display(&id),
            ignore = should_ignore,
            msg = "P2P New Outgoing",
        );
        if should_ignore {
            self.module.ignore(id.socket_id);
            return;
        }

        let (tx, rx) = mpsc::unbounded_channel();
        // drop old connection, it cause termination stream on the p2p parser,
        // so the p2p parser will know about it
        self.connections.insert(id.socket_id.clone(), tx);
        let parser = p2p::Parser {
            settings: self.settings.clone(),
            source_type: if incoming { SourceType::Remote } else { SourceType::Local },
            remote_address: address,
            id: id.socket_id.clone(),
            db: db,
        };
        tokio::spawn(async move { parser.run(rx).await });
    }
    
    fn on_close(&mut self, id: EventId) {
        tracing::info!(
            id = tracing::field::display(&id),
            msg = "P2P Close",
        );
        // can safely drop the old connection
        self.connections.remove(&id.socket_id);
    }
    
    fn on_data(&mut self, id: EventId, payload: Vec<u8>, incoming: bool) {
        match self.connections.get_mut(&id.socket_id) {
            Some(connection) => {
                let message = p2p::Message {
                    payload,
                    incoming,
                    counter: self.counter,
                    event_id: id.clone(),
                };
                match connection.send(message) {
                    Ok(()) => (),
                    Err(_) => {
                        tracing::error!(
                            id = tracing::field::display(&id),
                            incoming = incoming,
                            msg = "P2P Failed to forward message to the p2p parser",
                        )
                    },
                }
            },
            None => {
                // It is possible due to race condition,
                // when we consider to ignore connection, we do not create
                // connection structure in userspace, and send to bpf code 'ignore' command.
                // However, the bpf code might already sent us some message.
                // It is safe to ignore this message if it goes right after appearing
                // new P2P connection which we ignore.
                tracing::warn!(
                    id = tracing::field::display(&id),
                    msg = "P2P receive message for absent connection",
                )
            },
        }
    }
}

fn ignore(settings: &SystemSettings, address: &SocketAddr) -> bool {
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
