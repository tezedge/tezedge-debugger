use std::net::SocketAddr;
use tezos_messages::p2p::encoding::peer::{PeerMessage, PeerMessageResponse};
use crate::{
    network::connection_message::ConnectionMessage,
    actors::prelude::*,
};
use serde::{Serialize, Deserialize};
use crate::storage::rpc_message::RESTMessage;
use storage::persistent::BincodeEncoded;

/// Types of messages stored in database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StoreMessage {
    /// Raw Tcp message, part of tcp connection handling.
    /// Not part of tezos node communication, but internet working.
    TcpMessage {
        source: SocketAddr,
        destination: SocketAddr,
        packet: Vec<u8>,
    },

    /// Unencrypted message, which is part of tezos communication handshake
    ConnectionMessage {
        source: SocketAddr,
        destination: SocketAddr,
        payload: ConnectionMessage,
    },

    /// Actual deciphered P2P message sent by some tezos node
    P2PMessage {
        source: SocketAddr,
        destination: SocketAddr,
        payload: Vec<PeerMessage>,
    },

    /// RPC Request/Response
    RestMessage {
        source: SocketAddr,
        destination: SocketAddr,
        payload: RESTMessage,
    },
}

impl StoreMessage {
    pub fn new_conn(source: SocketAddr, destination: SocketAddr, msg: &ConnectionMessage) -> Self {
        let c = bincode::serialize(msg).unwrap();
        let payload = bincode::deserialize(&c).unwrap();
        Self::ConnectionMessage {
            source,
            destination,
            payload,
        }
    }

    pub fn new_peer(source: SocketAddr, destination: SocketAddr, msg: &PeerMessageResponse) -> Self {
        let c = bincode::serialize(msg.messages()).unwrap();
        let payload = bincode::deserialize(&c).unwrap();
        Self::P2PMessage {
            source,
            destination,
            payload,
        }
    }

    pub fn new_tcp(msg: &RawPacketMessage) -> Self {
        Self::TcpMessage {
            source: msg.source_addr(),
            destination: msg.destination_addr(),
            packet: msg.clone_packet(),
        }
    }
}

impl BincodeEncoded for StoreMessage {}
