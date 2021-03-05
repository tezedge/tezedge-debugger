use std::str::FromStr;
use failure::Fail;
use storage::persistent::{KeyValueSchema, Decoder, SchemaError, Encoder};
use super::{FilterField, Access};

#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum P2pType {
    // Base Types
    Tcp = 0x1 << 0,
    Metadata = 0x1 << 1,
    ConnectionMessage = 0x1 << 2,
    RestMessage = 0x1 << 3,
    // P2P messages
    P2PMessage = 0x1 << 4,
    Disconnect = 0x1 << 5,
    Advertise = 0x1 << 6,
    SwapRequest = 0x1 << 7,
    SwapAck = 0x1 << 8,
    Bootstrap = 0x1 << 9,
    GetCurrentBranch = 0x1 << 10,
    CurrentBranch = 0x1 << 11,
    Deactivate = 0x1 << 12,
    GetCurrentHead = 0x1 << 13,
    CurrentHead = 0x1 << 14,
    GetBlockHeaders = 0x1 << 15,
    BlockHeader = 0x1 << 16,
    GetOperations = 0x1 << 17,
    Operation = 0x1 << 18,
    GetProtocols = 0x1 << 19,
    Protocol = 0x1 << 20,
    GetOperationHashesForBlocks = 0x1 << 21,
    OperationHashesForBlock = 0x1 << 22,
    GetOperationsForBlocks = 0x1 << 23,
    OperationsForBlocks = 0x1 << 24,
    AckMessage = 0x1 << 25,
}

#[derive(Debug, Fail)]
#[fail(display = "Invalid message type {}", _0)]
pub struct ParseTypeError(String);

impl FromStr for P2pType {
    type Err = ParseTypeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "tcp" => Ok(Self::Tcp),
            "metadata" => Ok(Self::Metadata),
            "connection_message" => Ok(Self::ConnectionMessage),
            "rest_message" => Ok(Self::RestMessage),
            "p2p_message" => Ok(Self::P2PMessage),
            "disconnect" => Ok(Self::Disconnect),
            "advertise" => Ok(Self::Advertise),
            "swap_request" => Ok(Self::SwapRequest),
            "swap_ack" => Ok(Self::SwapAck),
            "bootstrap" => Ok(Self::Bootstrap),
            "get_current_branch" => Ok(Self::GetCurrentBranch),
            "current_branch" => Ok(Self::CurrentBranch),
            "deactivate" => Ok(Self::Deactivate),
            "get_current_head" => Ok(Self::GetCurrentHead),
            "current_head" => Ok(Self::CurrentHead),
            "get_block_headers" => Ok(Self::GetBlockHeaders),
            "block_header" => Ok(Self::BlockHeader),
            "get_operations" => Ok(Self::GetOperations),
            "operation" => Ok(Self::Operation),
            "get_protocols" => Ok(Self::GetProtocols),
            "protocol" => Ok(Self::Protocol),
            "get_operation_hashes_for_blocks" => Ok(Self::GetOperationHashesForBlocks),
            "operation_hashes_for_block" => Ok(Self::OperationHashesForBlock),
            "get_operations_for_blocks" => Ok(Self::GetOperationsForBlocks),
            "operations_for_blocks" => Ok(Self::OperationsForBlocks),
            "ack_message" => Ok(Self::AckMessage),
            s => Err(ParseTypeError(s.to_string())),
        }
    }
}

impl<Schema> FilterField<Schema> for P2pType
where
    Schema: KeyValueSchema<Key = u64>,
    Schema::Value: Access<P2pType>,
{
    type Key = P2pTypeKey;

    fn accessor(value: &<Schema as KeyValueSchema>::Value) -> Option<Self> {
        Some(value.accessor())
    }

    fn make_index(&self, primary_key: &<Schema as KeyValueSchema>::Key) -> Self::Key {
        P2pTypeKey {
            ty: *self as u32,
            index: primary_key.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct P2pTypeKey {
    ty: u32,
    index: u64,
}

/// * bytes layout: `[type(4)][padding(4)][index(8)]`
impl Decoder for P2pTypeKey {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        if bytes.len() != 16 {
            return Err(SchemaError::DecodeError);
        }

        Ok(P2pTypeKey {
            ty: {
                let mut b = [0; 4];
                b.clone_from_slice(&bytes[0..4]);
                u32::from_be_bytes(b)
            },
            index: {
                let mut b = [0; 8];
                b.clone_from_slice(&bytes[8..16]);
                u64::from_be_bytes(b)
            },
        })
    }
}

/// * bytes layout: `[type(4)][padding(4)][index(8)]`
impl Encoder for P2pTypeKey {
    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        let mut buf: Vec<u8> = Vec::with_capacity(16);
        buf.extend_from_slice(&self.ty.to_be_bytes());
        buf.extend_from_slice(&[0, 0, 0, 0]);
        buf.extend_from_slice(&self.index.to_be_bytes());

        if buf.len() != 16 {
            println!("{:?} - {:?}", self, buf);
            Err(SchemaError::EncodeError)
        } else {
            Ok(buf)
        }
    }
}
