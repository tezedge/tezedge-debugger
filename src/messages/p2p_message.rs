// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crypto::hash::HashType;
use serde::{Serialize, Deserialize};
use tezos_messages::p2p::encoding::{
    prelude::*,
    prelude::PeerMessage as TezosPeerMessage,
    version::Version,
    operation_hashes_for_blocks::OperationHashesForBlock,
};
use std::net::SocketAddr;
use std::time::{SystemTime, UNIX_EPOCH};
use storage::persistent::{Decoder, SchemaError, Encoder};
use tezos_messages::p2p::encoding::ack::{NackMotive, NackInfo};

#[derive(Debug, Serialize, Deserialize, Copy, Clone, PartialEq)]
pub enum SourceType {
    #[serde(rename = "local")]
    Local,
    #[serde(rename = "remote")]
    Remote,
}

impl SourceType {
    pub fn from_p2p_msg(msg: &PeerMessage, incoming: bool) -> Self {
        match msg {
            PeerMessage::Disconnect | PeerMessage::ConnectionMessage(_) | PeerMessage::MetadataMessage(_)
            | PeerMessage::Advertise(_) | PeerMessage::Bootstrap | PeerMessage::SwapRequest(_)
            | PeerMessage::GetCurrentBranch(_) | PeerMessage::Deactivate(_) | PeerMessage::GetCurrentHead(_)
            | PeerMessage::GetBlockHeaders(_) | PeerMessage::GetOperations(_) | PeerMessage::GetProtocols(_)
            | PeerMessage::GetOperationHashesForBlocks(_) | PeerMessage::GetOperationsForBlocks(_) => Self::from_incoming(incoming),
            PeerMessage::SwapAck(_) | PeerMessage::CurrentBranch(_) | PeerMessage::CurrentHead(_)
            | PeerMessage::BlockHeader(_) | PeerMessage::Operation(_) | PeerMessage::Protocol(_)
            | PeerMessage::OperationHashesForBlock(_) | PeerMessage::OperationsForBlocks(_) => Self::from_incoming(!incoming),
            PeerMessage::_Reserved => Self::from_incoming(incoming),
        }
    }

    pub fn from_incoming(incoming: bool) -> Self {
        if incoming {
            Self::Remote
        } else {
            Self::Local
        }
    }

