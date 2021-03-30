// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{net::SocketAddr, ops::Range, convert::TryFrom};
use serde::{Serialize, Deserialize};
use storage::persistent::{KeyValueSchema, BincodeEncoded};
use super::{
    common::{Initiator, Sender, MessageCategory, MessageKind},
    connection,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    cn_ts: u64,
    cn_ts_nanos: u32,
    timestamp: u64,
    remote_addr: SocketAddr,
    initiator: Initiator,
    sender: Sender,
    category: MessageCategory,
    kind: MessageKind,
    chunks: Range<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MessageFrontend {
    id: u64,
    timestamp: u64,
    remote_addr: SocketAddr,
    source_type: Initiator,
    incoming: bool,
    category: MessageCategory,
    kind: MessageKind,
}

impl MessageFrontend {
    pub fn new(item: Item, id: u64) -> Self {
        MessageFrontend {
            id,
            timestamp: item.timestamp,
            remote_addr: item.remote_addr,
            source_type: item.initiator,
            incoming: item.sender.incoming(),
            category: item.category,
            kind: item.kind,
        }
    }
}

pub struct MessageBuilder {
    category: MessageCategory,
    kind: MessageKind,
    length: u32,
    chunks: Range<u64>,
}

pub struct MessageBuilderFull(MessageBuilder);

impl MessageBuilder {
    pub fn connection_message(length: u16) -> Self {
        MessageBuilder {
            category: MessageCategory::Connection,
            kind: MessageKind::None,
            length: length as u32,
            chunks: 0..1,
        }
    }

    pub fn metadata_message(length: usize) -> Self {
        MessageBuilder {
            category: MessageCategory::Meta,
            kind: MessageKind::None,
            length: length as u32,
            chunks: 1..2,
        }
    }

    pub fn acknowledge_message(length: usize) -> Self {
        MessageBuilder {
            category: MessageCategory::Ack,
            kind: MessageKind::None,
            length: length as u32,
            chunks: 2..3,
        }
    }

    // chunk_number >= 3
    pub fn peer_message(bytes: [u8; 6], chunk_number: u64) -> Self {
        MessageBuilder {
            category: MessageCategory::P2p,
            kind: {
                let tag = u16::from_be_bytes(<[u8; 2]>::try_from(&bytes[4..]).unwrap());
                MessageKind::from_tag(tag)
            },
            length: u32::from_be_bytes(<[u8; 4]>::try_from(&bytes[..4]).unwrap()) + 4,
            chunks: chunk_number..chunk_number,
        }
    }

    pub fn link_chunk(mut self, length: usize) -> Result<MessageBuilderFull, Option<Self>> {
        let length = length as u32;
        if self.length < length {
            Err(None)
        } else {
            self.length -= length;
            self.chunks.end += 1;
            if self.length == 0 {
                Ok(MessageBuilderFull(self))
            } else {
                Err(Some(self))
            }
        }
    }

    pub fn remaining(&self) -> usize {
        self.length as usize
    }
}

impl MessageBuilderFull {
    pub fn build(self, sender: &Sender, connection: &connection::Item) -> Item {
        use std::time::{SystemTime, UNIX_EPOCH};

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Item {
            cn_ts: connection.ts,
            cn_ts_nanos: connection.ts_nanos,
            timestamp,
            remote_addr: connection.remote_addr,
            initiator: connection.initiator.clone(),
            sender: sender.clone(),
            category: self.0.category,
            kind: self.0.kind,
            chunks: self.0.chunks,
        }
    }
}

impl BincodeEncoded for Item {}

pub struct Schema;

impl KeyValueSchema for Schema {
    type Key = u64;
    type Value = Item;

    fn name() -> &'static str {
        "message_storage"
    }
}
