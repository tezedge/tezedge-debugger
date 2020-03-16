use std::{
    fs, path::Path, sync::Arc,
};
use rocksdb::DB;
use argh::FromArgs;
use failure::Error;
use serde::{Serialize, Deserialize};
use crate::storage::MessageStore;


#[derive(FromArgs, Debug, Clone, PartialEq)]
/// Simple packet sniffer for Tezos nodes (for testing and development purposes).
pub struct AppConfig {
    #[argh(option, default = "\"eth0\".to_string()")]
    /// network interface to listen for communication
    pub interface: String,
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