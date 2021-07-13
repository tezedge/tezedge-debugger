// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    collections::HashMap,
    sync::{Arc, atomic::AtomicBool},
    net::SocketAddr,
    io, thread,
};
use serde::Deserialize;
use anyhow::Result;
use thiserror::Error;
use tokio::{runtime::Runtime, task::JoinHandle};
use super::{
    database::{DatabaseNew, DatabaseFetch, Database},
    server, log_client,
};

#[derive(Clone, Deserialize)]
pub struct NodeConfig {
    pub db_path: String,
    pub identity_path: String,
    pub p2p_port: u16,
    pub rpc_port: Option<u16>,
    pub syslog_port: Option<u16>,
}

#[derive(Clone, Deserialize)]
struct Config {
    pub bpf_sniffer_path: String,
    pub rpc_port: Option<u16>,
    pub nodes: Vec<NodeConfig>,
}

#[derive(Clone)]
pub struct Identity {
    pub public_key: [u8; 32],
    pub secret_key: [u8; 32],
}

pub struct NodeInfo {
    identity: Identity,
    p2p_port: u16,
}

#[derive(Error, Debug)]
pub enum NodeError {
    #[error("failed to open identity {}", _0)]
    OpenIdentity(io::Error),
    #[error("failed to parse identity {}", _0)]
    ParseIdentity(serde_json::Error),
    #[error("failed to parse public key from hex")]
    ParsePk,
    #[error("failed to parse secret key from hex")]
    ParseSk,
}

pub struct NodeServer {
    _server: Option<JoinHandle<()>>,
    _log_client: Option<thread::JoinHandle<()>>,
}

pub struct System<Db> {
    config: Config,
    port_to_pid: HashMap<u16, u32>,
    node_info: HashMap<u32, NodeInfo>,
    node_servers: HashMap<u16, NodeServer>,
    node_dbs: HashMap<u16, Arc<Db>>,
    _old_server: Option<JoinHandle<()>>,
    tokio_rt: Runtime,
}

impl NodeServer {
    pub fn open_spawn<Db>(
        db_path: &str,
        rpc_port: Option<u16>,
        syslog_port: Option<u16>,
        rt: &Runtime,
        running: Arc<AtomicBool>,
    ) -> Result<(Self, Arc<Db>)>
    where
        Db: DatabaseNew + Database + DatabaseFetch + Sync + Send + 'static,
    {
        let db = Arc::new(Db::open(db_path)?);
        let server = if let Some(port) = rpc_port {
            let addr = ([0, 0, 0, 0], port);
            Some(rt.spawn(warp::serve(server::routes(db.clone())).run(addr)))
        } else {
            None
        };
        let log_client = if let Some(port) = syslog_port {
            Some(log_client::spawn(port, db.clone(), running)?)
        } else {
            None
        };

        Ok((
            NodeServer {
                _server: server,
                _log_client: log_client,
            },
            db,
        ))
    }
}

impl NodeInfo {
    pub fn new(identity_path: &str, p2p_port: u16) -> Result<Self, NodeError> {
        use std::{fs::File, convert::TryInto};

        #[derive(Deserialize)]
        pub struct Inner {
            #[allow(dead_code)]
            peer_id: String,
            public_key: String,
            secret_key: String,
            #[allow(dead_code)]
            proof_of_work_stamp: String,
        }

        let file = File::open(identity_path).map_err(NodeError::OpenIdentity)?;
        let Inner {
            public_key,
            secret_key,
            ..
        } = serde_json::from_reader(file).map_err(NodeError::ParseIdentity)?;

        let identity = Identity {
            public_key: {
                hex::decode(public_key)
                    .map_err(|_| NodeError::ParsePk)?
                    .try_into()
                    .map_err(|_| NodeError::ParsePk)?
            },
            secret_key: {
                hex::decode(secret_key)
                    .map_err(|_| NodeError::ParseSk)?
                    .try_into()
                    .map_err(|_| NodeError::ParseSk)?
            },
        };

        Ok(NodeInfo { identity, p2p_port })
    }

