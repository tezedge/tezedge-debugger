
// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, atomic::{Ordering, AtomicBool}},
};
use anyhow::Result;
use bpf_common::{BpfModuleClient, SnifferEvent, Command, EventId, SocketId};

use super::{connection::Connection, database::{Database, DatabaseNew}, system::System};

pub fn run<Db>(system: System<Db>, running: Arc<AtomicBool>) -> Result<()>
where
    Db: 'static + Database + DatabaseNew,
{
    let (client, mut rb) = BpfModuleClient::new_sync(system.sniffer_path())?;
    let mut list = ConnectionList::new(client, system);
    list.watching()?;

    while running.load(Ordering::Relaxed) {
        let events = rb.read_blocking::<SnifferEvent>(&running)?;
        for event in events {
            match event {
                SnifferEvent::Bind { id, address } => {
                    if let Err(error) = list.system.handle_bind(id.socket_id.pid, address.port()) {
                        log::error!("failed to handle bind syscall: {}", error);
                    }
                },
                SnifferEvent::Listen { id } => {
                    let _ = id;
                },
                SnifferEvent::Connect { id, address } => {
                    list.handle_connection(id, address, false);
                },
                SnifferEvent::Accept { id, address, listen_on_fd } => {
                    let _ = listen_on_fd;
                    list.handle_connection(id, address, true);
                },
                SnifferEvent::Write { id, data } => {
                    list.handle_data(id, data, false);
                },
                SnifferEvent::Read { id, data } => {
                    list.handle_data(id, data, true);
                },
                SnifferEvent::Close { id } => {
                    list.handle_close(id);
                },
                SnifferEvent::Debug { id, msg } => {
                    log::warn!("{} {}", id, msg);
                },
            }
        }
    }

    Ok(())
}

struct ConnectionList<Db> {
    client: BpfModuleClient,
    system: System<Db>,
    connections: HashMap<SocketId, Connection<Db>>,
}

impl<Db> ConnectionList<Db>
where
    Db: 'static + Database + DatabaseNew,
{
    fn new(client: BpfModuleClient, system: System<Db>) -> Self {
        ConnectionList {
            client,
            system,
            connections: HashMap::new(),
        }
    }

    fn watching(&mut self) -> Result<()> {
        for node_config in self.system.node_configs() {
            self.client.send_command(Command::WatchPort { port: node_config.p2p_port })?;
        }

        Ok(())
    }

    fn handle_connection(&mut self, event_id: EventId, address: SocketAddr, incoming: bool) {
        let socket_id = event_id.socket_id;
        let pid = socket_id.pid;
        let fd = socket_id.fd;
        if !self.system.should_ignore(&address) {
            if let Some(info) = self.system.get_mut(pid) {
                let connection = Connection::new(address, incoming, info.identity(), info.db());
                if let Some(old) = self.connections.insert(socket_id, connection) {
                    old.join();
                }
                return;
            }
        }
        match self.client.send_command(Command::IgnoreConnection { pid, fd }) {
            Ok(()) => (),
            Err(error) => {
                log::error!("cannot ignore connection id: {}, error: {}", socket_id, error)
            }
        }
    }

    fn handle_data(&mut self, id: EventId, payload: Vec<u8>, incoming: bool) {
        if let Some(connection) = self.connections.get_mut(&id.socket_id) {
            match connection.handle_data(&payload, incoming) {
                Ok(()) => (),
                Err(error) => {
                    log::error!("failed to handle data id: {}, error: {:?}", id, error);
                }
            }
        } else {
            log::debug!("failed to handle data, connection does not exist: {}", id);
        }
    }

    fn handle_close(&mut self, id: EventId) {
        let socket_id = id.socket_id;
        if let Some(old) = self.connections.remove(&socket_id) {
            old.join();
        }
    }
}
