// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::fmt;
use serde::{Serialize, Deserialize};
use storage::persistent::{KeyValueSchema, BincodeEncoded};
use super::common::Sender;

#[derive(Clone)]
pub struct Item {
    connection_id: u128,
    number: u64,
    sender: Sender,
    bytes: Vec<u8>,
    plain: Vec<u8>,
}

impl Item {
    pub fn new(connection_id: u128, number: u64, incoming: bool, bytes: Vec<u8>, plain: Vec<u8>) -> Self {
        Item {
            connection_id,
            number,
            sender: if incoming { Sender::Remote } else { Sender::Local },
            bytes,
            plain,
        }
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn split(self) -> (Key, Value) {
        match self {
            Item { connection_id, number, sender, bytes, plain } => {
                (Key { connection_id, number, sender }, Value { bytes, plain })
            }
        }
    }
}

impl fmt::Debug for Item {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Item")
            .field("connection_id", &self.connection_id)
            .field("number", &self.number)
            .field("sender", &self.sender)
            .field("bytes", &hex::encode(&self.bytes))
            .field("plain", &hex::encode(&self.plain))
            .finish()
    }
}

#[derive(Serialize, Deserialize)]
pub struct Key {
    connection_id: u128,
    number: u64,
    sender: Sender,
}

impl BincodeEncoded for Key {}

#[derive(Serialize, Deserialize)]
pub struct Value {
    bytes: Vec<u8>,
    plain: Vec<u8>,
}

impl BincodeEncoded for Value {}

pub struct Schema;

impl KeyValueSchema for Schema {
    type Key = Key;
    type Value = Value;

    fn name() -> &'static str {
        "chunk_storage"
    }
}
