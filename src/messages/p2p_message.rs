// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Serialize, Deserialize};
use tezos_messages::p2p::encoding::{
    connection::ConnectionMessage,
    metadata::MetadataMessage,
    ack::AckMessage,
    peer::{PeerMessage, PeerMessageResponse},
};
use tezos_encoding::encoding::{HasEncoding, Encoding};
use std::net::SocketAddr;
use std::time::{SystemTime, UNIX_EPOCH};
use storage::persistent::{Decoder, SchemaError, Encoder};
use std::str::FromStr;

#[derive(Debug, Serialize, Deserialize, Copy, Clone, PartialEq)]
/// Determines, if message belongs to communication originated
/// from remote or local node
pub enum SourceType {
    #[serde(rename = "local")]
    Local,
    #[serde(rename = "remote")]
    Remote,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
/// P2PMessage as stored in the database
pub struct P2pMessage {
    pub id: Option<u64>,
    pub timestamp: u128,
    pub remote_addr: SocketAddr,
    pub incoming: bool,
    pub source_type: SourceType,
    pub original_bytes: Vec<u8>,
    // decrypted_bytes is the same as the original_bytes if it is ConnectionMessage
    // it is empty if decryption failed
    pub decrypted_bytes: Vec<u8>,
    pub message: Result<TezosPeerMessage, String>,
}

impl Decoder for P2pMessage {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        serde_cbor::from_slice(bytes)
            .map_err(|_| SchemaError::DecodeError)
    }
}

impl Encoder for P2pMessage {
    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        serde_cbor::to_vec(self)
            .map_err(|_| SchemaError::EncodeError)
    }
}

impl P2pMessage {
    /// Create new UNIX timestamp
    fn make_ts() -> u128 {
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
    }

    /// Make new P2pMessage from parts
    pub fn new(
        remote_addr: SocketAddr,
        incoming: bool,
        source_type: SourceType,
        original_bytes: Vec<u8>,
        decrypted_bytes: Vec<u8>,
        message: Result<TezosPeerMessage, String>,
    ) -> Self {
        Self {
            id: None,
            timestamp: Self::make_ts(),
            source_type,
            remote_addr,
            incoming,
            original_bytes,
            decrypted_bytes,
            message,
        }
    }

    /// Get source type of this message
    pub fn source_type(&self) -> SourceType {
        self.source_type
    }

    /// Get incoming flag of this message
    pub fn is_incoming(&self) -> bool {
        self.incoming
    }

    /// Get remote address of this message
    pub fn remote_addr(&self) -> SocketAddr {
        self.remote_addr
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
/// Detailed representation of peer messages mapped from
/// tezedge encoding, with difference, that most of
/// binary data are cast to hex values
pub enum TezosPeerMessage {
    ConnectionMessage(ConnectionMessage),
    MetadataMessage(MetadataMessage),
    AckMessage(AckMessage),
    PeerMessage(PeerMessage),
    PartialPeerMessage(PartialPeerMessage),
}

#[derive(Serialize, Deserialize, Debug, Clone, strum::EnumString)]
pub enum PartialPeerMessage {
    Disconnect,
    Advertise,
    SwapRequest,
    SwapAck,
    Bootstrap,
    GetCurrentBranch,
    CurrentBranch,
    Deactivate,
    GetCurrentHead,
    CurrentHead,
    GetBlockHeaders,
    BlockHeader,
    GetOperations,
    Operation,
    GetProtocols,
    Protocol,
    GetOperationHashesForBlocks,
    OperationHashesForBlock,
    GetOperationsForBlocks,
    OperationsForBlocks,
}

impl PartialPeerMessage {
    pub fn from_bytes(s: &[u8]) -> Option<Self> {
        match PeerMessageResponse::encoding() {
            Encoding::Obj(obj) => {
                match obj.first() {
                    Some(field) => match field.get_encoding() {
                        // with box_patterns feature will be possible
                        // Encoding::Dynamic(box Encoding::List(box Encoding::Tags(s, tags)))
                        Encoding::Dynamic(encoding) => match &**encoding {
                            Encoding::List(encoding) => match &**encoding {
                                Encoding::Tags(2, tags) => {
                                    let mut id = [0; 2];
                                    id.clone_from_slice(&s[4..6]);
                                    match tags.find_by_id(u16::from_be_bytes(id)) {
                                        Some(tag) => Self::from_str(tag.get_variant().as_str()).ok(),
                                        None => None,
                                    }
                                },
                                _ => None,
                            },
                            _ => None,
                        },
                        _ => None,
                    },
                    None => None,
                }
            },
            _ => None
        }
    }
}

impl Clone for TezosPeerMessage {
    fn clone(&self) -> Self {
        match self {
            &TezosPeerMessage::ConnectionMessage(ref m) => TezosPeerMessage::ConnectionMessage(m.clone()),
            &TezosPeerMessage::MetadataMessage(ref m) => TezosPeerMessage::MetadataMessage(m.clone()),
            &TezosPeerMessage::AckMessage(ref m) => {
                // `tezos_messages` does not provide `AckMessage::clone`, let's emulate it using serde
                let j = serde_json::to_value(m).unwrap();
                let m = serde_json::from_value(j).unwrap();
                TezosPeerMessage::AckMessage(m)
            },
            &TezosPeerMessage::PeerMessage(ref m) => TezosPeerMessage::PeerMessage(m.clone()),
            &TezosPeerMessage::PartialPeerMessage(ref s) => TezosPeerMessage::PartialPeerMessage(s.clone()),
        }
    }
}