    pub fn identity(&self) -> Identity {
        self.identity.clone()
    }
}

impl<Db> System<Db> {
    pub fn load_config() -> Result<Self> {
        use std::{fs::File, io::Read};

        let mut settings_file = File::open("config.toml")
            .or_else(|_| File::open("/etc/config.toml"))?;
        let mut settings_toml = String::new();
        settings_file.read_to_string(&mut settings_toml)?;
        let config = toml::from_str(&settings_toml)?;

        Ok(System {
            config,
            port_to_pid: HashMap::new(),
            node_info: HashMap::new(),
            node_servers: HashMap::new(),
            node_dbs: HashMap::new(),
            _old_server: None,
            tokio_rt: Runtime::new().unwrap(),
        })
    }

    pub fn sniffer_path(&self) -> &str {
        self.config.bpf_sniffer_path.as_ref()
    }

    pub fn node_configs(&self) -> &[NodeConfig] {
        self.config.nodes.as_slice()
    }

    pub fn should_ignore(&self, address: &SocketAddr) -> bool {
        use std::net::IpAddr;

        match address.port() {
            0 | 65535 => {
                return true;
            },
            // dns and other well known not tezos
            53 | 80 | 443 | 22 => {
                return true;
            },
            // ignore syslog
            p => {
                if self
                    .config
                    .nodes
                    .iter()
                    .any(|n| n.syslog_port.unwrap_or(0) == p)
                {
                    return true;
                }
            },
        }
        // lo v6
        if address.ip() == "::1".parse::<IpAddr>().unwrap() {
            return true;
        }
        // lo v4
        if address.ip() == "127.0.0.1".parse::<IpAddr>().unwrap() {
            return true;
        }

        false
    }
}

impl<Db> System<Db>
where
    Db: DatabaseNew + Database + DatabaseFetch + Sync + Send + 'static,
{
    pub fn run_dbs(&mut self, running: Arc<AtomicBool>) {
        for c in &self.config.nodes {
            let r = running.clone();
            let rt = &self.tokio_rt;
            match NodeServer::open_spawn(&c.db_path, c.rpc_port, c.syslog_port, rt, r) {
                Ok((server, db)) => {
                    self.node_servers.insert(c.p2p_port, server);
                    self.node_dbs.insert(c.p2p_port, db);
                },
                Err(error) => {
                    log::error!("{}", error);
                },
            }
        }

        if let Some(port) = self.config.rpc_port {
            let addr = ([0, 0, 0, 0], port);
            let s = warp::serve(server::routes_old(self.node_dbs.clone())).run(addr);
            self._old_server = Some(self.tokio_rt.spawn(s));
        }
    }

    pub fn handle_bind(&mut self, pid: u32, port: u16) -> Result<()> {
        let info = if let Some(old_pid) = self.port_to_pid.remove(&port) {
            log::info!("detaching from pid: {} at port: {}", old_pid, port);
            self.node_info.remove(&old_pid).unwrap()
        } else {
            let c = self
                .node_configs()
                .iter()
                .find(|c| c.p2p_port == port)
                .unwrap();
            NodeInfo::new(&c.identity_path, c.p2p_port)?
        };
        log::info!("attaching to pid: {} at port: {}", pid, port);
        self.port_to_pid.insert(port, pid);
        self.node_info.insert(pid, info);

        Ok(())
    }

    pub fn get_mut(&mut self, pid: u32) -> Option<(&mut NodeInfo, Arc<Db>)> {
        let db = self
            .node_info
            .get(&pid)
            .map(|i| i.p2p_port)
            .and_then(|port| self.node_dbs.get(&port))?
            .clone();
        let info = self.node_info.get_mut(&pid)?;
        Some((info, db))
    }
}
