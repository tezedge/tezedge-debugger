// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

/// Defines RpcMessage type, used only by Proxy RPC endpoints, maps directly to the StoreMessage

use super::StoreMessage;
use crate::network::connection_message::ConnectionMessage;
use serde::{Serialize, Deserialize};
use std::net::SocketAddr;
use tezos_messages::p2p::encoding::prelude::*;
use tezos_messages::p2p::encoding::version::Version;
use crypto::hash::HashType;
use tezos_messages::p2p::encoding::operation_hashes_for_blocks::OperationHashesForBlock;
use storage::persistent::BincodeEncoded;
use crate::storage::RESTMessage;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RpcMessage {
    Packet {
        incoming: bool,
        timestamp: u128,
        id: u64,
        remote_addr: SocketAddr,
        packet: String,
    },
    Metadata {
        incoming: bool,
        timestamp: u128,
        id: u64,
        remote_addr: SocketAddr,
        message: MetadataMessage,
    },
    ConnectionMessage {
        incoming: bool,
        timestamp: u128,
        id: u64,
        remote_addr: SocketAddr,
        message: MappedConnectionMessage,
    },
    P2pMessage {
        incoming: bool,
        timestamp: u128,
        id: u64,
        remote_addr: SocketAddr,
        request_id: Option<u64>,
        message: Vec<MappedPeerMessage>,
    },
    RestMessage {
        incoming: bool,
        timestamp: u128,
        id: u64,
        remote_addr: SocketAddr,
        message: MappedRESTMessage,
    },
}

impl BincodeEncoded for RpcMessage {}

impl RpcMessage {
    pub fn from_store(msg: &StoreMessage, id: u64) -> Self {
        match msg {
            StoreMessage::TcpMessage { remote_addr, incoming, packet, timestamp } => {
                RpcMessage::Packet {
                    id,
                    timestamp: timestamp.clone(),
                    remote_addr: remote_addr.clone(),
                    incoming: incoming.clone(),
                    packet: hex::encode(packet),
                }
            }
            StoreMessage::ConnectionMessage { remote_addr, incoming, payload, timestamp } => {
                RpcMessage::ConnectionMessage {
                    id,
                    timestamp: timestamp.clone(),
                    remote_addr: remote_addr.clone(),
                    incoming: incoming.clone(),
                    message: payload.clone().into(),
                }
            }
            StoreMessage::P2PMessage { remote_addr, incoming, payload, request_id, timestamp } => {
                RpcMessage::P2pMessage {
                    id,
                    request_id: request_id.clone(),
                    timestamp: timestamp.clone(),
                    remote_addr: remote_addr.clone(),
                    incoming: incoming.clone(),
                    message: payload.into_iter().map(|x| MappedPeerMessage::from(x.clone())).collect(),
                }
            }
            StoreMessage::RestMessage { remote_addr, incoming, payload, timestamp } => {
                RpcMessage::RestMessage {
                    id,
                    timestamp: timestamp.clone(),
                    remote_addr: remote_addr.clone(),
                    incoming: incoming.clone(),
                    message: payload.clone().into(),
                }
            }
            StoreMessage::Metadata { remote_addr, incoming, message, timestamp } => {
                RpcMessage::Metadata {
                    id,
                    timestamp: timestamp.clone(),
                    remote_addr: remote_addr.clone(),
                    incoming: incoming.clone(),
                    message: message.clone(),
                }
            }
        }
    }

    fn fix_id(id: u64) -> u64 {
        std::u64::MAX.saturating_sub(id)
    }

