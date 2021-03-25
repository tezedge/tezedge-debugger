// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{collections::HashMap, sync::Arc, net::SocketAddr};
use serde::Deserialize;
use anyhow::Result;
use tokio::runtime::Runtime;
use super::{
    database::{DatabaseNew, DatabaseFetch},
    server,
};

#[derive(Clone, Deserialize)]
pub struct NodeConfig {
    pub db_path: String,
    pub identity_path: String,
    pub p2p_port: u16,
    pub rpc_port: u16,
}

#[derive(Clone, Deserialize)]
struct Config {
    pub syslog_port: u16,
    pub bpf_sniffer_path: String,
    pub nodes: Vec<NodeConfig>,
}

#[derive(Clone)]
pub struct Identity {
    pub peer_id: String,
    pub public_key: Vec<u8>,
    pub secret_key: Vec<u8>,
}

pub struct NodeInfo<Db> {
    identity: Identity,
    db: Arc<Db>,
}

pub struct System<Db> {
    config: Config,
    node_info: HashMap<u32, NodeInfo<Db>>,
    tokio_rt: Runtime,
}

impl<Db> NodeInfo<Db>
where
    Db: DatabaseNew + DatabaseFetch + Sync + Send + 'static,
{
    pub fn new(identity_path: &str, db_path: &str, rpc_port: u16, rt: &Runtime) -> Result<Self> {
        use std::fs::File;

        #[derive(Deserialize)]
        pub struct Inner {
            peer_id: String,
            public_key: String,
            secret_key: String,
            #[allow(dead_code)]
            proof_of_work_stamp: String,
        }

        let file = File::open(identity_path)?;
        let Inner {
            peer_id,
            public_key,
            secret_key,
            ..
        } = serde_json::from_reader(file)?;

        let identity = Identity {
            peer_id,
            public_key: hex::decode(public_key)?,
            secret_key: hex::decode(secret_key)?,
        };

        let db = Db::open(db_path)?;
        rt.spawn(warp::serve(server::routes(db.clone())).run(([0, 0, 0, 0], rpc_port)));

        Ok(NodeInfo { identity, db })
    }

    pub fn identity(&self) -> Identity {
        self.identity.clone()
    }

    pub fn db(&self) -> Arc<Db> {
        self.db.clone()
    }
}

impl<Db> System<Db> {
    pub fn load_config() -> Result<Self> {
        use std::{fs::File, io::Read};

        let mut settings_file = File::open("config-new.toml")?;
        let mut settings_toml = String::new();
        settings_file.read_to_string(&mut settings_toml)?;
        let config = toml::from_str(&settings_toml)?;

        Ok(System {
            config,
            node_info: HashMap::new(),
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
                if self.config.syslog_port == p {
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

    pub fn get_mut(&mut self, pid: u32) -> Option<&mut NodeInfo<Db>> {
        self.node_info.get_mut(&pid)
    }
}

impl<Db> System<Db>
where
    Db: DatabaseNew + DatabaseFetch + Sync + Send + 'static,
{
    pub fn handle_bind(&mut self, pid: u32, port: u16) -> Result<()> {
        let node_config = self
            .node_configs()
            .iter()
            .find(|c| c.p2p_port == port)
            .unwrap();
        let info = NodeInfo::new(
            &node_config.identity_path,
            &node_config.db_path,
            node_config.rpc_port,
            &self.tokio_rt,
        )?;
        self.node_info.insert(pid, info);

        Ok(())
    }
}
