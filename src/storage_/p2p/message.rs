use std::{net::SocketAddr, str::FromStr, time::{SystemTime, UNIX_EPOCH}};
use serde::{Serialize, Deserialize};
use storage::persistent::{KeyValueSchema, BincodeEncoded};
use tezos_messages::p2p::encoding::{
    connection::ConnectionMessage,
    metadata::MetadataMessage,
    ack::AckMessage,
    peer::PeerMessageResponse,
    prelude::*,
};
use tezos_encoding::encoding::{HasEncoding, Encoding};
use super::{Access, indices::{P2pType, Initiator, Sender, NodeName}, MessageHasId, KeyValueSchemaExt};

/// P2PMessage as stored in the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: u64,
    pub node_name: NodeName,
    pub timestamp: u128,
    pub remote_addr: SocketAddr,
    pub source_type: Initiator,
    pub sender: Sender,
    pub original_bytes: Vec<u8>,
    // decrypted_bytes is the same as the original_bytes if it is ConnectionMessage
    // it is empty if decryption failed
    pub decrypted_bytes: Vec<u8>,
    pub error: Option<String>,
    pub message: Option<TezosPeerMessage>,
}

impl Message {
    /// Make new P2pMessage from parts
    pub fn new(
        node_name: NodeName,
        remote_addr: SocketAddr,
        source_type: Initiator,
        sender: Sender,
        original_bytes: Vec<u8>,
        decrypted_bytes: Vec<u8>,
        error: Option<String>,
    ) -> Self {
        Message {
            id: 0,
            node_name,
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos(),
            source_type,
            remote_addr,
            sender,
            original_bytes,
            decrypted_bytes,
            error,
            message: None,
        }
    }

    pub fn with_message(
        node_name: NodeName,
        remote_addr: SocketAddr,
        source_type: Initiator,
        sender: Sender,
        original_bytes: Vec<u8>,
        decrypted_bytes: Vec<u8>,
        message: Result<TezosPeerMessage, String>,
    ) -> Self {
        let (message, error) = match message {
            Ok(message) => (Some(message), None),
            Err(error) => (None, Some(error)),
        };
        Message {
            id: 0,
            node_name,
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos(),
            source_type,
            remote_addr,
            sender,
            original_bytes,
            decrypted_bytes,
            error,
            message,
        }
    }
}

/// Detailed representation of peer messages mapped from
/// tezedge encoding, with difference, that most of
/// binary data are cast to hex values
#[derive(Debug, Serialize, Deserialize, Clone)]
//#[serde(untagged)]
pub enum TezosPeerMessage {
    HandshakeMessage(HandshakeMessage),
    PeerMessage(FullPeerMessage),
    PartialPeerMessage(PartialPeerMessage),
}

