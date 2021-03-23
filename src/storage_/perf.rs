use storage::persistent::{BincodeEncoded, KeyValueSchema};
use serde::{Serialize, Deserialize};
use super::{store::MessageHasId, remote::KeyValueSchemaExt};

#[derive(Serialize)]
pub struct Report<'a> {
    bytes_received: u64,
    bytes_processed: u64,
    per_connection: &'a [Message],
}

impl<'a> Report<'a> {
    pub fn try_new(msgs: &'a [Message]) -> Option<Self> {
        if msgs.is_empty() {
            return None;
        }

        let bytes_processed = msgs[1..].iter().map(|p| p.bytes_count).sum();

        Some(Report {
            bytes_received: msgs[0].bytes_count,
            bytes_processed,
            per_connection: &msgs[1..],
        })
    }
}

#[derive(Serialize, Deserialize)]
pub struct Message {
    bytes_count: u64,
}

impl Message {
    pub fn new(bytes_count: u64) -> Self {
        Message {
            bytes_count,
        }
    }
}

impl MessageHasId for Message {
    fn set_id(&mut self, id: u64) {
        let _ = id;
    }
}

impl BincodeEncoded for Message {}

pub struct Schema;

impl KeyValueSchema for Schema {
    type Key = u64;
    type Value = Message;

    fn name() -> &'static str { "perf_message_storage" }
}

impl KeyValueSchemaExt for Schema {
    fn short_id() -> u16 {
        0x0003
    }
}
