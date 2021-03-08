use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Serialize, Deserialize};
use super::compare::PeerMetadata;
use crate::storage_::indices::Initiator;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    last_updated_timestamp: u128,
    total_chunks: u64,
    decrypted_chunks: u64,
    closed_connections: Vec<ConnectionReport>,
    working_connections: Vec<ConnectionReport>,
}

impl Report {
    pub fn prepare(closed_connections: Vec<ConnectionReport>, working_connections: Vec<ConnectionReport>) -> Self {
        let total_chunks =
            working_connections.iter().map(|report| report.total_chunks).sum::<u64>() +
            closed_connections.iter().map(|report| report.total_chunks).sum::<u64>();
        let decrypted_chunks =
            working_connections.iter().map(|report| report.decrypted_chunks).sum::<u64>() +
            closed_connections.iter().map(|report| report.decrypted_chunks).sum::<u64>();
        Report {
            last_updated_timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos(),
            total_chunks,
            decrypted_chunks,
            closed_connections,
            working_connections,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionReport {
    pub remote_address: String,
    pub source_type: Initiator,
    pub peer_id: Option<String>,
    pub sent_bytes: u128,
    pub received_bytes: u128,
    pub incomplete_dropped_messages: u64,
    pub total_chunks: u64,
    pub decrypted_chunks: u64,
    pub error_report: Option<ParserErrorReport>,
    pub metadata: Option<PeerMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParserErrorReport {
    pub position: u64,
    pub error: ParserError,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParserError {
    FailedToWriteInDatabase,
    FailedToDecrypt,
    FirstPacketContainMultipleChunks,
    WrongProofOfWork,
    NoDecipher,
    Unknown,
}
