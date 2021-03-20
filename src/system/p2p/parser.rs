use std::{
    path::Path,
    net::SocketAddr,
    collections::HashMap,
    time::Duration,
};
use tokio::{sync::mpsc, time};
use tezos_conversation::Identity;
use bpf_common::{EventId, SocketId};

use crate::{
    storage_::{StoreClient, p2p::Message as P2pMessage, indices::Initiator},
    system::NodeConfig,
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

#[derive(Debug, Clone, Copy)]
pub enum Command {
    GetReport,
    Terminate,
}

pub struct ProcessingConnectionResult {
    pub have_identity: bool,
}

pub struct Parser {
    identity_cache: Option<Identity>,
    tx_report: mpsc::Sender<Report>,
    rx_connection_report: mpsc::Receiver<ConnectionReport>,
    tx_connection_report: mpsc::Sender<ConnectionReport>,
    working_connections: HashMap<SocketId, Connection>,
    closed_connections: Vec<ConnectionReport>,
}

impl Parser {
    pub fn new(tx_report: mpsc::Sender<Report>) -> Self {
        let (tx_connection_report, rx_connection_report) = mpsc::channel(0x1000);
        Parser {
            identity_cache: None,
            tx_report,
            rx_connection_report,
            tx_connection_report,
            working_connections: HashMap::new(),
            closed_connections: Vec::new(),
        }
    }

    pub async fn execute(&mut self, command: Command) {
        let mut num = 0;
        for (_, c) in &mut self.working_connections {
            c.send_command(command).await;
            num += 1;
        }

        match command {
            Command::GetReport => {
                let mut working_connections = Vec::with_capacity(num);
                let get_reports = async {
                    for _ in 0..num {
                        if let Some(report) = self.rx_connection_report.recv().await {
                            working_connections.push(report);
                        } else {
                            tracing::error!("failed to receive reports from all parsers");
                            break;
                        }
                    }
                };
                match time::timeout(Duration::from_millis(1_000), get_reports).await {
                    Ok(()) => (),
                    Err(_) => tracing::error!("failed to receive reports from all parsers, timeout"),
                }
                let mut closed_connections = self.closed_connections.clone();
                closed_connections.iter_mut().for_each(|report| report.metadata = None);
        
                let report = Report::prepare(closed_connections, working_connections);
                match self.tx_report.send(report).await {
                    Ok(()) => (),
                    Err(_) => (),
                }
            },
            Command::Terminate => (),
        }
    }

    pub async fn terminate(mut self) {
        // TODO: this is the final report, compare it with ocaml report
        for (_, c) in self.working_connections {
            if let Some(report) = c.join().await {
                self.closed_connections.push(report)
            }
        }
        let report = Report::prepare(self.closed_connections, Vec::new());
        match self.tx_report.send(report).await {
            Ok(()) => (),
            Err(_) => (),
        }
    }

    /// Try to load identity lazy from one of the well defined paths
    fn load_identity(path: &str) -> Result<Identity, ()> {
        if !Path::new(path).is_file() {
            return Err(());
        }
        match Identity::from_path(path.to_string()) {
            Ok(identity) => {
                tracing::info!(file_path = tracing::field::display(&path), "loaded identity");
                return Ok(identity);
            },
            Err(err) => {
                tracing::warn!(error = tracing::field::display(&err), "identity file does not contains valid identity");
                Err(())
            },
        }
    }

    fn try_load_identity(&mut self, path: &str) -> Option<Identity> {
        if self.identity_cache.is_none() {
            self.identity_cache = Self::load_identity(path).ok();
        }
        self.identity_cache.clone()
    }

    pub async fn process_connect(
        &mut self,
        config: &NodeConfig,
        id: EventId,
        remote_address: SocketAddr,
        db: &StoreClient<P2pMessage>,
        source_type: Initiator,
    ) -> ProcessingConnectionResult {
        let have_identity = if let Some(identity) = self.try_load_identity(&config.identity_path) {
            let parser = connection_parser::Parser {
                identity,
                config: config.clone(),
                source_type,
                remote_address,
                id: id.socket_id.clone(),
                db: db.clone(),
            };
            let connection = Connection::spawn(self.tx_connection_report.clone(), parser);
            if let Some(old) = self.working_connections.insert(id.socket_id, connection) {
                if let Some(report) = old.join().await {
                    self.closed_connections.push(report)
                }
            }
            true
        } else {
            false
        };
        ProcessingConnectionResult { have_identity }
    }

    pub async fn process_close(&mut self, event_id: EventId) {
        // can safely drop the old connection
        if let Some(old) = self.working_connections.remove(&event_id.socket_id) {
            if let Some(report) = old.join().await {
                self.closed_connections.push(report)
            }
        }
    }

    pub async fn process_data(&mut self, message: Message) {
        match self.working_connections.get_mut(&message.event_id.socket_id) {
            Some(connection) => connection.process(message).await,
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
