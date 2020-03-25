use std::{
    fs, path::Path, sync::Arc,
};
use rocksdb::DB;
use argh::FromArgs;
use failure::Error;
use serde::{Serialize, Deserialize};
use crate::storage::MessageStore;


#[derive(FromArgs, Debug, Clone, PartialEq)]
/// Tezos communication proxy.
///
/// This works by utilizing two tun devices which captures and re-transmit all communication
/// on local node.
/// - First tun device (tun0 - inner) captures communication coming from local node, and transit
/// processed data incoming to local node (captured by tun1)
/// - Second tun device (tun1 - outer) transmit process communication coming from local node (capture by tun0),
/// and capture all incoming data from remote connections
pub struct AppConfig {
    #[argh(option)]
    /// network interface to listen for communication
    pub interface: String,
    #[argh(option)]
    /// local address associated with provided interface
    pub local_address: String,
    #[argh(option, default = "9732")]
    /// tezedge p2p port
    pub port: u16,
    #[argh(option)]
    /// path to the local identity
    pub identity_file: String,
    #[argh(option, default = "\"./storage\".to_string()")]
    /// path to initialize storage
    pub storage_path: String,
    #[argh(option, default = "true")]
    /// clean storage when starting the tool
    pub clean_storage: bool,
    #[argh(option, default = "\"tun0\".to_string()")]
    /// name for tun0 (inner) device
    pub tun0_name: String,
    #[argh(option, default = "\"tun1\".to_string()")]
    /// name for tun1 (outer) device
    pub tun1_name: String,
    #[argh(option, default = "\"10.0.0.0/31\".to_string()")]
    /// address space for tun0 (inner) device
    pub tun0_address_space: String,
    #[argh(option, default = "\"10.0.1.0/31\".to_string()")]
    /// address space for tun1 (outer) device
    pub tun1_address_space: String,
    #[argh(option, default = "\"10.0.0.1\".to_string()")]
    /// address space for tun0 (inner) device
    pub tun0_address: String,
    #[argh(option, default = "\"10.0.1.1\".to_string()")]
    /// address space for tun1 (outer) device
    pub tun1_address: String,
}

impl AppConfig {
    pub fn from_env() -> Self {
        argh::from_env()
    }

    pub fn load_identity(&self) -> Result<Identity, Error> {
        let content = fs::read_to_string(&self.identity_file)?;
        Ok(serde_json::from_str(&content)?)
    }

    pub fn open_database(&self) -> Result<MessageStore, Error> {
        let path = Path::new(&self.storage_path);
        Ok(MessageStore::new(Arc::new(DB::open_default(path)?)))
    }
}

/// This node identity information
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Identity {
    pub peer_id: String,
    pub public_key: String,
    pub secret_key: String,
    pub proof_of_work_stamp: String,
}