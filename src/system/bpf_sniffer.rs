// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    convert::TryFrom,
    net::{IpAddr, SocketAddr},
    collections::HashMap,
    fs::File,
    io::Write,
    path::{PathBuf, Path},
    fmt,
    lazy::SyncLazy,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};
use tezos_conversation::Identity;
use tokio::{stream::StreamExt, sync::mpsc::{self, error::TryRecvError}};
use serde::{Serialize, Deserialize};
use sniffer::{SocketId, EventId, Module, SnifferEvent, RingBufferData, RingBuffer};
use futures::{
    future::{self, FutureExt, Either},
    pin_mut,
};

use super::{SystemSettings, p2p, processor};
use crate::messages::p2p_message::{P2pMessage, SourceType};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BpfSnifferReport {
    pub timestamp: u128,
    pub total_chunks: u64,
    pub decrypted_chunks: u64,
    pub closed_connections: Vec<p2p::ConnectionReport>,
    pub alive_connections: Vec<p2p::ConnectionReport>,
    pub difference: Vec<PeerDifference>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "case", rename_all = "snake_case")]
pub enum PeerDifference {
    Difference {
        remote_address: SocketAddr,
        peer_id: String,
        metadata: p2p::PeerMetadata,
    },
    NodeHasDebuggerHasNot {
        peer_id: String,
        metadata: p2p::PeerMetadata,
    },
    DebuggerHasNodeHasNot {
        remote_address: SocketAddr,
        peer_id: String,
        metadata: p2p::PeerMetadata,
    },
}

