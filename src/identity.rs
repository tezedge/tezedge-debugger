// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::fs;
use std::io;
use std::path::Path;

use failure::Fail;
use serde::{Deserialize, Serialize};

/// This node identity information
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Identity {
    pub peer_id: String,
    pub public_key: String,
    pub secret_key: String,
    pub proof_of_work_stamp: String,
}

#[derive(Fail, Debug)]
pub enum IdentityError {
    #[fail(display = "I/O error: {}", reason)]
    IoError {
        reason: io::Error
    },
    #[fail(display = "Identity serialization error: {}", reason)]
    SerializationError {
        reason: serde_json::Error
    },
    #[fail(display = "Identity de-serialization error: {}", reason)]
    DeserializationError {
        reason: serde_json::Error
    },
}

impl From<io::Error> for IdentityError {
    fn from(reason: io::Error) -> Self {
        IdentityError::IoError { reason }
    }
}

/// Load identity from tezos configuration file.
pub fn load_identity<P: AsRef<Path>>(identity_json_file_path: P) -> Result<Identity, IdentityError> {
    let identity = fs::read_to_string(identity_json_file_path)
        .map(|contents| serde_json::from_str::<Identity>(&contents).map_err(|err| IdentityError::DeserializationError { reason: err }))??;
    Ok(identity)
}