use std::net::SocketAddr;
use serde::{Serialize, ser::{self, SerializeSeq}};
use super::{
    message::{Message, TezosPeerMessage, HandshakeMessage},
    indices::{P2pType, Initiator},
    Access,
};

/// P2PMessage as stored sent to the frontend
#[derive(Debug, Clone, Serialize)]
pub struct FrontendMessage {
    pub id: u64,
    pub timestamp: u128,
    pub remote_addr: SocketAddr,
    pub source_type: Initiator,
    pub incoming: bool,
    pub category: Option<MessageCategory>,
    pub kind: Option<MessageKind>,
    pub message_preview: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageCategory {
    Unknown,
    Connection,
    Meta,
    Ack,
    P2p,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageKind {
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

#[derive(Debug, Clone, Serialize)]
pub struct FrontendMessageDetails {
    id: u64,
    message: Option<TezosPeerMessage>,
    original_bytes: HexString,
    decrypted_bytes: HexString,
    error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct HexString(String);

impl HexString {
    pub fn from_bytes<T>(bytes: T) -> Self
    where
        T: AsRef<[u8]>,
    {
        HexString(hex::encode(bytes))
    }
}

impl Serialize for HexString {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        debug_assert_eq!(self.0.len() % 2, 0);

        let l = self.0.len() / 2;
        let mut s = serializer.serialize_seq(Some(l))?;
        for i in 0..l {
            s.serialize_element(&self.0[(2 * i)..(2 * i + 2)])?;
        }
        s.end()
    }
}

impl FrontendMessage {
    pub fn new(message: Message, truncate: usize) -> Self {
        let tezos_message = message.message.as_ref();
        let category = tezos_message
            .map(|m| match m {
                TezosPeerMessage::HandshakeMessage(HandshakeMessage::ConnectionMessage(_)) => MessageCategory::Connection,
                TezosPeerMessage::HandshakeMessage(HandshakeMessage::MetadataMessage(_)) => MessageCategory::Meta,
                TezosPeerMessage::HandshakeMessage(HandshakeMessage::AckMessage(_)) => MessageCategory::Ack,
                TezosPeerMessage::PartialPeerMessage(_) => MessageCategory::P2p,
                TezosPeerMessage::PeerMessage(_) => MessageCategory::P2p,
            });
        let kind = match Access::<P2pType>::accessor(&message) {
            P2pType::Tcp => None,
            P2pType::P2PMessage => None,
            P2pType::RestMessage => None,
            P2pType::ConnectionMessage => None,
            P2pType::Metadata => None,
            P2pType::AckMessage => None,
            P2pType::Disconnect => Some(MessageKind::Disconnect),
            P2pType::Advertise => Some(MessageKind::Advertise),
            P2pType::SwapRequest => Some(MessageKind::SwapRequest),
            P2pType::SwapAck => Some(MessageKind::SwapAck),
            P2pType::Bootstrap => Some(MessageKind::Bootstrap),
            P2pType::GetCurrentBranch => Some(MessageKind::GetCurrentBranch),
            P2pType::CurrentBranch => Some(MessageKind::CurrentBranch),
            P2pType::Deactivate => Some(MessageKind::Deactivate),
            P2pType::GetCurrentHead => Some(MessageKind::GetCurrentHead),
            P2pType::CurrentHead => Some(MessageKind::CurrentHead),
            P2pType::GetBlockHeaders => Some(MessageKind::GetBlockHeaders),
            P2pType::BlockHeader => Some(MessageKind::BlockHeader),
            P2pType::GetOperations => Some(MessageKind::GetOperations),
            P2pType::Operation => Some(MessageKind::Operation),
            P2pType::GetProtocols => Some(MessageKind::GetProtocols),
            P2pType::Protocol => Some(MessageKind::Protocol),
            P2pType::GetOperationHashesForBlocks => Some(MessageKind::GetOperationHashesForBlocks),
            P2pType::OperationHashesForBlock => Some(MessageKind::OperationHashesForBlock),
            P2pType::GetOperationsForBlocks => Some(MessageKind::GetOperationsForBlocks),
            P2pType::OperationsForBlocks => Some(MessageKind::OperationsForBlocks),
        };
        let message_preview = serde_json::to_string(&message.message)
            .ok()
            .map(|mut s| {
                s.truncate(truncate);
                s
            });
        FrontendMessage {
            id: message.id,
            timestamp: message.timestamp,
            remote_addr: message.remote_addr,
            source_type: message.source_type,
            incoming: message.sender.is_incoming(),
            category,
            kind,
            message_preview,
        }
    }
}

impl FrontendMessageDetails {
    pub fn new(message: Message) -> Self {
        FrontendMessageDetails {
            id: message.id,
            message: message.message,
            original_bytes: HexString::from_bytes(&message.original_bytes),
            decrypted_bytes: HexString::from_bytes(&message.decrypted_bytes),
            error: message.error,
        }
    }
}