#[derive(Debug, Serialize, Deserialize)]
//#[serde(tag = "type", rename_all = "snake_case")]
pub enum HandshakeMessage {
    ConnectionMessage(ConnectionMessage),
    MetadataMessage(MetadataMessage),
    AckMessage(AckMessage),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
//#[serde(tag = "type", rename_all = "snake_case")]
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

#[derive(Debug, Serialize, Deserialize, strum::EnumString, Clone)]
//#[serde(tag = "type", rename_all = "snake_case")]
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

impl Access<P2pType> for Message {
    fn accessor(&self) -> P2pType {
        if let Some(msg) = &self.message {
            match msg {
                TezosPeerMessage::PartialPeerMessage(PartialPeerMessage::Disconnect) |
                TezosPeerMessage::PeerMessage(FullPeerMessage::Disconnect) => P2pType::Disconnect,
                TezosPeerMessage::PartialPeerMessage(PartialPeerMessage::Bootstrap) |
                TezosPeerMessage::PeerMessage(FullPeerMessage::Bootstrap) => P2pType::Bootstrap,
                TezosPeerMessage::PartialPeerMessage(PartialPeerMessage::Advertise) |
                TezosPeerMessage::PeerMessage(FullPeerMessage::Advertise(_)) => P2pType::Advertise,
                TezosPeerMessage::PartialPeerMessage(PartialPeerMessage::SwapRequest) |
                TezosPeerMessage::PeerMessage(FullPeerMessage::SwapRequest(_)) => P2pType::SwapRequest,
                TezosPeerMessage::PartialPeerMessage(PartialPeerMessage::SwapAck) |
                TezosPeerMessage::PeerMessage(FullPeerMessage::SwapAck(_)) => P2pType::SwapAck,
                TezosPeerMessage::PartialPeerMessage(PartialPeerMessage::GetCurrentBranch) |
                TezosPeerMessage::PeerMessage(FullPeerMessage::GetCurrentBranch(_)) => P2pType::GetCurrentBranch,
                TezosPeerMessage::PartialPeerMessage(PartialPeerMessage::CurrentBranch) |
                TezosPeerMessage::PeerMessage(FullPeerMessage::CurrentBranch(_)) => P2pType::CurrentBranch,
                TezosPeerMessage::PartialPeerMessage(PartialPeerMessage::Deactivate) |
                TezosPeerMessage::PeerMessage(FullPeerMessage::Deactivate(_)) => P2pType::Deactivate,
                TezosPeerMessage::PartialPeerMessage(PartialPeerMessage::GetCurrentHead) |
                TezosPeerMessage::PeerMessage(FullPeerMessage::GetCurrentHead(_)) => P2pType::GetCurrentHead,
                TezosPeerMessage::PartialPeerMessage(PartialPeerMessage::CurrentHead) |
                TezosPeerMessage::PeerMessage(FullPeerMessage::CurrentHead(_)) => P2pType::CurrentHead,
                TezosPeerMessage::PartialPeerMessage(PartialPeerMessage::GetBlockHeaders) |
                TezosPeerMessage::PeerMessage(FullPeerMessage::GetBlockHeaders(_)) => P2pType::GetBlockHeaders,
                TezosPeerMessage::PartialPeerMessage(PartialPeerMessage::BlockHeader) |
                TezosPeerMessage::PeerMessage(FullPeerMessage::BlockHeader(_)) => P2pType::BlockHeader,
                TezosPeerMessage::PartialPeerMessage(PartialPeerMessage::GetOperations) |
                TezosPeerMessage::PeerMessage(FullPeerMessage::GetOperations(_)) => P2pType::GetOperations,
                TezosPeerMessage::PartialPeerMessage(PartialPeerMessage::Operation) |
                TezosPeerMessage::PeerMessage(FullPeerMessage::Operation(_)) => P2pType::Operation,
                TezosPeerMessage::PartialPeerMessage(PartialPeerMessage::GetProtocols) |
                TezosPeerMessage::PeerMessage(FullPeerMessage::GetProtocols(_)) => P2pType::GetProtocols,
                TezosPeerMessage::PartialPeerMessage(PartialPeerMessage::Protocol) |
                TezosPeerMessage::PeerMessage(FullPeerMessage::Protocol(_)) => P2pType::Protocol,
                TezosPeerMessage::PartialPeerMessage(PartialPeerMessage::GetOperationHashesForBlocks) |
                TezosPeerMessage::PeerMessage(FullPeerMessage::GetOperationHashesForBlocks(_)) => P2pType::GetOperationHashesForBlocks,
                TezosPeerMessage::PartialPeerMessage(PartialPeerMessage::OperationHashesForBlock) |
                TezosPeerMessage::PeerMessage(FullPeerMessage::OperationHashesForBlock(_)) => P2pType::OperationHashesForBlock,
                TezosPeerMessage::PartialPeerMessage(PartialPeerMessage::GetOperationsForBlocks) |
                TezosPeerMessage::PeerMessage(FullPeerMessage::GetOperationsForBlocks(_)) => P2pType::GetOperationsForBlocks,
                TezosPeerMessage::PartialPeerMessage(PartialPeerMessage::OperationsForBlocks) |
                TezosPeerMessage::PeerMessage(FullPeerMessage::OperationsForBlocks(_)) => P2pType::OperationsForBlocks,
                TezosPeerMessage::HandshakeMessage(HandshakeMessage::ConnectionMessage(_)) => P2pType::ConnectionMessage,
                TezosPeerMessage::HandshakeMessage(HandshakeMessage::MetadataMessage(_)) => P2pType::Metadata,
                TezosPeerMessage::HandshakeMessage(HandshakeMessage::AckMessage(_)) => P2pType::AckMessage,
            }
        } else {
            P2pType::P2PMessage
        }
    }
}

impl Access<Sender> for Message {
    fn accessor(&self) -> Sender {
        self.sender.clone()
    }
}

impl Access<Initiator> for Message {
    fn accessor(&self) -> Initiator {
        self.source_type.clone()
    }
}

impl Access<NodeName> for Message {
    fn accessor(&self) -> NodeName {
        self.node_name.clone()
    }
}

impl Access<SocketAddr> for Message {
    fn accessor(&self) -> SocketAddr {
        self.remote_addr.clone()
    }
}

impl MessageHasId for Message {
    fn set_id(&mut self, id: u64) {
        self.id = id;
    }
}

impl BincodeEncoded for Message {}

pub struct Schema;

impl KeyValueSchema for Schema {
    type Key = u64;
    type Value = Message;

    fn name() -> &'static str { "p2p_message_storage" }
}

impl KeyValueSchemaExt for Schema {
    fn short_id() -> u16 {
        0x0002
    }
}
