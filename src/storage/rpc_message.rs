use super::StoreMessage;
use serde::Serialize;
use std::net::IpAddr;
use tezos_messages::p2p::encoding::prelude::*;
use tezos_messages::p2p::encoding::{
    version::Version,
    connection::ConnectionMessage,
};
use crypto::hash::HashType;
use tezos_messages::p2p::encoding::operation_hashes_for_blocks::OperationHashesForBlock;


#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
/// Types of messages sent by external RPC, directly maps to the StoreMessage, with different naming
pub enum RpcMessage {
    Packet {
        source: IpAddr,
        destination: IpAddr,
        packet: String,
    },
    ConnectionMessage {
        source: IpAddr,
        destination: IpAddr,
        message: MappedConnectionMessage,
    },
    P2pMessage {
        source: IpAddr,
        destination: IpAddr,
        messages: Vec<MappedPeerMessage>,
    },
}

impl From<StoreMessage> for RpcMessage {
    fn from(value: StoreMessage) -> Self {
        match value {
            StoreMessage::TcpMessage { source, destination, packet } => {
                RpcMessage::Packet {
                    source,
                    destination,
                    packet: hex::encode(packet),
                }
            }
            StoreMessage::ConnectionMessage { source, destination, payload } => {
                RpcMessage::ConnectionMessage {
                    source,
                    destination,
                    message: payload.into(),
                }
            }
            StoreMessage::P2PMessage { source, destination, payload } => {
                RpcMessage::P2pMessage {
                    source,
                    destination,
                    messages: payload.into_iter().map(|x| MappedPeerMessage::from(x)).collect(),
                }
            }
        }
    }
}

#[derive(Debug, Serialize)]
pub struct MappedConnectionMessage {
    pub port: u16,
    pub versions: Vec<Version>,
    pub public_key: String,
    pub proof_of_work_stamp: String,
    pub message_nonce: String,
}

impl From<ConnectionMessage> for MappedConnectionMessage {
    fn from(value: ConnectionMessage) -> Self {
        let mut json = serde_json::to_value(value)
            .expect("failed to serialized valid value");
        let port = json.get_mut("port").unwrap().as_i64().unwrap() as u16;
        let versions = serde_json::from_value(json.get_mut("versions").unwrap().take())
            .unwrap();
        let public_key: Vec<u8> = serde_json::from_value(json.get_mut("public_key").unwrap().take())
            .unwrap();
        let public_key = hex::encode(public_key);
        let poows: Vec<u8> = serde_json::from_value(json.get_mut("proof_of_work_stamp").unwrap().take())
            .unwrap();
        let proof_of_work_stamp = hex::encode(poows);
        let message_nonce: Vec<u8> = serde_json::from_value(json.get_mut("message_nonce").unwrap().take())
            .unwrap();
        let message_nonce = hex::encode(message_nonce);
        Self {
            port,
            versions,
            public_key,
            proof_of_work_stamp,
            message_nonce,
        }
    }
}

#[derive(Debug, Serialize)]
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

#[derive(Debug, Serialize)]
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

#[derive(Debug, Serialize)]
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

#[derive(Debug, Serialize)]
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

#[derive(Debug, Serialize)]
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

#[derive(Debug, Serialize)]
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

#[derive(Debug, Serialize)]
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

#[derive(Debug, Serialize)]
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

#[derive(Debug, Serialize)]
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

#[derive(Debug, Serialize)]
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

#[derive(Debug, Serialize)]
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

#[derive(Debug, Serialize)]
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

#[derive(Debug, Serialize)]
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

#[derive(Debug, Serialize)]
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

#[derive(Debug, Serialize)]
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

#[derive(Debug, Serialize)]
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

#[derive(Debug, Serialize)]
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

#[derive(Debug, Serialize)]
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

#[derive(Debug, Serialize)]
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

#[derive(Debug, Serialize)]
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

#[derive(Debug, Serialize)]
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
