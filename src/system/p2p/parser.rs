use std::{
    path::Path,
    net::SocketAddr,
    collections::HashMap,
};
use tokio::{sync::mpsc, task::JoinHandle};
use tezos_conversation::Identity;
use sniffer::{EventId, SocketId};

use crate::{
    messages::p2p_message::{SourceType, P2pMessage},
    system::SystemSettings,
};
use super::{
    connection::Connection,
    connection_parser,
    report::{Report, ConnectionReport},
};

pub struct Message {
    pub payload: Vec<u8>,
    pub incoming: bool,
    pub counter: u64,
    pub event_id: EventId,
}

#[derive(Debug)]
pub struct Command;

pub struct ProcessingConnectionResult {
    pub have_identity: bool,
}

pub struct Parser {
    identity_cache: Option<Identity>,
    tx_report: mpsc::Sender<Report>,
    connections: HashMap<SocketId, ConnectionState>,
}

enum ConnectionState {
    Running(Connection, JoinHandle<ConnectionReport>),
    Closed(ConnectionReport),
}

impl Parser {
    pub fn new(tx_report: mpsc::Sender<Report>) -> Self {
        Parser {
            identity_cache: None,
            tx_report,
            connections: HashMap::new(),
        }
    }

    pub async fn execute(&mut self, command: Command) {
        match command {
            Command => {
                let report = self.prepare_report().await;
                match self.tx_report.send(report).await {
                    Ok(()) => (),
                    Err(_) => (),
                }
            },
        }
    }

    async fn prepare_report(&mut self) -> Report {
        for (_, connection_state) in &mut self.connections {
            match connection_state {
                ConnectionState::Running(connection, _) => connection.send_command(Command),
                ConnectionState::Closed(_) => (),
            }
        }

        let mut closed_connections = Vec::new();
        let mut working_connections = Vec::new();
        for (_, connection_state) in &mut self.connections {
            match connection_state {
                ConnectionState::Running(connection, _) => {
                    if let Some(report) = connection.receive_report().await {
                        working_connections.push(report);
                    }
                },
                ConnectionState::Closed(report) => closed_connections.push(report.clone()),
            }
        }

        Report::prepare(closed_connections, working_connections)
    }

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

    fn try_load_identity(&mut self) -> Option<Identity> {
        if self.identity_cache.is_none() {
            self.identity_cache = Self::load_identity().ok();
        }
        self.identity_cache.clone()
    }

    pub fn process_connect(
        &mut self,
        settings: &SystemSettings,
        id: EventId,
        remote_address: SocketAddr,
        db: &mpsc::UnboundedSender<P2pMessage>,
        source_type: SourceType,
    ) -> ProcessingConnectionResult {
        let have_identity = if let Some(identity) = self.try_load_identity() {
            let (tx, rx) = mpsc::unbounded_channel();
            let parser = connection_parser::Parser {
                identity,
                settings: settings.clone(),
                source_type,
                remote_address,
                id: id.socket_id.clone(),
                db: db.clone(),
            };
            let (tx_report, rx_report) = mpsc::channel(0x100);
            let connection = Connection::new(tx, rx_report, source_type, remote_address);
            let handle = tokio::spawn(parser.run(rx, tx_report));
            self.connections.insert(id.socket_id, ConnectionState::Running(connection, handle));
            true
        } else {
            false
        };
        ProcessingConnectionResult { have_identity }
    }

    pub async fn process_close(&mut self, event_id: EventId) {
        // can safely drop the old connection
        match self.connections.remove(&event_id.socket_id) {
            Some(connection_state) => {
                match connection_state {
                    ConnectionState::Running(connection, handle) => {
                        drop(connection);
                        match handle.await {
                            Ok(report) => {
                                self.connections.insert(event_id.socket_id.clone(), ConnectionState::Closed(report));
                            },
                            Err(error) => {
                                tracing::error!(
                                    id = tracing::field::display(&event_id.socket_id),
                                    error = tracing::field::display(&error),
                                    msg = "P2P failed to join task which was processing the connection",
                                )
                            }
                        }
                    },
                    ConnectionState::Closed(report) => {
                        tracing::warn!(
                            id = tracing::field::display(&event_id.socket_id),
                            msg = "P2P try to close already closed connection",
                        );
                        self.connections.insert(event_id.socket_id.clone(), ConnectionState::Closed(report));
                    }
                }
            },
            None => {
                tracing::warn!(
                    id = tracing::field::display(&event_id.socket_id),
                    msg = "P2P try to close absent connection",
                )
            }
        }
    }

    pub fn process_data(&mut self, message: Message) {
        match self.connections.get_mut(&message.event_id.socket_id) {
            Some(connection_state) => {
                match connection_state {
                    ConnectionState::Running(connection, _) => connection.process(message),
                    ConnectionState::Closed(_) => {
                        tracing::warn!(
                            id = tracing::field::display(&message.event_id),
                            msg = "P2P receive message for already closed connection",
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
                    id = tracing::field::display(&message.event_id),
                    msg = "P2P receive message for absent connection",
                )
            },
        }
    }
}
