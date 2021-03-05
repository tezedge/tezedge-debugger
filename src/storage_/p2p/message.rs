use std::net::SocketAddr;
use serde::{Serialize, Deserialize};
use storage::persistent::{KeyValueSchema, BincodeEncoded};
use super::{DbMessage, Access};

/// P2PMessage as stored in the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub node_name: String,
    pub id: Option<u64>,
    pub timestamp: u128,
    pub remote_addr: SocketAddr,
    pub incoming: bool,
    //pub source_type: SourceType,
    pub original_bytes: Vec<u8>,
    // decrypted_bytes is the same as the original_bytes if it is ConnectionMessage
    // it is empty if decryption failed
    pub decrypted_bytes: Vec<u8>,
    pub error: Vec<String>,
    //pub message: Vec<TezosPeerMessage>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub ordinal_id: Option<u64>,
}

impl DbMessage for Message {
    fn set_id(&mut self, id: u64) {
        self.id = Some(id);
    }

    fn set_ordinal_id(&mut self, id: u64) {
        self.ordinal_id = Some(id);
    }
}

impl Access<SocketAddr> for Message {
    fn accessor(&self) -> SocketAddr {
        self.remote_addr.clone()
    }
}

impl BincodeEncoded for Message {}

pub struct Schema;

impl KeyValueSchema for Schema {
    type Key = u64;
    type Value = Message;

    fn name() -> &'static str { "p2p_message_storage" }
}
