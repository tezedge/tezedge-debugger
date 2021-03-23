// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Serialize, Deserialize};
use super::common::Sender;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    connection: u64,
    number: u64,
    sender: Sender,
    bytes: Vec<u8>,
    plain: Vec<u8>,
    error: Option<String>,
}

impl Item {
    pub fn new(connection: u64, number: u64, sender: Sender, bytes: Vec<u8>, plain: Vec<u8>) -> Self {
        Item {
            connection,
            number,
            sender,
            bytes,
            plain,
            error: None,
        }
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }
}
