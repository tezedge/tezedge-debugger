// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{fmt, str::FromStr};
use serde::{Serialize, Deserialize};
use thiserror::Error;

pub type Local = typenum::B0;
pub type Remote = typenum::B1;

/// Determines, if message belongs to communication originated
/// from remote or local node
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Initiator {
    Local,
    Remote,
}

impl Initiator {
    pub fn new(incoming: bool) -> Self {
        if incoming {
            Initiator::Remote
        } else {
            Initiator::Local
        }
    }

    pub fn incoming(&self) -> bool {
        match self {
            Initiator::Local => false,
            Initiator::Remote => true,
        }
    }
}

/// Determines, if message itself originated
/// from remote or local node
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sender {
    Local,
    Remote,
}

#[derive(Error, Debug)]
#[error("Invalid message type {}", _0)]
pub struct ParseSenderError(String);

impl FromStr for Sender {
    type Err = ParseSenderError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "local" => Ok(Sender::Local),
            "remote" => Ok(Sender::Remote),
            s => Err(ParseSenderError(s.to_string())),
        }
    }
}

impl fmt::Display for Sender {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Sender::Local => write!(f, "local"),
            Sender::Remote => write!(f, "remote"),
        }
    }
}

impl Default for Sender {
    fn default() -> Self {
        Sender::Remote
    }
}

impl Sender {
    pub fn new(incoming: bool) -> Self {
        if incoming {
            Sender::Remote
        } else {
            Sender::Local
        }
    }

