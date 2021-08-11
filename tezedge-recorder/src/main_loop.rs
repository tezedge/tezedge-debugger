// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{Ordering, AtomicBool},
    },
    time::Duration,
};
use anyhow::Result;
use bpf_recorder::{BpfModuleClient, SnifferEvent, Command, EventId, SocketId};

use super::{
    processor::Connection,
    database::{Database, DatabaseNew, DatabaseFetch},
    system::System,
};

pub fn run<Db>(system: &mut System<Db>, running: Arc<AtomicBool>) -> Result<()>
where
    Db: Database + DatabaseNew + DatabaseFetch + Sync + Send + 'static,
{
    let (client, mut rb) = BpfModuleClient::new_sync(system.sniffer_path())?;
    let mut list = ConnectionList::new(client, system);
    list.watching()?;

    while running.load(Ordering::Relaxed) {
        let events = rb.read_blocking::<SnifferEvent>(&running)?;
        for event in events {
            match event {
                SnifferEvent::Bind { id, address } => {
                    // TODO: remove old connections on this port
                    if let Err(error) = list.system.handle_bind(id.socket_id.pid, address.port()) {
                        log::error!("failed to handle bind syscall: {}", error);
                    }
                    list.calibrate_duration(id.ts);
                },
                SnifferEvent::Listen { id } => {
                    let _ = id;
                },
                SnifferEvent::Connect { id, address, error } => {
                    let info = if let Some(error) = error {
                        ConnectionInfo::ConnectErr(address, error)
                    } else {
                        ConnectionInfo::ConnectOk(address)
                    };
                    list.handle_connection(id, info);
                },
                SnifferEvent::Accept {
                    id,
                    address,
                    listen_on_fd,
                } => {
                    let _ = listen_on_fd;
                    let info = match address {
                        Ok(address) => ConnectionInfo::AcceptOk(address),
                        Err(code) => ConnectionInfo::AcceptErr(code),
                    };
                    list.handle_connection(id, info);
                },
                SnifferEvent::Data {
                    id,
                    data,
                    net,
                    incoming,
                    error,
                } => {
                    list.handle_data(id, data, error, net, incoming);
                },
                SnifferEvent::Close { id } => {
                    list.handle_close(id);
                },
                SnifferEvent::GetFd { id } => {
                    list.handle_get_fd(id);
                },
                SnifferEvent::Debug { id, msg } => {
                    log::warn!("{} {}", id, msg);
                },
            }
        }
    }

    Ok(())
}

struct ConnectionList<'a, Db> {
    client: BpfModuleClient,
    // between 1970 and system boot
    timestamp_difference: Option<Duration>,
    system: &'a mut System<Db>,
    connections: HashMap<SocketId, Connection<Db>>,
}

#[derive(Debug)]
pub enum ConnectionInfo {
    AcceptOk(SocketAddr),
    AcceptErr(i32),
    ConnectOk(SocketAddr),
    ConnectErr(SocketAddr, i32),
}

impl ConnectionInfo {
    pub fn address(&self) -> Option<&SocketAddr> {
        match self {
            &ConnectionInfo::AcceptOk(ref address) => Some(address),
            &ConnectionInfo::AcceptErr(_) => None,
            &ConnectionInfo::ConnectOk(ref address) => Some(address),
            &ConnectionInfo::ConnectErr(ref address, _) => Some(address),
        }
    }
}

impl<'a, Db> ConnectionList<'a, Db>
where
    Db: Database + DatabaseNew + DatabaseFetch + Sync + Send + 'static,
{
    fn new(client: BpfModuleClient, system: &'a mut System<Db>) -> Self {
        ConnectionList {
            client,
            timestamp_difference: None,
            system,
            connections: HashMap::new(),
        }
    }

    fn calibrate_duration(&mut self, nanos_from_boot: u64) {
        use std::time::SystemTime;

        if self.timestamp_difference.is_none() {
            let timestamp = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap();
            self.timestamp_difference = Some(timestamp - Duration::from_nanos(nanos_from_boot));
        }
    }

    // duration between 1970 and event
    fn convert_time(&self, event_id: &EventId) -> Duration {
        if let Some(ts) = self.timestamp_difference {
            ts + Duration::from_nanos(event_id.ts)
        } else {
            log::error!("got event before calibrate time");
            Duration::from_nanos(0)
        }
    }

    fn watching(&mut self) -> Result<()> {
        for p2p_config in self.system.p2p_configs() {
            self.client.send_command(Command::WatchPort {
                port: p2p_config.port,
            })?;
        }

        Ok(())
    }

    fn handle_connection(&mut self, event_id: EventId, c_info: ConnectionInfo) {
        let timestamp = self.convert_time(&event_id);
        let socket_id = event_id.socket_id;
        let pid = socket_id.pid;
        let fd = socket_id.fd;
        log::info!("socket_id: {:?}", socket_id);
        log::info!("connection: {:?}", c_info);
        if c_info.address().map(|a| !self.system.should_ignore(a)).unwrap_or(true) {
            if let Some((info, db)) = self.system.get_mut(pid) {
                if let Some(connection) = Connection::new(timestamp, c_info, info.identity(), db) {
                    if let Some(old) = self.connections.insert(socket_id, connection) {
                        old.join(timestamp);
                    }
                }
            }
            return;
        }
        match self
            .client
            .send_command(Command::IgnoreConnection { pid, fd })
        {
            Ok(()) => (),
            Err(error) => {
                log::error!(
                    "cannot ignore connection id: {}, error: {}",
                    socket_id,
                    error
                )
            },
        }
    }

    fn handle_data(&mut self, id: EventId, payload: Vec<u8>, error: Option<i32>, net: bool, incoming: bool) {
        let timestamp = self.convert_time(&id);

        if payload.len() > 0x1000000 {
            log::warn!("received from ring buffer big payload {}", payload.len());
        }
        if let Some(connection) = self.connections.get_mut(&id.socket_id) {
            connection.handle_data(timestamp, &payload, error, net, incoming);
        } else {
            log::debug!("failed to handle data, connection does not exist: {}", id);
        }
    }

    fn handle_get_fd(&mut self, id: EventId) {
        let timestamp = self.convert_time(&id);
        let socket_id = id.socket_id;
        if let Some(c) = self.connections.remove(&socket_id) {
            c.warn_fd_changed();
            c.join(timestamp);
        }
    }

    fn handle_close(&mut self, id: EventId) {
        let timestamp = self.convert_time(&id);
        let socket_id = id.socket_id;
        if let Some(old) = self.connections.remove(&socket_id) {
            old.join(timestamp);
        }
    }
}
