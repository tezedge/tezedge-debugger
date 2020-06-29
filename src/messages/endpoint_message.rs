// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Serialize, Deserialize};
use crate::messages::p2p_message::{P2pMessage, SourceType, PeerMessage};
use std::net::SocketAddr;


#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EndpointMessage {
    P2pMessage {
        #[serde(flatten)]
        message: P2pMessage,
    },
    Metadata {
        incoming: bool,
        timestamp: u128,
        id: u64,
        source_type: SourceType,
        remote_addr: SocketAddr,
        message: PeerMessage,
    },
    ConnectionMessage {
        incoming: bool,
        timestamp: u128,
        id: u64,
        source_type: SourceType,
        remote_addr: SocketAddr,
        message: PeerMessage,
    },
}

impl From<P2pMessage> for EndpointMessage {
    fn from(mut message: P2pMessage) -> Self {
        let msg = message.message.first().unwrap();
        match msg {
            PeerMessage::ConnectionMessage(_) => {
                let msg = message.message.pop().unwrap();
                Self::ConnectionMessage {
                    message: msg,
                    incoming: message.incoming,
                    timestamp: message.timestamp,
                    id: message.id.unwrap_or_default(),
                    source_type: message.source_type,
                    remote_addr: message.remote_addr,
                }
            }
            PeerMessage::MetadataMessage(_) => {
                let msg = message.message.pop().unwrap();
                Self::Metadata {
                    message: msg,
                    incoming: message.incoming,
                    timestamp: message.timestamp,
                    id: message.id.unwrap_or_default(),
                    source_type: message.source_type,
                    remote_addr: message.remote_addr,
                }
            }
            _ => Self::P2pMessage { message }
        }
    }
}