    pub fn incoming(&self) -> bool {
        match self {
            Sender::Local => false,
            Sender::Remote => true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageKind {
    // 0x0X
    Disconnect,
    Bootstrap,
    Advertise,
    SwapRequest,
    SwapAck,
    // 0x1X
    GetCurrentBranch,
    CurrentBranch,
    Deactivate,
    GetCurrentHead,
    CurrentHead,
    // 0x2X
    GetBlockHeaders,
    BlockHeader,
    // 0x3X
    GetOperations,
    Operation,
    // 0x4X
    GetProtocols,
    Protocol,
    // 0x5X
    GetOperationHashesForBlocks,
    OperationHashesForBlocks,
    // 0x6X
    GetOperationsForBlocks,
    OperationsForBlocks,
    // 0xXXXX
    Unknown,
}

impl MessageKind {
    pub fn from_tag(tag: u16) -> Self {
        match tag {
            0x01 => MessageKind::Disconnect,
            0x02 => MessageKind::Bootstrap,
            0x03 => MessageKind::Advertise,
            0x04 => MessageKind::SwapRequest,
            0x05 => MessageKind::SwapAck,

            0x10 => MessageKind::GetCurrentBranch,
            0x11 => MessageKind::CurrentBranch,
            0x12 => MessageKind::Deactivate,
            0x13 => MessageKind::GetCurrentHead,
            0x14 => MessageKind::CurrentHead,

            0x20 => MessageKind::GetBlockHeaders,
            0x21 => MessageKind::BlockHeader,

            0x30 => MessageKind::GetOperations,
            0x31 => MessageKind::Operation,

            0x40 => MessageKind::GetProtocols,
            0x41 => MessageKind::Protocol,

            0x50 => MessageKind::GetOperationHashesForBlocks,
            0x51 => MessageKind::OperationHashesForBlocks,

            0x60 => MessageKind::GetOperationsForBlocks,
            0x61 => MessageKind::OperationsForBlocks,

            _ => MessageKind::Unknown,
        }
    }

    #[allow(dead_code)]
    pub fn valid_tag(&self) -> bool {
        !matches!(self, MessageKind::Unknown)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageCategory {
    Connection,
    Meta,
    Ack,
    P2p,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageType {
    Connection,
    Meta,
    Ack,
    P2p(MessageKind),
}

#[derive(Error, Debug)]
#[error("Invalid message type {}", _0)]
pub struct ParseTypeError(String);

impl FromStr for MessageType {
    type Err = ParseTypeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "connection_message" => Ok(MessageType::Connection),
            "metadata" => Ok(MessageType::Meta),
            "ack_message" => Ok(MessageType::Ack),

            "disconnect" => Ok(MessageType::P2p(MessageKind::Disconnect)),
            "advertise" => Ok(MessageType::P2p(MessageKind::Advertise)),
            "swap_request" => Ok(MessageType::P2p(MessageKind::SwapRequest)),
            "swap_ack" => Ok(MessageType::P2p(MessageKind::SwapAck)),
            "bootstrap" => Ok(MessageType::P2p(MessageKind::Bootstrap)),
            "get_current_branch" => Ok(MessageType::P2p(MessageKind::GetCurrentBranch)),
            "current_branch" => Ok(MessageType::P2p(MessageKind::CurrentBranch)),
            "deactivate" => Ok(MessageType::P2p(MessageKind::Deactivate)),
            "get_current_head" => Ok(MessageType::P2p(MessageKind::GetCurrentHead)),
            "current_head" => Ok(MessageType::P2p(MessageKind::CurrentHead)),
            "get_block_headers" => Ok(MessageType::P2p(MessageKind::GetBlockHeaders)),
            "block_header" => Ok(MessageType::P2p(MessageKind::BlockHeader)),
            "get_operations" => Ok(MessageType::P2p(MessageKind::GetOperations)),
            "operation" => Ok(MessageType::P2p(MessageKind::Operation)),
            "get_protocols" => Ok(MessageType::P2p(MessageKind::GetProtocols)),
            "protocol" => Ok(MessageType::P2p(MessageKind::Protocol)),
            "get_operation_hashes_for_blocks" => {
                Ok(MessageType::P2p(MessageKind::GetOperationHashesForBlocks))
            },
            "operation_hashes_for_block" => {
                Ok(MessageType::P2p(MessageKind::OperationHashesForBlocks))
            },
            "get_operations_for_blocks" => {
                Ok(MessageType::P2p(MessageKind::GetOperationsForBlocks))
            },
            "operations_for_blocks" => Ok(MessageType::P2p(MessageKind::OperationsForBlocks)),

            s => Err(ParseTypeError(s.to_string())),
        }
    }
}

impl MessageType {
    pub fn split(self) -> (MessageCategory, Option<MessageKind>) {
        match self {
            MessageType::Connection => (MessageCategory::Connection, None),
            MessageType::Meta => (MessageCategory::Meta, None),
            MessageType::Ack => (MessageCategory::Ack, None),
            MessageType::P2p(kind) => (MessageCategory::P2p, Some(kind)),
        }
    }

    pub fn from_int(v: u8) -> Self {
        match v {
            0x00 => MessageType::Connection,
            0x01 => MessageType::Meta,
            0x02 => MessageType::Ack,
            0x03 => MessageType::P2p(MessageKind::Disconnect),
            0x04 => MessageType::P2p(MessageKind::Bootstrap),
            0x05 => MessageType::P2p(MessageKind::Advertise),
            0x06 => MessageType::P2p(MessageKind::SwapRequest),
            0x07 => MessageType::P2p(MessageKind::SwapAck),
            0x08 => MessageType::P2p(MessageKind::GetCurrentBranch),
            0x09 => MessageType::P2p(MessageKind::CurrentBranch),
            0x0a => MessageType::P2p(MessageKind::Deactivate),
            0x0b => MessageType::P2p(MessageKind::GetCurrentHead),
            0x0c => MessageType::P2p(MessageKind::CurrentHead),
            0x0d => MessageType::P2p(MessageKind::GetBlockHeaders),
            0x0e => MessageType::P2p(MessageKind::BlockHeader),
            0x0f => MessageType::P2p(MessageKind::GetOperations),
            0x10 => MessageType::P2p(MessageKind::Operation),
            0x11 => MessageType::P2p(MessageKind::GetProtocols),
            0x12 => MessageType::P2p(MessageKind::Protocol),
            0x13 => MessageType::P2p(MessageKind::GetOperationHashesForBlocks),
            0x14 => MessageType::P2p(MessageKind::OperationHashesForBlocks),
            0x15 => MessageType::P2p(MessageKind::GetOperationsForBlocks),
            0x16 => MessageType::P2p(MessageKind::OperationsForBlocks),
            _ => MessageType::P2p(MessageKind::Unknown),
        }
    }

    pub fn into_int(self) -> u8 {
        match self {
            MessageType::Connection => 0x00,
            MessageType::Meta => 0x01,
            MessageType::Ack => 0x02,
            MessageType::P2p(MessageKind::Disconnect) => 0x03,
            MessageType::P2p(MessageKind::Bootstrap) => 0x04,
            MessageType::P2p(MessageKind::Advertise) => 0x05,
            MessageType::P2p(MessageKind::SwapRequest) => 0x06,
            MessageType::P2p(MessageKind::SwapAck) => 0x07,
            MessageType::P2p(MessageKind::GetCurrentBranch) => 0x08,
            MessageType::P2p(MessageKind::CurrentBranch) => 0x09,
            MessageType::P2p(MessageKind::Deactivate) => 0x0a,
            MessageType::P2p(MessageKind::GetCurrentHead) => 0x0b,
            MessageType::P2p(MessageKind::CurrentHead) => 0x0c,
            MessageType::P2p(MessageKind::GetBlockHeaders) => 0x0d,
            MessageType::P2p(MessageKind::BlockHeader) => 0x0e,
            MessageType::P2p(MessageKind::GetOperations) => 0x0f,
            MessageType::P2p(MessageKind::Operation) => 0x10,
            MessageType::P2p(MessageKind::GetProtocols) => 0x11,
            MessageType::P2p(MessageKind::Protocol) => 0x12,
            MessageType::P2p(MessageKind::GetOperationHashesForBlocks) => 0x13,
            MessageType::P2p(MessageKind::OperationHashesForBlocks) => 0x14,
            MessageType::P2p(MessageKind::GetOperationsForBlocks) => 0x15,
            MessageType::P2p(MessageKind::OperationsForBlocks) => 0x16,
            MessageType::P2p(MessageKind::Unknown) => 0xff,
        }
    }
}
