use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use storage::persistent::{Encoder, SchemaError, Decoder};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogMessage {
    pub level: String,
    pub date: String,
    pub section: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    #[serde(rename = "loc-file", skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(rename = "loc-line", skip_serializing_if = "Option::is_none")]
    pub line: Option<String>,
    #[serde(rename = "loc-column", skip_serializing_if = "Option::is_none")]
    pub column: Option<String>,

    #[serde(flatten)]
    pub extra: HashMap<String, String>,
}

impl Encoder for LogMessage {
    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        serde_cbor::to_vec(self)
            .map_err(|_| SchemaError::EncodeError)
    }
}

impl Decoder for LogMessage {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        serde_cbor::from_slice(bytes)
            .map_err(|_| SchemaError::DecodeError)
    }
}
