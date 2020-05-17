// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::net::SocketAddr;
use tezos_messages::p2p::encoding::peer::{PeerMessage, PeerMessageResponse};
use crate::{
    network::connection_message::ConnectionMessage,
    actors::prelude::*,
};
use serde::{Serialize, Deserialize};
use storage::persistent::BincodeEncoded;
use tezos_messages::p2p::encoding::metadata::MetadataMessage;
use std::time::{SystemTime, UNIX_EPOCH};

/// Types of messages stored in database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StoreMessage {
    /// Raw Tcp message, part of tcp connection handling.
    /// Not part of tezos node communication, but internet working.
    TcpMessage {
        timestamp: u128,
        incoming: bool,
        remote_addr: SocketAddr,
        packet: Vec<u8>,
    },
    /// Metadata message, (first encrypted message received/sent between peers)
    Metadata {
        timestamp: u128,
        incoming: bool,
        remote_addr: SocketAddr,
        message: MetadataMessage,
    },
    /// Unencrypted message, which is part of tezos communication handshake
    ConnectionMessage {
        timestamp: u128,
        incoming: bool,
        remote_addr: SocketAddr,
        payload: ConnectionMessage,
    },
    /// Actual deciphered P2P message sent by some tezos node
    P2PMessage {
        timestamp: u128,
        incoming: bool,
        remote_addr: SocketAddr,
        request_id: Option<u64>,
        remote_requested: Option<bool>,
        payload: Vec<PeerMessage>,
    },
    /// RPC Request/Response
    RestMessage {
        timestamp: u128,
        incoming: bool,
        remote_addr: SocketAddr,
        payload: RESTMessage,
    },
}

impl StoreMessage {
    /// Create new nano-second timestamp
    fn make_ts() -> u128 {
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
    }

    /// Create storage message for raw TCP packet
    pub fn new_tcp(msg: &RawPacketMessage) -> Self {
        StoreMessage::TcpMessage {
            remote_addr: msg.remote_addr(),
            incoming: msg.is_incoming(),
            packet: msg.clone_packet(),
            timestamp: Self::make_ts(),
        }
    }

    /// Create new storage message for parsed Metadata
    pub fn new_metadata(remote_addr: SocketAddr, incoming: bool, message: MetadataMessage) -> Self {
        StoreMessage::Metadata {
            incoming,
            remote_addr,
            message,
            timestamp: Self::make_ts(),
        }
    }

    /// Create new storage message for parsed Connection message
    pub fn new_connection(remote_addr: SocketAddr, incoming: bool, msg: &ConnectionMessage) -> Self {
        StoreMessage::ConnectionMessage {
            incoming,
            remote_addr,
            payload: msg.clone(),
            timestamp: Self::make_ts(),
        }
    }

    /// Create new storeage message for parse P2P message
    pub fn new_p2p(remote_addr: SocketAddr, incoming: bool, msg: &PeerMessageResponse) -> Self {
        let c = bincode::serialize(msg.messages()).unwrap();
        let payload = bincode::deserialize(&c).unwrap();
        StoreMessage::P2PMessage {
            remote_addr,
            incoming,
            payload,
            request_id: None,
            remote_requested: None,
            timestamp: Self::make_ts(),
        }
    }

    /// Create new storeage message for parse RPC message
    pub fn new_rest(remote_addr: SocketAddr, incoming: bool, payload: RESTMessage) -> Self {
        StoreMessage::RestMessage {
            remote_addr,
            incoming,
            payload,
            timestamp: Self::make_ts(),
        }
    }

    /// Get remote address of this storage message
    pub fn remote_addr(&self) -> SocketAddr {
        match self {
            StoreMessage::RestMessage { remote_addr, .. } | StoreMessage::ConnectionMessage { remote_addr, .. } |
            StoreMessage::P2PMessage { remote_addr, .. } | StoreMessage::TcpMessage { remote_addr, .. } |
            StoreMessage::Metadata { remote_addr, .. } => remote_addr.clone()
        }
    }
}

impl BincodeEncoded for StoreMessage {}

#[derive(Debug, Serialize, Deserialize, Clone)]
/// Represents REST Response or Request made to the local node
pub enum RESTMessage {
    Request {
        method: String,
        path: String,
        payload: String,
    },
    Response {
        status: String,
        payload: String,
    },
}

impl BincodeEncoded for RESTMessage {}