    pub fn id(&self) -> u64 {
        match self {
            Self::Packet { id, .. } => id.clone(),
            Self::Metadata { id, .. } => id.clone(),
            Self::ConnectionMessage { id, .. } => id.clone(),
            Self::P2pMessage { id, .. } => id.clone(),
            Self::RestMessage { id, .. } => id.clone(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MappedRESTMessage {
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

impl From<RESTMessage> for MappedRESTMessage {
    fn from(value: RESTMessage) -> Self {
        match value {
            RESTMessage::Response { status, payload } => Self::Response { status, payload },
            RESTMessage::Request { method, path, payload } => Self::Request { method, path, payload },
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MappedConnectionMessage {
    pub port: u16,
    pub versions: Vec<Version>,
    pub public_key: String,
    pub proof_of_work_stamp: String,
    pub message_nonce: String,
}

impl From<ConnectionMessage> for MappedConnectionMessage {
    fn from(value: ConnectionMessage) -> Self {
        let ConnectionMessage { port, versions, public_key, proof_of_work_stamp, message_nonce, .. } = value;

        Self {
            port,
            versions,
            public_key: hex::encode(public_key),
            proof_of_work_stamp: hex::encode(proof_of_work_stamp),
            message_nonce: hex::encode(message_nonce),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MappedPeerMessage {
    Disconnect,
    Bootstrap,
    Advertise(AdvertiseMessage),
    SwapRequest(SwapMessage),
    SwapAck(SwapMessage),
    GetCurrentBranch(MappedGetCurrentBranchMessage),
    CurrentBranch(MappedCurrentBranchMessage),
    Deactivate(MappedDeactivateMessage),
    GetCurrentHead(MappedGetCurrentHeadMessage),
    CurrentHead(MappedCurrentHeadMessage),
    GetBlockHeaders(MappedGetBlockHeadersMessage),
    BlockHeader(MappedBlockHeaderMessage),
    GetOperations(MappedGetOperationsMessage),
    Operation(MappedOperationMessage),
    GetProtocols(MappedGetProtocolsMessage),
    Protocol(ProtocolMessage),
    GetOperationHashesForBlocks(MappedGetOperationHashesForBlocksMessage),
    OperationHashesForBlock(MappedOperationHashesForBlocksMessage),
    GetOperationsForBlocks(MappedGetOperationsForBlocksMessage),
    OperationsForBlocks(MappedOperationsForBlocksMessage),
    Dummy,
}

impl From<PeerMessage> for MappedPeerMessage {
    fn from(value: PeerMessage) -> Self {
        match value {
            PeerMessage::Disconnect => MappedPeerMessage::Disconnect,
            PeerMessage::Bootstrap => MappedPeerMessage::Bootstrap,
            PeerMessage::Advertise(msg) => MappedPeerMessage::Advertise(msg),
            PeerMessage::SwapRequest(msg) => MappedPeerMessage::SwapRequest(msg),
            PeerMessage::SwapAck(msg) => MappedPeerMessage::SwapAck(msg),
            PeerMessage::GetCurrentBranch(msg) => MappedPeerMessage::GetCurrentBranch(msg.into()),
            PeerMessage::CurrentBranch(msg) => MappedPeerMessage::CurrentBranch(msg.into()),
            PeerMessage::Deactivate(msg) => MappedPeerMessage::Deactivate(msg.into()),
            PeerMessage::GetCurrentHead(msg) => MappedPeerMessage::GetCurrentHead(msg.into()),
            PeerMessage::CurrentHead(msg) => MappedPeerMessage::CurrentHead(msg.into()),
            PeerMessage::GetBlockHeaders(msg) => MappedPeerMessage::GetBlockHeaders(msg.into()),
            PeerMessage::BlockHeader(msg) => MappedPeerMessage::BlockHeader(msg.into()),
            PeerMessage::GetOperations(msg) => MappedPeerMessage::GetOperations(msg.into()),
            PeerMessage::Operation(msg) => MappedPeerMessage::Operation(msg.into()),
            PeerMessage::GetProtocols(msg) => MappedPeerMessage::GetProtocols(msg.into()),
            PeerMessage::Protocol(msg) => MappedPeerMessage::Protocol(msg.into()),
            PeerMessage::GetOperationHashesForBlocks(msg) => MappedPeerMessage::GetOperationHashesForBlocks(msg.into()),
            PeerMessage::OperationHashesForBlock(msg) => MappedPeerMessage::OperationHashesForBlock(msg.into()),
            PeerMessage::GetOperationsForBlocks(msg) => MappedPeerMessage::GetOperationsForBlocks(msg.into()),
            PeerMessage::OperationsForBlocks(msg) => MappedPeerMessage::OperationsForBlocks(msg.into()),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
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
