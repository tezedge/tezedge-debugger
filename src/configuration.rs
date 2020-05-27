use std::fs;
use argh::FromArgs;
use failure::Error;
use crate::utility::identity::Identity;
use crate::storage::Storage;
use std::net::IpAddr;

#[derive(FromArgs, Debug, Clone)]
/// TezEdge Debugger - Tezos p2p network debugger.
pub struct AppConfig {
    #[argh(option, default = "\"./identity/identity.json\".to_string()")]
    /// path to the local identity file
    pub identity: String,
    #[argh(option, default = "9732")]
    /// local node RPC port number
    pub rpc_port: u16,
    #[argh(option, default = "\"./storage\".to_string()")]
    /// path to the local storage
    pub storage: String,
    #[argh(option)]
    /// address on which runs the ocaml node
    pub local_address: IpAddr,
}

impl AppConfig {
    pub fn from_env() -> Self { argh::from_env() }

    pub fn load_identity(&self) -> Result<Identity, Error> {
        let content = fs::read_to_string(&self.identity)?;
        Ok(serde_json::from_str(&content)?)
    }

    pub fn open_storage(&self) -> Result<Storage, Error> {
        Storage::open(&self.storage)
    }
}