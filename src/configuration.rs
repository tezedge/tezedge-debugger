use argh::FromArgs;
use serde::{Serialize, Deserialize};
use failure::Error;

#[derive(FromArgs, Debug, Clone, PartialEq)]
/// Simple packet manipulator for tezedge node for testing and development purposes.
pub struct AppConfig {
    #[argh(option, default = "9732")]
    /// tezedge p2p port
    pub port: u16,
    #[argh(option)]
    /// path to the local identity
    pub identity_file: String,
}

impl AppConfig {
    pub fn from_env() -> Self {
        argh::from_env()
    }

    pub fn load_identity(&self) -> Result<Identity, Error> {
        use std::fs;
        let content = fs::read_to_string(&self.identity_file)?;
        Ok(serde_json::from_str(&content)?)
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