#[derive(Debug)]
/// The command for the sniffer
pub enum BpfSnifferCommand {
    /// Stop sniffing, the async task will terminate
    Terminate,
    /// if `filename` has a value, will dump the content of ring buffer to the file
    /// if report is true, will send a `BpfSnifferReport` as a `BpfSnifferResponse`
    GetDebugData {
        filename: Option<PathBuf>,
        report: bool,
        difference: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BpfSnifferResponse {
    Report(BpfSnifferReport),
}

#[derive(Clone)]
pub struct BpfSniffer {
    command_tx: mpsc::UnboundedSender<BpfSnifferCommand>,
}

static SNIFFER_RESPONSE: SyncLazy<Mutex<Option<BpfSnifferResponse>>> = SyncLazy::new(|| Mutex::new(None));

impl BpfSniffer {
    pub fn spawn(settings: &SystemSettings) -> Self {
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let bpf_sniffer = BpfSnifferInner::new(settings);
        tokio::spawn(bpf_sniffer.run(command_rx));
        BpfSniffer { command_tx }
    }

    pub fn send(&self, command: BpfSnifferCommand) {
        self.command_tx.send(command)
            .expect("failed to send command")
    }

    pub fn recv() -> Option<BpfSnifferResponse> {
        SNIFFER_RESPONSE.lock().unwrap().take()
    }
}

struct Connection {
    tx: mpsc::UnboundedSender<Either<p2p::Message, p2p::Command>>,
    source_type: SourceType,
    // it is possible we receive/send connection message in wrong order
    // do connect and receive the message and then send
    // or do accept and send the message and then receive
    // probably it is due to TCP Fast Open
    unordered_connection_message: Option<(EventId, Vec<u8>)>,
    empty: bool,
    remote_address: SocketAddr,
}

impl Connection {
    fn new(source_type: SourceType, remote_address: SocketAddr) -> (Self, mpsc::UnboundedReceiver<Either<p2p::Message, p2p::Command>>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (
            Connection {
                tx,
                source_type,
                unordered_connection_message: None,
                empty: true,
                remote_address,
            },
            rx,
        )
    }

    fn send_message(&mut self, counter: u64, incoming: bool, payload: Vec<u8>, id: &EventId) {
        let message = p2p::Message {
            payload,
            incoming,
            counter,
            event_id: id.clone(),
        };
        match self.tx.send(Either::Left(message)) {
            Ok(()) => (),
            Err(_) => {
                tracing::error!(
                    id = tracing::field::display(&id),
                    incoming = incoming,
                    msg = "P2P Failed to forward message to the p2p parser",
                )
            },
        }
    }
}

struct BpfSnifferInner {
    module: Module,
    settings: SystemSettings,
    connections: HashMap<SocketId, Connection>,
    counter: u64,
    debug_stop_tx: Option<mpsc::Sender<p2p::ConnectionReport>>,
    debug_stop_rx: mpsc::Receiver<p2p::ConnectionReport>,
    last_timestamp: u64,
    identity: Option<Identity>,
    node_pid: Option<u32>,
}

/// Try to load identity from one of the well defined paths
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

impl BpfSnifferInner {
    pub fn new(settings: &SystemSettings) -> Self {
        let (tx, rx) = mpsc::channel(0x1000);
        BpfSnifferInner {
            module: Module::load(),
            settings: settings.clone(),
            connections: HashMap::new(),
            counter: 1,
            debug_stop_tx: Some(tx),
            debug_stop_rx: rx,
            last_timestamp: 0,
            identity: None,
            node_pid: None,
        }
    }

    async fn next_event(events: &mut RingBuffer, commands: &mut mpsc::UnboundedReceiver<BpfSnifferCommand>) -> Option<Either<RingBufferData, BpfSnifferCommand>> {
        match commands.try_recv() {
            Ok(command) => Some(Either::Right(command)),
            Err(TryRecvError::Closed) => events.next().await.map(Either::Left),
            Err(TryRecvError::Empty) => {
                let command = commands.recv().fuse();
                pin_mut!(command);
                let next_event = events.next().fuse();
                pin_mut!(next_event);
        
                match future::select(command, next_event).await {
                    Either::Left((None, events)) => {
                        tracing::info!("command sender disconnected");
                        events.await.map(Either::Left)
                    },
                    Either::Left((Some(BpfSnifferCommand::Terminate), _)) => None,
                    Either::Left((Some(command), _)) => Some(Either::Right(command)),
                    Either::Right((event, _)) => event.map(Either::Left),
                }
            }
        }
    }

    async fn dump_rb<P>(filename: &P, events: &mut RingBuffer)
    where
        P: AsRef<Path> + fmt::Debug,
    {
        let dump = events.dump();
        tracing::error!(consumer_pos = dump.pos, msg = "writing dump");
        let mut file = File::create(filename).unwrap();
        file.write_all(dump.as_ref()).unwrap();
        file.sync_all().unwrap();
        tracing::info!(path = tracing::field::debug(filename), msg = "written dump");
    }

    async fn prepare_report(&mut self, closed_connections: &Vec<p2p::ConnectionReport>, difference: bool) -> BpfSnifferReport {
        debug_assert!(self.debug_stop_rx.try_recv().is_err(), "should collect all reports in main loop");

        let mut report = BpfSnifferReport {
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos(),
            closed_connections: closed_connections.clone(),
            alive_connections: vec![],
            total_chunks: 0,
            decrypted_chunks: 0,
            difference: vec![],
        };

        for (_, connection) in &self.connections {
            connection.tx.send(Either::Right(p2p::Command::GetDebugData)).ok().unwrap();
            let running = self.debug_stop_rx.recv().await.unwrap();
            report.alive_connections.push(running);
        }

        let (total_chunks_closed, decrypted_chunks_closed) = report.closed_connections.iter()
            .fold((0, 0), |(a, b), c| (a + c.report.total_chunks, b + c.report.decrypted_chunks));

        let (total_chunks_running, decrypted_chunks_running) = report.alive_connections.iter()
            .fold((0, 0), |(a, b), c| (a + c.report.total_chunks, b + c.report.decrypted_chunks));

        report.total_chunks = total_chunks_closed + total_chunks_running;
        report.decrypted_chunks = decrypted_chunks_closed + decrypted_chunks_running;

        if difference {
            let i = report.closed_connections.iter().chain(report.alive_connections.iter());
            let difference = self.prepare_difference(i);
            report.difference = difference;
        }

        report
    }

    fn prepare_difference<'a, 'b, I>(&'b mut self, i: I) -> Vec<PeerDifference>
    where
        I: Iterator<Item = &'a p2p::ConnectionReport>,
    {
        // TODO: move to settings
        let node_report = serde_json::from_reader::<_, Vec<p2p::Peer>>(
            File::open("/tmp/volume/data/peers.json").unwrap(),
        ).unwrap();
        let mut node_report = node_report.into_iter().map(|p| (p.peer_id, p.peer_metadata))
            .collect::<HashMap<_, _>>();

        let mut difference = i.filter_map(|connection| {
                connection.report.peer_id.as_ref()
                    .map(|peer_id| {
                        if let Some(metadata) = node_report.remove(peer_id) {
                            PeerDifference::Difference {
                                remote_address: connection.remote_address.parse().unwrap(),
                                peer_id: peer_id.clone(),
                                metadata: &connection.report.peer_metadata - &metadata,
                            }
                        } else {
                            PeerDifference::DebuggerHasNodeHasNot {
                                remote_address: connection.remote_address.parse().unwrap(),
                                peer_id: peer_id.clone(),
                                metadata: connection.report.peer_metadata.clone(),
                            }
                        }
                    })
            })
            .collect::<Vec<_>>();
        for (peer_id, metadata) in node_report {
            difference.push(PeerDifference::NodeHasDebuggerHasNot { peer_id, metadata })
        }

        difference
    }

    pub async fn run(self, command_rx: mpsc::UnboundedReceiver<BpfSnifferCommand>) {
        let db = processor::spawn_processor(self.settings.clone());
        let mut s = self;
        let mut events = s.module.main_buffer();
        let mut commands = command_rx;
        let mut closed_connections = vec![];
        while let Some(event) = Self::next_event(&mut events, &mut commands).await {
            match event {
                Either::Left(slice) => {
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
                },
                Either::Right(BpfSnifferCommand::Terminate) => break,
                Either::Right(BpfSnifferCommand::GetDebugData { filename, report, difference }) => {
                    if let Some(filename) = filename {
                        Self::dump_rb(&filename, &mut events).await;
                    }
                    if report {
                        let report = s.prepare_report(&closed_connections, difference).await;
                        let mut response = SNIFFER_RESPONSE.lock().unwrap();
                        *response = Some(BpfSnifferResponse::Report(report));
                    }
                },
            }
            while let Ok(c) = s.debug_stop_rx.try_recv() {
                closed_connections.push(c);
            }
        }
    }

    fn check(&mut self, id: &EventId) {
        let ts = id.ts_finish();
        if ts < self.last_timestamp {
            tracing::error!("have unordered event");
        }
        self.last_timestamp = ts;
    }

    fn is_node_p2p_event(&self, id: &EventId) -> bool {
        if let Some(node_pid) = self.node_pid {
            id.socket_id.pid == node_pid
        } else {
            false
        }
    }

    fn on_connect(&mut self, id: EventId, address: SocketAddr, db: mpsc::UnboundedSender<P2pMessage>, listened_on: Option<u32>) {
        self.check(&id);

        let belong_to_node = self.is_node_p2p_event(&id);
        if !belong_to_node {
            tracing::info!(id = tracing::field::display(&id), msg = "ignore, filtered by pid");
            self.module.ignore(id.socket_id);
            return;
        }

        let should_ignore = ignore(&self.settings, &address);
        if should_ignore {
            tracing::info!(id = tracing::field::display(&id), msg = "ignore");
            self.module.ignore(id.socket_id);
            return;
        }

        let identity = match &self.identity {
            &Some(ref identity) => identity.clone(),
            &None => {
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
        let (connection, rx) = Connection::new(source_type.clone(), address.clone());
        // drop old connection, it cause termination stream on the p2p parser,
        // so the p2p parser will know about it
        self.connections.insert(id.socket_id.clone(), connection);
        let mut debug_tx = self.debug_stop_tx.as_ref().cloned().unwrap();
        let parser = p2p::Parser {
            settings: self.settings.clone(),
            source_type,
            remote_address: address,
            id: id.socket_id.clone(),
            db: db,
            identity: identity,
        };
        tokio::spawn(async move {
            let remote_address = parser.remote_address.to_string();
            let source_type = parser.source_type.clone();
            // factor out `Result`, it needed only for convenient error propagate deeper in stack
            let statistics = match parser.run(rx, debug_tx.clone()).await {
                Err(statistics) => statistics,
                Ok(statistics) => statistics,
            };
            let connection_report = p2p::ConnectionReport {
                remote_address,
                source_type,
                report: statistics,
            };
            debug_tx.send(connection_report).await.unwrap();
            drop(debug_tx);
        });
    }

    fn on_close(&mut self, id: EventId) {
        self.check(&id);

        // can safely drop the old connection
        self.connections.remove(&id.socket_id);
    }

    fn on_data(&mut self, id: EventId, payload: Vec<u8>, incoming: bool) {
        self.check(&id);

        match self.connections.get_mut(&id.socket_id) {
            Some(connection) => {
                match (connection.empty, connection.source_type, incoming) {
                    // send stored message first if any
                    (false, _, _) => {
                        if let Some((id, payload)) = connection.unordered_connection_message.take() {
                            connection.send_message(self.counter, !incoming, payload, &id);
                            self.counter += 1;
                        }
                        connection.send_message(self.counter, incoming, payload, &id);
                        self.counter += 1;
                    },
                    // connection is empty, we are initiator and receive an incoming message
                    // or we are responder and send an outgoing message
                    // should reorder connection messages in such case
                    (true, SourceType::Local, true) | (true, SourceType::Remote, false) => {
                        if payload.len() == 24 && incoming {
                            tracing::error!(
                                id = tracing::field::display(&id),
                                payload = tracing::field::display(hex::encode(payload.as_slice())),
                                msg = "P2P unexpected 24 bytes message",
                                address = tracing::field::display(&connection.remote_address),
                            );
                            return;
                        }
                        tracing::info!(
                            id = tracing::field::display(&id),
                            msg = "P2P receive connection messages in wrong order",        
                        );
                        // store the message without sending
                        connection.unordered_connection_message = Some((id, payload));
                        connection.empty = false;
                    },
                    // ok case, should not reorder
                    // mark as non empty and send message normally
                    (true, SourceType::Local, false) | (true, SourceType::Remote, true) => {
                        connection.empty = false;
                        connection.send_message(self.counter, incoming, payload, &id);
                        self.counter += 1;
                    }
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
        // dns and other well known not tezos
        53 | 80 | 443 | 22 => {
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
