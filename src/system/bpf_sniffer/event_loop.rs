// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    convert::TryFrom,
    net::{IpAddr, SocketAddr},
    collections::HashMap,
    path::Path,
};
use tezos_conversation::Identity;
use tokio::{stream::StreamExt, sync::mpsc};
use sniffer::{SocketId, EventId, Module, SnifferEvent};

use super::{SystemSettings, connection::Connection, p2p, processor};
use crate::messages::p2p_message::{P2pMessage, SourceType};

pub struct EventProcessor {
    module: Module,
    settings: SystemSettings,
    connections: HashMap<SocketId, Connection>,
    counter: u64,
    identity: Option<Identity>,
    node_pid: Option<u32>,
}

impl EventProcessor {
    pub fn new(module: Module, settings: &SystemSettings) -> Self {
        EventProcessor {
            module,
            settings: settings.clone(),
            connections: HashMap::new(),
            counter: 0,
            identity: None,
            node_pid: None,
        }
    }

    pub async fn run(self) {
        let db = processor::spawn_processor(self.settings.clone());
        let mut s = self;
        let mut events = s.module.main_buffer();
        while let Some(slice) = events.next().await {
            match SnifferEvent::try_from(slice.as_ref()) {
                Err(error) => tracing::error!("{:?}", error),
                Ok(SnifferEvent::Connect { id, address }) => {
                    tracing::info!(
                        id = tracing::field::display(&id),
                        address = tracing::field::display(&address),
                        msg = "Syscall Connect",
                    );
                    s.on_connect(id, address, db.clone(), None)
                },
                Ok(SnifferEvent::Bind { id, address }) => {
                    tracing::info!(
                        id = tracing::field::display(&id),
                        address = tracing::field::display(&address),
                        msg = "Syscall Bind",
                    );
                    if address.ip().is_unspecified() && address.port() == s.settings.node_p2p_port {
                        s.node_pid = Some(id.socket_id.pid);
                    }
                },
                Ok(SnifferEvent::Listen { id }) => {
                    tracing::info!(
                        id = tracing::field::display(&id),
                        msg = "Syscall Listen",
                    );
                },
                Ok(SnifferEvent::Accept { id, listen_on_fd, address }) => {
                    tracing::info!(
                        id = tracing::field::display(&id),
                        listen_on_fd = tracing::field::display(&listen_on_fd),
                        address = tracing::field::display(&address),
                        msg = "Syscall Accept",
                    );
                    s.on_connect(id, address, db.clone(), Some(listen_on_fd))
                },
                Ok(SnifferEvent::Close { id }) => {
                    tracing::info!(
                        id = tracing::field::display(&id),
                        msg = "Syscall Close",
                    );
                    s.on_close(id)
                },
                Ok(SnifferEvent::Read { id, data }) => {
                    s.on_data(id, data.to_vec(), true)
                },
                Ok(SnifferEvent::Write { id, data }) => {
                    s.on_data(id, data.to_vec(), false)
                },
                Ok(SnifferEvent::Debug { id, msg }) => tracing::warn!("{} {}", id, msg),
            }    
        }
    }

    fn should_ignore(&self, address: &SocketAddr) -> bool {
        match address.port() {
            0 | 65535 => {
                return true;
            },
            // dns and other well known not tezos
            53 | 80 | 443 | 22 => {
                return true;
            },
            // our
            p if p == self.settings.syslog_port || p == self.settings.rpc_port => {
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

    fn on_connect(
        &mut self,
        id: EventId,
        address: SocketAddr,
        db: mpsc::UnboundedSender<P2pMessage>,
        listened_on: Option<u32>,
    ) {
        /// Try to load identity lazy from one of the well defined paths
        fn load_identity() -> Result<Identity, ()> {
            let identity_paths = [
                "/tmp/volume/identity.json".to_string(),
                "/tmp/volume/data/identity.json".to_string(),
                format!("{}/.tezos-node/identity.json", std::env::var("HOME").unwrap()),
            ];

            for path in &identity_paths {
                if !Path::new(path).is_file() {
                    continue;
                }
                match Identity::from_path(path.clone()) {
                    Ok(identity) => {
                        tracing::info!(file_path = tracing::field::display(&path), "loaded identity");
                        return Ok(identity);
                    },
                    Err(err) => {
                        tracing::warn!(error = tracing::field::display(&err), "identity file does not contains valid identity");
                    },
                }
            }

            Err(())
        }

        // the message is not belong to the node or not p2p
        if Some(id.socket_id.pid) != self.node_pid {
            tracing::info!(id = tracing::field::display(&id), msg = "ignore, filtered by pid");
            self.module.ignore(id.socket_id);
            return;
        }

        if self.should_ignore(&address) {
            tracing::info!(id = tracing::field::display(&id), msg = "ignore");
            self.module.ignore(id.socket_id);
            return;
        }

        let identity = match &self.identity {
            Some(identity) => identity.clone(),
            None => {
                match load_identity() {
                    Ok(identity) => {
                        tracing::info!(public_key = tracing::field::display(hex::encode(&identity.public_key())), "loaded identity");
                        self.identity = Some(identity.clone());
                        identity
                    },
                    Err(()) => {
                        tracing::warn!("ignore connection because no identity");
                        self.module.ignore(id.socket_id);
                        return;
                    },
                }
            }
        };

        let source_type = if listened_on.is_some() { SourceType::Remote } else { SourceType::Local };
        let (tx, rx) = mpsc::unbounded_channel();
        let connection = Connection::new(tx, source_type.clone(), address.clone());
        // drop old connection, it cause termination stream on the p2p parser,
        // so the p2p parser will know about it
        self.connections.insert(id.socket_id.clone(), connection);
        let parser = p2p::Parser {
            identity,
            settings: self.settings.clone(),
            source_type,
            remote_address: address,
            id: id.socket_id.clone(),
            db: db,
        };
        let (debug_tx, _debug_rx) = mpsc::channel(0x100);
        tokio::spawn(parser.run(rx, debug_tx));
    }

    fn on_close(&mut self, id: EventId) {
        // can safely drop the old connection
        self.connections.remove(&id.socket_id);
    }

    fn on_data(&mut self, id: EventId, payload: Vec<u8>, incoming: bool) {
        self.counter += 1;
        match self.connections.get_mut(&id.socket_id) {
            Some(connection) => {
                let message = p2p::Message {
                    payload,
                    incoming,
                    counter: self.counter,
                    event_id: id,
                };
                connection.process(message)
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
