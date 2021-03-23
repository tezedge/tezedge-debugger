// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Serialize, Deserialize};

/// Determines, if message belongs to communication originated
/// from remote or local node
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Initiator {
    Local,
    Remote,
}

/// Determines, if message itself originated
/// from remote or local node
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sender {
    Local,
    Remote,
}

impl Sender {
    pub fn incoming(&self) -> bool {
        match self {
            &Sender::Local => false,
            &Sender::Remote => true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageCategory {
    Connection,
    Meta,
    Ack,
    P2p,
}
