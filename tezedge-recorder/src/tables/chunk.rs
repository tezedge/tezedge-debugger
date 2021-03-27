// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::fmt;
use serde::{Serialize, Deserialize};
use storage::persistent::{KeyValueSchema, BincodeEncoded};
use super::common::Sender;

#[derive(Clone)]
pub struct Item {
    connection_id: u128,
    sender: Sender,
    counter: u64,
    pub bytes: Vec<u8>,
    plain: Vec<u8>,
}

impl Item {
    pub fn new(
        connection_id: u128,
        sender: Sender,
        counter: u64,
        bytes: Vec<u8>,
        plain: Vec<u8>,
    ) -> Self {
        Item {
            connection_id,
            sender,
            counter,
            bytes,
            plain,
        }
    }

    #[rustfmt::skip]
    pub fn split(self) -> (Key, Value) {
        let Item { connection_id, counter, sender, bytes, plain } = self;
        (Key { connection_id, counter, sender }, Value { bytes, plain })
    }
}

impl fmt::Debug for Item {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Item")
            .field("connection_id", &self.connection_id)
            .field("sender", &self.sender)
            .field("counter", &self.counter)
            .field("bytes", &hex::encode(&self.bytes))
            .field("plain", &hex::encode(&self.plain))
            .finish()
    }
}

#[derive(Serialize, Deserialize)]
pub struct Key {
    connection_id: u128,
    counter: u64,
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
