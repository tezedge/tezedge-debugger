// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Serialize, Deserialize};
use tezos_messages::p2p::encoding::{
    connection::ConnectionMessage,
    metadata::MetadataMessage,
    ack::AckMessage,
    peer::PeerMessageResponse,
    prelude::*,
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

impl SourceType {
    pub fn is_local(&self) -> bool {
        match self {
            &SourceType::Local => true,
            &SourceType::Remote => false,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
/// P2PMessage as stored in the database
pub struct P2pMessage {
    pub node_name: String,
    pub id: Option<u64>,
    pub timestamp: u128,
    pub remote_addr: SocketAddr,
    pub incoming: bool,
    pub source_type: SourceType,
    pub original_bytes: Vec<u8>,
    // decrypted_bytes is the same as the original_bytes if it is ConnectionMessage
    // it is empty if decryption failed
    pub decrypted_bytes: Vec<u8>,
    pub error: Vec<String>,
    pub message: Vec<TezosPeerMessage>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub ordinal_id: Option<u64>,
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
        node_name: String,
        remote_addr: SocketAddr,
        incoming: bool,
        source_type: SourceType,
        original_bytes: Vec<u8>,
        decrypted_bytes: Vec<u8>,
        message_result: Result<TezosPeerMessage, String>,
    ) -> Self {
        let mut error = Vec::new();
        let mut message = Vec::new();
        match message_result {
            Ok(m) => message.push(m),
            Err(e) => error.push(e),
        };
        P2pMessage {
            node_name,
            id: None,
            timestamp: Self::make_ts(),
            source_type,
            remote_addr,
            incoming,
            original_bytes,
            decrypted_bytes,
            error,
            message,
            ordinal_id: None,
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

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
/// Detailed representation of peer messages mapped from
/// tezedge encoding, with difference, that most of
/// binary data are cast to hex values
pub enum TezosPeerMessage {
    HandshakeMessage(HandshakeMessage),
    PeerMessage(FullPeerMessage),
    PartialPeerMessage(PartialPeerMessage),
}

impl TezosPeerMessage {
    pub fn as_cm(&self) -> Option<&ConnectionMessage> {
        match self {
            TezosPeerMessage::HandshakeMessage(HandshakeMessage::ConnectionMessage(m)) => Some(m),
            _ => None,
        }
    }

    pub fn as_mut_cm(&mut self) -> Option<&mut ConnectionMessage> {
        match self {
            TezosPeerMessage::HandshakeMessage(HandshakeMessage::ConnectionMessage(m)) => Some(m),
            _ => None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HandshakeMessage {
    ConnectionMessage(ConnectionMessage),
    MetadataMessage(MetadataMessage),
    AckMessage(AckMessage),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FullPeerMessage {
    Disconnect,
    Advertise(AdvertiseMessage),
    SwapRequest(SwapMessage),
    SwapAck(SwapMessage),
    Bootstrap,
    GetCurrentBranch(GetCurrentBranchMessage),
    CurrentBranch(CurrentBranchMessage),
    Deactivate(DeactivateMessage),
    GetCurrentHead(GetCurrentHeadMessage),
    CurrentHead(CurrentHeadMessage),
    GetBlockHeaders(GetBlockHeadersMessage),
    BlockHeader(BlockHeaderMessage),
    GetOperations(GetOperationsMessage),
    Operation(OperationMessage),
    GetProtocols(GetProtocolsMessage),
    Protocol(ProtocolMessage),
    GetOperationHashesForBlocks(GetOperationHashesForBlocksMessage),
    OperationHashesForBlock(OperationHashesForBlocksMessage),
    GetOperationsForBlocks(GetOperationsForBlocksMessage),
    OperationsForBlocks(OperationsForBlocksMessage),
}

impl From<PeerMessage> for FullPeerMessage {
    fn from(v: PeerMessage) -> Self {
        match v {
            PeerMessage::Disconnect => FullPeerMessage::Disconnect,
            PeerMessage::Advertise(v) => FullPeerMessage::Advertise(v),
            PeerMessage::SwapRequest(v) => FullPeerMessage::SwapRequest(v),
            PeerMessage::SwapAck(v) => FullPeerMessage::SwapAck(v),
            PeerMessage::Bootstrap => FullPeerMessage::Bootstrap,
            PeerMessage::GetCurrentBranch(v) => FullPeerMessage::GetCurrentBranch(v),
            PeerMessage::CurrentBranch(v) => FullPeerMessage::CurrentBranch(v),
            PeerMessage::Deactivate(v) => FullPeerMessage::Deactivate(v),
            PeerMessage::GetCurrentHead(v) => FullPeerMessage::GetCurrentHead(v),
            PeerMessage::CurrentHead(v) => FullPeerMessage::CurrentHead(v),
            PeerMessage::GetBlockHeaders(v) => FullPeerMessage::GetBlockHeaders(v),
            PeerMessage::BlockHeader(v) => FullPeerMessage::BlockHeader(v),
            PeerMessage::GetOperations(v) => FullPeerMessage::GetOperations(v),
            PeerMessage::Operation(v) => FullPeerMessage::Operation(v),
            PeerMessage::GetProtocols(v) => FullPeerMessage::GetProtocols(v),
            PeerMessage::Protocol(v) => FullPeerMessage::Protocol(v),
            PeerMessage::GetOperationHashesForBlocks(v) => FullPeerMessage::GetOperationHashesForBlocks(v),
            PeerMessage::OperationHashesForBlock(v) => FullPeerMessage::OperationHashesForBlock(v),
            PeerMessage::GetOperationsForBlocks(v) => FullPeerMessage::GetOperationsForBlocks(v),
            PeerMessage::OperationsForBlocks(v) => FullPeerMessage::OperationsForBlocks(v),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, strum::EnumString)]
#[serde(tag = "type", rename_all = "snake_case")]
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

impl Clone for HandshakeMessage {
    fn clone(&self) -> Self {
        match self {
            &HandshakeMessage::ConnectionMessage(ref m) => HandshakeMessage::ConnectionMessage(m.clone()),
            &HandshakeMessage::MetadataMessage(ref m) => HandshakeMessage::MetadataMessage(m.clone()),
            &HandshakeMessage::AckMessage(ref m) => {
                // `tezos_messages` does not provide `AckMessage::clone`, let's emulate it using serde
                let j = serde_json::to_value(m).unwrap();
                let m = serde_json::from_value(j).unwrap();
                HandshakeMessage::AckMessage(m)
            },
        }
    }
}
