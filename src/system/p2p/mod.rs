mod parser;
pub use self::parser::{Parser, Command, Message};

pub mod report;

mod compare;
pub use self::compare::{PeerMetadata, PeerMetadataDiff, Peer};

use serde::{Serialize, Deserialize};
use crate::messages::p2p_message::SourceType;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionReport {
    pub remote_address: String,
    pub source_type: SourceType,
    pub report: ParserStatistics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParserStatistics {
    pub peer_id: Option<String>,
    pub sent_bytes: u128,
    pub received_bytes: u128,
    pub incomplete_dropped_messages: u64,
    pub total_chunks: u64,
    pub decrypted_chunks: u64,
    pub peer_metadata: PeerMetadata,
    pub error_report: Option<ParserErrorReport>,
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