    pub fn as_bool(self) -> bool {
        if Self::Local == self {
            false
        } else {
            true
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct P2pMessage {
    pub id: Option<u64>,
    pub timestamp: u128,
    pub remote_addr: SocketAddr,
    pub incoming: bool,
    pub source_type: SourceType,
    pub message: Vec<PeerMessage>,
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
    fn make_ts() -> u128 {
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
    }

    pub fn new<T: Into<PeerMessage>>(remote_addr: SocketAddr, incoming: bool, values: Vec<T>) -> Self {
        let payload = values.into_iter().map(|x| x.into()).collect::<Vec<PeerMessage>>();
        let source_type = payload.first().map(|msg| SourceType::from_p2p_msg(msg, incoming))
            .unwrap_or(SourceType::from_incoming(incoming));
        Self {
            id: None,
            timestamp: Self::make_ts(),
            source_type,
            remote_addr,
            incoming,
            message: payload,
        }
    }

    pub fn source_type(&self) -> SourceType {
        self.source_type
    }

    pub fn is_incoming(&self) -> bool {
        self.incoming
    }

    pub fn remote_addr(&self) -> SocketAddr {
        self.remote_addr
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PeerMessage {
    Disconnect,
    Bootstrap,
    Advertise(AdvertiseMessage),
    SwapRequest(SwapMessage),
    SwapAck(SwapMessage),
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
    ConnectionMessage(ConnectionMessage),
    MetadataMessage(MetadataMessage),
    _Reserved,
}

impl From<TezosPeerMessage> for PeerMessage {
    fn from(value: TezosPeerMessage) -> Self {
        match value {
            TezosPeerMessage::Disconnect => PeerMessage::Disconnect,
            TezosPeerMessage::Bootstrap => PeerMessage::Bootstrap,
            TezosPeerMessage::Advertise(msg) => PeerMessage::Advertise(msg),
            TezosPeerMessage::SwapRequest(msg) => PeerMessage::SwapRequest(msg),
            TezosPeerMessage::SwapAck(msg) => PeerMessage::SwapAck(msg),
            TezosPeerMessage::GetCurrentBranch(msg) => PeerMessage::GetCurrentBranch(msg.into()),
            TezosPeerMessage::CurrentBranch(msg) => PeerMessage::CurrentBranch(msg.into()),
            TezosPeerMessage::Deactivate(msg) => PeerMessage::Deactivate(msg.into()),
            TezosPeerMessage::GetCurrentHead(msg) => PeerMessage::GetCurrentHead(msg.into()),
            TezosPeerMessage::CurrentHead(msg) => PeerMessage::CurrentHead(msg.into()),
            TezosPeerMessage::GetBlockHeaders(msg) => PeerMessage::GetBlockHeaders(msg.into()),
            TezosPeerMessage::BlockHeader(msg) => PeerMessage::BlockHeader(msg.into()),
            TezosPeerMessage::GetOperations(msg) => PeerMessage::GetOperations(msg.into()),
            TezosPeerMessage::Operation(msg) => PeerMessage::Operation(msg.into()),
            TezosPeerMessage::GetProtocols(msg) => PeerMessage::GetProtocols(msg.into()),
            TezosPeerMessage::Protocol(msg) => PeerMessage::Protocol(msg.into()),
            TezosPeerMessage::GetOperationHashesForBlocks(msg) => PeerMessage::GetOperationHashesForBlocks(msg.into()),
            TezosPeerMessage::OperationHashesForBlock(msg) => PeerMessage::OperationHashesForBlock(msg.into()),
            TezosPeerMessage::GetOperationsForBlocks(msg) => PeerMessage::GetOperationsForBlocks(msg.into()),
            TezosPeerMessage::OperationsForBlocks(msg) => PeerMessage::OperationsForBlocks(msg.into()),
        }
    }
}

impl From<ConnectionMessage> for PeerMessage {
    fn from(value: ConnectionMessage) -> Self {
        PeerMessage::ConnectionMessage(value.into())
    }
}

impl From<MetadataMessage> for PeerMessage {
    fn from(value: MetadataMessage) -> Self {
        PeerMessage::MetadataMessage(value)
    }
}

impl PeerMessage {
    pub fn inner(&self) -> Option<TezosPeerMessage> {
        match self {
            PeerMessage::Disconnect => Some(TezosPeerMessage::Disconnect),
            PeerMessage::Bootstrap => Some(TezosPeerMessage::Bootstrap),
            PeerMessage::Advertise(msg) => Some(TezosPeerMessage::Advertise(msg.clone())),
            PeerMessage::SwapRequest(msg) => Some(TezosPeerMessage::SwapRequest(msg.clone())),
            PeerMessage::SwapAck(msg) => Some(TezosPeerMessage::SwapAck(msg.clone())),
            PeerMessage::GetCurrentBranch(msg) => Some(TezosPeerMessage::GetCurrentBranch(msg.clone())),
            PeerMessage::CurrentBranch(msg) => Some(TezosPeerMessage::CurrentBranch(msg.clone())),
            PeerMessage::Deactivate(msg) => Some(TezosPeerMessage::Deactivate(msg.clone())),
            PeerMessage::GetCurrentHead(msg) => Some(TezosPeerMessage::GetCurrentHead(msg.clone())),
            PeerMessage::CurrentHead(msg) => Some(TezosPeerMessage::CurrentHead(msg.clone())),
            PeerMessage::GetBlockHeaders(msg) => Some(TezosPeerMessage::GetBlockHeaders(msg.clone())),
            PeerMessage::BlockHeader(msg) => Some(TezosPeerMessage::BlockHeader(msg.clone())),
            PeerMessage::GetOperations(msg) => Some(TezosPeerMessage::GetOperations(msg.clone())),
            PeerMessage::Operation(msg) => Some(TezosPeerMessage::Operation(msg.clone())),
            PeerMessage::GetProtocols(msg) => Some(TezosPeerMessage::GetProtocols(msg.clone())),
            PeerMessage::Protocol(msg) => Some(TezosPeerMessage::Protocol(msg.clone())),
            PeerMessage::GetOperationHashesForBlocks(msg) => Some(TezosPeerMessage::GetOperationHashesForBlocks(msg.clone())),
            PeerMessage::OperationHashesForBlock(msg) => Some(TezosPeerMessage::OperationHashesForBlock(msg.clone())),
            PeerMessage::GetOperationsForBlocks(msg) => Some(TezosPeerMessage::GetOperationsForBlocks(msg.clone())),
            PeerMessage::OperationsForBlocks(msg) => Some(TezosPeerMessage::OperationsForBlocks(msg.clone())),
            PeerMessage::ConnectionMessage(_) => None,
            PeerMessage::MetadataMessage(_) => None,
            PeerMessage::_Reserved => None,
        }
    }
}

// impl From<AckMessage> for PeerMessage {
//     fn from(value: AckMessage) -> Self {
//         PeerMessage::AckMessage(value)
//     }
// }


#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub enum MappedAckMessage {
    Ack,
    NackV0,
    Nack(MappedNackInfo),
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct MappedNackInfo {
    pub motive: MappedNackMotive,
    pub potential_peers_to_connect: Vec<String>,
}

impl From<NackInfo> for MappedNackInfo {
    fn from(value: NackInfo) -> Self {
        Self {
            motive: value.motive().into(),
            potential_peers_to_connect: value.potential_peers_to_connect().clone(),
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone, Copy)]
pub enum MappedNackMotive {
    NoMotive,
    TooManyConnections,
    UnknownChainName,
    DeprecatedP2pVersion,
    DeprecatedDistributedDbVersion,
    AlreadyConnected,
}

impl From<&NackMotive> for MappedNackMotive {
    fn from(value: &NackMotive) -> Self {
        match value {
            NackMotive::NoMotive => Self::NoMotive,
            NackMotive::TooManyConnections => Self::TooManyConnections,
            NackMotive::UnknownChainName => Self::UnknownChainName,
            NackMotive::DeprecatedP2pVersion => Self::DeprecatedP2pVersion,
            NackMotive::DeprecatedDistributedDbVersion => Self::DeprecatedDistributedDbVersion,
            NackMotive::AlreadyConnected => Self::AlreadyConnected,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MappedOperationsForBlocksMessage {
    operations_for_block: MappedOperationsForBlock,
    operation_hashes_path: Path,
    operations: Vec<MappedOperation>,
}

impl From<OperationsForBlocksMessage> for MappedOperationsForBlocksMessage {
    fn from(value: OperationsForBlocksMessage) -> Self {
        Self {
            operations_for_block: value.operations_for_block().into(),
            operation_hashes_path: value.operation_hashes_path().clone(),
            operations: value.operations().iter().map(|x| MappedOperation::from(x)).collect(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MappedOperation {
    branch: String,
    data: String,
}

impl From<&Operation> for MappedOperation {
    fn from(value: &Operation) -> Self {
        Self {
            branch: HashType::BlockHash.bytes_to_string(value.branch()),
            data: hex::encode(value.data()),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MappedGetOperationsForBlocksMessage {
    get_operations_for_blocks: Vec<MappedOperationsForBlock>
}

impl From<GetOperationsForBlocksMessage> for MappedGetOperationsForBlocksMessage {
    fn from(value: GetOperationsForBlocksMessage) -> Self {
        Self {
            get_operations_for_blocks: value.get_operations_for_blocks().iter().map(|x| MappedOperationsForBlock::from(x)).collect(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MappedOperationsForBlock {
    hash: String,
    validation_pass: i8,
}

impl From<&OperationsForBlock> for MappedOperationsForBlock {
    fn from(value: &OperationsForBlock) -> Self {
        Self {
            hash: HashType::BlockHash.bytes_to_string(value.hash()),
            validation_pass: value.validation_pass().clone(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MappedOperationHashesForBlocksMessage {
    operation_hashes_for_block: MappedOperationHashesForBlock,
    operation_hashes_path: Path,
    operation_hashes: Vec<String>,
}

impl From<OperationHashesForBlocksMessage> for MappedOperationHashesForBlocksMessage {
    fn from(value: OperationHashesForBlocksMessage) -> Self {
        Self {
            operation_hashes_for_block: value.operation_hashes_for_block().into(),
            operation_hashes_path: value.operation_hashes_path().clone(),
            operation_hashes: value.operation_hashes().iter().map(|x| HashType::OperationHash.bytes_to_string(x)).collect(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MappedGetOperationHashesForBlocksMessage {
    get_operation_hashes_for_blocks: Vec<MappedOperationHashesForBlock>
}

impl From<GetOperationHashesForBlocksMessage> for MappedGetOperationHashesForBlocksMessage {
    fn from(value: GetOperationHashesForBlocksMessage) -> Self {
        Self {
            get_operation_hashes_for_blocks: value.get_operation_hashes_for_blocks().iter()
                .map(|x| MappedOperationHashesForBlock::from(x)).collect(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MappedOperationHashesForBlock {
    hash: String,
    validation_pass: i8,
}

impl From<&OperationHashesForBlock> for MappedOperationHashesForBlock {
    fn from(value: &OperationHashesForBlock) -> Self {
        Self {
            hash: HashType::BlockHash.bytes_to_string(value.hash()),
            validation_pass: value.validation_pass(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MappedGetProtocolsMessage {
    get_protocols: Vec<String>,
}

impl From<GetProtocolsMessage> for MappedGetProtocolsMessage {
    fn from(value: GetProtocolsMessage) -> Self {
        let mut json = serde_json::to_value(value).unwrap();
        let protos: Vec<Vec<u8>> = serde_json::from_value(json.get_mut("get_protocols").unwrap().take()).unwrap();
        Self {
            get_protocols: protos.iter().map(|x| HashType::ProtocolHash.bytes_to_string(x)).collect(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MappedOperationMessage {
    branch: String,
    data: String,
}

impl From<OperationMessage> for MappedOperationMessage {
    fn from(value: OperationMessage) -> Self {
        let mut json = serde_json::to_value(value).unwrap();
        let operation: Operation = serde_json::from_value(json.get_mut("operation").unwrap().take()).unwrap();
        Self {
            branch: HashType::BlockHash.bytes_to_string(operation.branch()),
            data: hex::encode(operation.data()),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MappedGetOperationsMessage {
    get_operations: Vec<String>
}

impl From<GetOperationsMessage> for MappedGetOperationsMessage {
    fn from(value: GetOperationsMessage) -> Self {
        let mut json = serde_json::to_value(value).unwrap();
        let ops: Vec<Vec<u8>> = serde_json::from_value(json.get_mut("get_operations").unwrap().take()).unwrap();
        Self {
            get_operations: ops.iter()
                .map(|x| HashType::OperationHash.bytes_to_string(x)).collect(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MappedBlockHeaderMessage {
    block_header: MappedBlockHeader,
}

impl From<BlockHeaderMessage> for MappedBlockHeaderMessage {
    fn from(value: BlockHeaderMessage) -> Self {
        Self {
            block_header: value.block_header().clone().into()
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MappedGetBlockHeadersMessage {
    get_block_headers: Vec<String>
}

impl From<GetBlockHeadersMessage> for MappedGetBlockHeadersMessage {
    fn from(value: GetBlockHeadersMessage) -> Self {
        Self {
            get_block_headers: value.get_block_headers().iter().map(|x| HashType::BlockHash.bytes_to_string(x)).collect(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MappedCurrentHeadMessage {
    chain_id: String,
    current_block_header: MappedBlockHeader,
    current_mempool: MappedMempool,
}

impl From<CurrentHeadMessage> for MappedCurrentHeadMessage {
    fn from(value: CurrentHeadMessage) -> Self {
        Self {
            chain_id: HashType::ChainId.bytes_to_string(value.chain_id()),
            current_block_header: value.current_block_header().clone().into(),
            current_mempool: value.current_mempool().into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MappedMempool {
    known_valid: Vec<String>,
    pending: Vec<String>,
}

impl From<&Mempool> for MappedMempool {
    fn from(value: &Mempool) -> Self {
        Self {
            known_valid: value.known_valid().iter().map(|x| HashType::OperationHash.bytes_to_string(x)).collect(),
            pending: value.pending().iter().map(|x| HashType::OperationHash.bytes_to_string(x)).collect(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MappedGetCurrentHeadMessage {
    chain_id: String
}

impl From<GetCurrentHeadMessage> for MappedGetCurrentHeadMessage {
    fn from(value: GetCurrentHeadMessage) -> Self {
        Self {
            chain_id: HashType::ChainId.bytes_to_string(value.chain_id()),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MappedDeactivateMessage {
    deactivate: String,
}

impl From<DeactivateMessage> for MappedDeactivateMessage {
    fn from(value: DeactivateMessage) -> Self {
        Self {
            deactivate: HashType::ChainId.bytes_to_string(&value.deactivate()),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MappedCurrentBranchMessage {
    chain_id: String,
    current_branch: MappedCurrentBranch,
}

impl From<CurrentBranchMessage> for MappedCurrentBranchMessage {
    fn from(value: CurrentBranchMessage) -> Self {
        Self {
            chain_id: HashType::ChainId.bytes_to_string(value.chain_id()),
            current_branch: value.current_branch().clone().into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MappedCurrentBranch {
    current_head: MappedBlockHeader,
    history: Vec<String>,
}

impl From<CurrentBranch> for MappedCurrentBranch {
    fn from(value: CurrentBranch) -> Self {
        Self {
            current_head: value.current_head().clone().into(),
            history: value.history().iter().map(|x| HashType::BlockHash.bytes_to_string(x)).collect(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MappedBlockHeader {
    level: i32,
    proto: u8,
    predecessor: String,
    timestamp: i64,
    validation_pass: u8,
    operations_hash: String,
    fitness: Vec<Vec<u8>>,
    context: String,
    protocol_data: String,
}

impl From<BlockHeader> for MappedBlockHeader {
    fn from(value: BlockHeader) -> Self {
        Self {
            level: value.level().clone(),
            proto: value.proto().clone(),
            timestamp: value.timestamp().clone(),
            validation_pass: value.validation_pass().clone(),
            fitness: value.fitness().clone(),
            context: HashType::ContextHash.bytes_to_string(value.context()),
            operations_hash: HashType::OperationListListHash.bytes_to_string(value.operations_hash()),
            predecessor: HashType::BlockHash.bytes_to_string(value.predecessor()),
            protocol_data: hex::encode(value.protocol_data()),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MappedGetCurrentBranchMessage {
    pub chain_id: String
}

impl From<GetCurrentBranchMessage> for MappedGetCurrentBranchMessage {
    fn from(value: GetCurrentBranchMessage) -> Self {
        Self {
            chain_id: HashType::ChainId.bytes_to_string(&value.chain_id),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MappedConnectionMessage {
    pub port: u16,
    pub versions: Vec<Version>,
    pub public_key: String,
    pub proof_of_work_stamp: String,
    pub message_nonce: String,
}

impl From<ConnectionMessage> for MappedConnectionMessage {
    fn from(value: ConnectionMessage) -> Self {
        Self {
            port: value.port,
            versions: value.versions,
            public_key: HashType::CryptoboxPublicKeyHash.bytes_to_string(&value.public_key),
            proof_of_work_stamp: hex::encode(value.proof_of_work_stamp),
            message_nonce: hex::encode(value.message_nonce),
        }
    }
}

