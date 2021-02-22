use std::{
    convert::TryFrom,
    net::{SocketAddr, IpAddr},
    sync::{Arc, Mutex},
};
use tokio::{stream::StreamExt, sync::mpsc};
use sniffer::{BpfModule, SnifferEvent, RingBufferData, EventId};

use super::{p2p, reporter::Reporter, processor, SystemSettings};
use crate::{messages::p2p_message::{P2pMessage, SourceType}};

pub struct Parser {
    module: BpfModule,
    settings: SystemSettings,
    counter: u64,
    node_pid: Option<u32>,
}

enum Event {
    RbData(RingBufferData),
    P2pCommand(p2p::Command),
}

impl Parser {
    pub fn new(settings: &SystemSettings) -> Self {
        Parser {
            module: BpfModule::load(&settings.namespace),
            settings: settings.clone(),
            counter: 0,
            // unknown for now,
            // consider the process which first do bind syscall on `settings.node_p2p_port`
            // is our node
            node_pid: None,
        }
    }

    /// spawn a (green)thread which parse the data from the kernel,
    /// returns object which can report statistics
    pub fn spawn(self) -> Arc<Mutex<Reporter>> {
        let (tx_p2p_command, rx_p2p_command) = mpsc::channel(1);
        let (tx_p2p_report, rx_p2p_report) = mpsc::channel(1);
        tokio::spawn(self.run(rx_p2p_command, tx_p2p_report));
        let reporter = Reporter::new(tx_p2p_command, rx_p2p_report);
        Arc::new(Mutex::new(reporter))
    }

    async fn run(
        self,
        rx_p2p_command: mpsc::Receiver<p2p::Command>,
        tx_p2p_report: mpsc::Sender<serde_json::Value>,
    ) {
        let db = processor::spawn_processor(self.settings.clone());
        let rb = self.module.main_buffer();
        let mut s = self;
        // merge streams, let await either some data from the kernel,
        // or some command from the overlying code
        let mut stream =
            rb.map(Event::RbData).merge(rx_p2p_command.map(Event::P2pCommand));
        let mut p2p_parser = p2p::Parser::new(tx_p2p_report);
        while let Some(event) = stream.next().await {
            match event {
                Event::RbData(slice) => s.process(&mut p2p_parser, slice, &db).await,
                // while executing this command new slices from the kernel will not be processed
                // so it is impossible to have data race
                Event::P2pCommand(command) => p2p_parser.execute(command).await,
            }
        }
    }

    async fn process(&mut self, parser: &mut p2p::Parser, slice: RingBufferData, db: &mpsc::UnboundedSender<P2pMessage>) {
        match SnifferEvent::try_from(slice.as_ref()) {
            Err(error) => tracing::error!("{:?}", error),
            Ok(SnifferEvent::Bind { id, address }) => {
                tracing::info!(
                    id = tracing::field::display(&id),
                    address = tracing::field::display(&address),
                    msg = "Syscall Bind",
                );
                if address.ip().is_unspecified() && address.port() == self.settings.node_p2p_port {
                    self.node_pid = Some(id.socket_id.pid);
                }
            },
            Ok(SnifferEvent::Listen { id }) => {
                tracing::info!(
                    id = tracing::field::display(&id),
                    msg = "Syscall Listen",
                );
            },
            Ok(SnifferEvent::Connect { id, address }) => {
                tracing::info!(
                    id = tracing::field::display(&id),
                    address = tracing::field::display(&address),
                    msg = "Syscall Connect",
                );
                self.process_connect(parser, id, address, &db, None);
            },
            Ok(SnifferEvent::Accept { id, listen_on_fd, address }) => {
                tracing::info!(
                    id = tracing::field::display(&id),
                    listen_on_fd = tracing::field::display(&listen_on_fd),
                    address = tracing::field::display(&address),
                    msg = "Syscall Accept",
                );
                self.process_connect(parser, id, address, &db, Some(listen_on_fd))
            },
            Ok(SnifferEvent::Close { id }) => {
                tracing::info!(
                    id = tracing::field::display(&id),
                    msg = "Syscall Close",
                );
                self.process_close(parser, id).await
            },
            Ok(SnifferEvent::Read { id, data }) => {
                self.process_data(parser, id, data.to_vec(), true)
            },
            Ok(SnifferEvent::Write { id, data }) => {
                self.process_data(parser, id, data.to_vec(), false)
            },
            Ok(SnifferEvent::Debug { id, msg }) => tracing::warn!("{} {}", id, msg),
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
            // ignore rpc for now
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

    fn process_connect(
        &mut self,
        parser: &mut p2p::Parser,
        id: EventId,
        address: SocketAddr,
        db: &mpsc::UnboundedSender<P2pMessage>,
        listened_on: Option<u32>,
    ) {
        let source_type = if listened_on.is_some() {
            SourceType::Remote
        } else {
            SourceType::Local
        };
        let socket_id = id.socket_id.clone();

        // the message is not belong to the node
        if Some(socket_id.pid) != self.node_pid {
            tracing::info!(id = tracing::field::display(&id), msg = "ignore, filtered by pid");
            self.module.ignore(socket_id);
        } else if self.should_ignore(&address) {
            tracing::info!(id = tracing::field::display(&id), msg = "ignore");
            self.module.ignore(socket_id);
        } else {
            let r = parser.process_connect(&self.settings, id, address, db, source_type);
            if !r.have_identity {
                tracing::warn!("ignore connection because no identity");
                self.module.ignore(socket_id);
            }
        }
    }

    async fn process_close(&mut self, parser: &mut p2p::Parser, id: EventId) {
        parser.process_close(id).await;
    }

    fn process_data(
        &mut self,
        parser: &mut p2p::Parser,
        id: EventId,
        payload: Vec<u8>,
        incoming: bool,
    ) {
        self.counter += 1;
        let message = p2p::Message {
            payload,
            incoming,
            counter: self.counter,
            event_id: id,
        };
        parser.process_data(message);
    }
}
