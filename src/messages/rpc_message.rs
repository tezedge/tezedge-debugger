// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Serialize, Deserialize};
use storage::persistent::{Encoder, SchemaError, Decoder};
use std::net::SocketAddr;

#[derive(Debug, Serialize, Deserialize, Clone)]
/// Rpc message as stored in the database
pub struct RpcMessage {
    pub incoming: bool,
    pub timestamp: u128,
    pub id: u64,
    pub remote_addr: SocketAddr,
    pub message: RESTMessage,
}

impl Decoder for RpcMessage {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        serde_cbor::from_slice(bytes)
            .map_err(|_| SchemaError::DecodeError)
    }
}

impl Encoder for RpcMessage {
    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        serde_cbor::to_vec(self)
            .map_err(|_| SchemaError::EncodeError)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
/// Detailed representation of REST messages
pub enum RESTMessage {
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

impl Decoder for RESTMessage {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        serde_cbor::from_slice(bytes)
            .map_err(|_| SchemaError::DecodeError)
    }
}

impl Encoder for RESTMessage {
    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        serde_cbor::to_vec(self)
            .map_err(|_| SchemaError::EncodeError)
    }
}