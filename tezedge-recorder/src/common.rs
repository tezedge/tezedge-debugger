// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::fmt;
use serde::{Serialize, Deserialize};

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
    None,
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
        match self {
            MessageKind::Unknown | MessageKind::None => false,
            _ => true,
        }
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
