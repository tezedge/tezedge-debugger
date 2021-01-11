mod parser;
pub use self::parser::{Parser, Command, Message};

pub mod report;

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
    pub total_chunks: usize,
    pub decrypted_chunks: usize,
    pub error_report: Option<ParserErrorReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParserErrorReport {
    pub position: usize,
    pub error: ParserError,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParserError {
    FailedToWriteInDatabase,
    FailedToDecrypt,
    WrongProofOfWork,
}
