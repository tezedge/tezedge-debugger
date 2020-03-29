use std::net::IpAddr;
use tezos_messages::p2p::encoding::peer::{PeerMessage, PeerMessageResponse};
use crate::{
    network::connection_message::ConnectionMessage,
    actors::prelude::*,
};
use serde::{Serialize, Deserialize};

/// Types of messages stored in database
#[derive(Debug, Serialize, Deserialize)]
pub enum StoreMessage {
    /// Raw Tcp message, part of tcp connection handling.
    /// Not part of tezos node communication, but internet working.
    TcpMessage {
        source: IpAddr,
        destination: IpAddr,
        packet: Vec<u8>,
    },

    /// Unencrypted message, which is part of tezos communication handshake
    ConnectionMessage {
        source: IpAddr,
        destination: IpAddr,
        payload: ConnectionMessage,
    },

    /// Actual deciphered P2P message sent by some tezos node
    P2PMessage {
        source: IpAddr,
        destination: IpAddr,
        payload: Vec<PeerMessage>,
    },
}

impl StoreMessage {
    pub fn new_conn(source: IpAddr, destination: IpAddr, msg: &ConnectionMessage) -> Self {
        let c = bincode::serialize(msg).unwrap();
        let payload = bincode::deserialize(&c).unwrap();
        Self::ConnectionMessage {
            source,
            destination,
            payload,
        }
    }

    pub fn new_peer(source: IpAddr, destination: IpAddr, msg: &PeerMessageResponse) -> Self {
        let c = bincode::serialize(msg.messages()).unwrap();
        let payload = bincode::deserialize(&c).unwrap();
        Self::P2PMessage {
            source,
            destination,
            payload,
        }
    }

    pub fn new_tcp(msg: &RawPacketMessage) -> Self {
        StoreMessage::TcpMessage {
            source: msg.source_addr(),
            destination: msg.destination_addr(),
            packet: msg.clone_packet(),
        }
    }
}
