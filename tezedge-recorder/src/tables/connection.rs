// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::net::SocketAddr;
use serde::{Serialize, Deserialize};
use storage::persistent::{KeyValueSchema, BincodeEncoded};
use super::common::Initiator;

#[derive(Debug, Clone, Serialize)]
pub struct Item {
    pub id: u128,
    initiator: Initiator,
    pub remote_addr: SocketAddr,
    peer_id: Option<String>,
    comments: Vec<String>,
}

impl Item {
    pub fn new(id: u128, incoming: bool, remote_addr: SocketAddr) -> Self {
        Item {
            id,
            initiator: if incoming { Initiator::Remote } else { Initiator::Local },
            remote_addr,
            peer_id: None,
            comments: Vec::new(),
        }
    }

    pub fn set_peer_id(&mut self, peer_id: String) {
        self.peer_id = Some(peer_id);
    }

    pub fn add_comment(&mut self, comment: String) {
        self.comments.push(comment);
    }

    pub fn split(self) -> (Key, Value) {
        let Item { id, initiator, remote_addr, peer_id, comments } = self;
        (Key { id }, Value { initiator, remote_addr, peer_id, comments })
    }

    pub fn unite(key: Key, value: Value) -> Self {
        let (Key { id }, Value { initiator, remote_addr, peer_id, comments }) = (key, value);
        Item { id, initiator, remote_addr, peer_id, comments }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Key {
    id: u128,
}

impl BincodeEncoded for Key {}

#[derive(Serialize, Deserialize)]
pub struct Value {
    initiator: Initiator,
    remote_addr: SocketAddr,
    peer_id: Option<String>,
    comments: Vec<String>,
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
