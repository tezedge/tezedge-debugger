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
    pub closed_connections: Vec<p2p::ConnectionReport>,
    pub alive_connections: Vec<p2p::ConnectionReport>,
    pub total_chunks: usize,
    pub decrypted_chunks: usize,
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
        SNIFFER_RESPONSE.lock().unwrap().clone()
    }
}

struct BpfSnifferInner {
    module: Module,
    settings: SystemSettings,
    connections: HashMap<SocketId, mpsc::UnboundedSender<Either<p2p::Message, p2p::Command>>>,
    counter: u64,
    debug_stop_tx: Option<mpsc::Sender<p2p::ConnectionReport>>,
    debug_stop_rx: mpsc::Receiver<p2p::ConnectionReport>,
    last_timestamp: u64,
    identity: Option<Identity>,
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
        }
    }

    async fn next_event(events: &mut RingBuffer, commands: &mut mpsc::UnboundedReceiver<BpfSnifferCommand>) -> Option<Either<RingBufferData, BpfSnifferCommand>> {
        /*match commands.try_recv() {
            Ok(command) => Some(Either::Right(command)),
            Err(TryRecvError::Closed) => events.next().await.map(Either::Left),
            Err(TryRecvError::Empty) => {
                let command = commands.recv().fuse();
                pin_mut!(command);

                // TODO:
                // WARNING: it is a hack
                // sometimes ring buffer stop producing events, should be fixed on ring buffer side
                // as a quick fix, lets poll it again if it does not issue any event in a second
                let timer = tokio::time::delay_for(std::time::Duration::from_secs(1)).fuse();
                pin_mut!(timer);
                let next_event_intermediate = events.next().fuse();
                pin_mut!(next_event_intermediate);

                let next_event = future::select(timer, next_event_intermediate)
                    .map(|either| match either {
                        // bad variant, timer resolved earlier then any other event
                        Either::Left((_, event_future)) => {
                            tracing::warn!("ring buffer stuck");
                            Either::Left(event_future)
                        },
                        Either::Right((event, timer)) => {
                            // drop the timer
                            let _ = timer;
                            Either::Right(event)
                        }
                    });
                pin_mut!(next_event);

                match future::select(command, next_event).await {
                    Either::Left((None, events)) => {
                        tracing::info!("command sender disconnected");
                        match events.await {
                            Either::Right(event) => event.map(Either::Left),
                            Either::Left(event_future) => event_future.await.map(Either::Left),
                        }
                    },
                    Either::Left((Some(BpfSnifferCommand::Terminate), _)) => None,
                    Either::Left((Some(command), _)) => Some(Either::Right(command)),
                    Either::Right((Either::Right(event), _)) => event.map(Either::Left),
                    // our bad case, let's await it again
                    Either::Right((Either::Left(event_future), _)) => event_future.await.map(Either::Left),
                }
            }
        }*/
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

    async fn send_report(&mut self, closed_connections: &Vec<p2p::ConnectionReport>) {
        debug_assert!(self.debug_stop_rx.try_recv().is_err(), "should collect all reports in main loop");

        let mut report = BpfSnifferReport {
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos(),
            closed_connections: closed_connections.clone(),
            alive_connections: vec![],
            total_chunks: 0,
            decrypted_chunks: 0,
        };

        for (_, connection) in &self.connections {
            connection.send(Either::Right(p2p::Command::GetDebugData)).ok().unwrap();
            let running = self.debug_stop_rx.recv().await.unwrap();
            report.alive_connections.push(running);
        }

        let (total_chunks_closed, decrypted_chunks_closed) = report.closed_connections.iter()
            .fold((0, 0), |(a, b), c| (a + c.report.total_chunks, b + c.report.decrypted_chunks));

        let (total_chunks_running, decrypted_chunks_running) = report.alive_connections.iter()
            .fold((0, 0), |(a, b), c| (a + c.report.total_chunks, b + c.report.decrypted_chunks));

        report.total_chunks = total_chunks_closed + total_chunks_running;
        report.decrypted_chunks = decrypted_chunks_closed + decrypted_chunks_running;

        let mut response = SNIFFER_RESPONSE.lock().unwrap();
        *response = Some(BpfSnifferResponse::Report(report));
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
                        Ok(SnifferEvent::Connect { id, address }) => s.on_connect(id, address, db.clone(), false),
                        Ok(SnifferEvent::Listen { id }) => {
                            tracing::info!(
                                id = tracing::field::display(&id),
                                msg = "P2P Listen",
                            );
                        }
                        Ok(SnifferEvent::Accept { id: _, listen_on_fd: _ }) => {
                            // TODO: fix it
                            // let address = SocketAddr::new(s.settings.local_address.clone(), 0x4321);
                            // s.on_connect(id, address, db.clone(), true)
                        },
                        Ok(SnifferEvent::Close { id }) => s.on_close(id),
                        Ok(SnifferEvent::Read { id, data }) => s.on_data(id, data.to_vec(), true),
                        Ok(SnifferEvent::Write { id, data }) => s.on_data(id, data.to_vec(), false),
                        Ok(SnifferEvent::Debug { id, msg }) => tracing::warn!("{} {}", id, msg),
                    }        
                },
                Either::Right(BpfSnifferCommand::Terminate) => break,
                Either::Right(BpfSnifferCommand::GetDebugData { filename, report }) => {
                    if let Some(filename) = filename {
                        Self::dump_rb(&filename, &mut events).await;
                    }
                    if report {
                        s.send_report(&closed_connections).await;
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

    fn on_connect(&mut self, id: EventId, address: SocketAddr, db: mpsc::UnboundedSender<P2pMessage>, incoming: bool) {
        self.check(&id);

        let should_ignore = !incoming && ignore(&self.settings, &address);
        let msg = if incoming { "P2P New Incoming" } else { "P2P New Outgoing" };
        tracing::info!(
            address = tracing::field::debug(&address),
            id = tracing::field::display(&id),
            ignore = should_ignore,
            msg = msg,
        );
        if should_ignore {
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

        let (tx, rx) = mpsc::unbounded_channel();
        // drop old connection, it cause termination stream on the p2p parser,
        // so the p2p parser will know about it
        self.connections.insert(id.socket_id.clone(), tx);
        let mut debug_tx = self.debug_stop_tx.as_ref().cloned().unwrap();
        let parser = p2p::Parser {
            settings: self.settings.clone(),
            source_type: if incoming { SourceType::Remote } else { SourceType::Local },
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

        tracing::info!(
            id = tracing::field::display(&id),
            msg = "P2P Close",
        );
        // can safely drop the old connection
        self.connections.remove(&id.socket_id);
    }

    fn on_data(&mut self, id: EventId, payload: Vec<u8>, incoming: bool) {
        self.check(&id);

        match self.connections.get_mut(&id.socket_id) {
            Some(connection) => {
                let message = p2p::Message {
                    payload,
                    incoming,
                    counter: self.counter,
                    event_id: id.clone(),
                };
                self.counter += 1;
                match connection.send(Either::Left(message)) {
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
        // dns and other well known not tezos
        53 | 80 | 443 => {
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
