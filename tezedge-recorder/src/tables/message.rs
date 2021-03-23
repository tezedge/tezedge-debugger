// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::net::SocketAddr;
use serde::{Serialize, Deserialize};
use super::common::{Initiator, Sender, MessageCategory, MessageKind};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    timestamp: u128,
    remote_addr: SocketAddr,
    initiator: Initiator,
    sender: Sender,
    category: Option<MessageCategory>,
    kind: Option<MessageKind>,
    chunks: Vec<u64>,
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MessageFrontend {
    timestamp: u128,
    remote_addr: SocketAddr,
    source_type: Initiator,
    incoming: bool,
    category: Option<MessageCategory>,
    kind: Option<MessageKind>,
    error: Option<String>,
}

impl MessageFrontend {
    pub fn new(item: Item) -> Self {
        MessageFrontend {
            timestamp: item.timestamp,
            remote_addr: item.remote_addr,
            source_type: item.initiator,
            incoming: item.sender.incoming(),
            category: item.category,
            kind: item.kind,
            error: item.error,
        }
    }
}
