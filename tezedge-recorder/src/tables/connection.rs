// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::net::SocketAddr;
use serde::{Serialize, Deserialize};
use storage::persistent::{KeyValueSchema, BincodeEncoded};
use super::common::Initiator;

#[derive(Debug, Clone)]
pub struct Item {
    timestamp: u128,
    initiator: Initiator,
    remote_addr: SocketAddr,
    peer_id: Option<String>,
}

impl Item {
    pub fn new(incoming: bool, remote_addr: SocketAddr, peer_id: Option<String>) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();

        Item {
            timestamp,
            initiator: if incoming { Initiator::Remote } else { Initiator::Local },
            remote_addr,
            peer_id,
        }
    }

    pub fn split(self) -> (Key, Value) {
        (
            Key {
                timestamp: self.timestamp,
                initiator: self.initiator,
            },
            Value {
                remote_addr: self.remote_addr,
                peer_id: self.peer_id,
            },
        )
    }
}

#[derive(Serialize, Deserialize)]
pub struct Key {
    timestamp: u128,
    initiator: Initiator,
}

impl BincodeEncoded for Key {}

#[derive(Serialize, Deserialize)]
pub struct Value {
    remote_addr: SocketAddr,
    peer_id: Option<String>,
}

impl BincodeEncoded for Value {}

pub struct Schema;

impl KeyValueSchema for Schema {
    type Key = Key;
    type Value = Value;

    fn name() -> &'static str {
        "connection_storage"
    }
}
