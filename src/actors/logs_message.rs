use serde::{Serialize, Deserialize, Deserializer};
use std::collections::HashMap;
use storage::persistent::{Encoder, SchemaError, Decoder};
use crate::storage::get_ts;
use serde_json::Value;

fn deserialize_date<'de, D>(deserializer: D) -> Result<u128, D::Error>
    where
        D: Deserializer<'de>
{
    if deserializer.is_human_readable() {
        let _ = String::deserialize(deserializer)?;
        Ok(get_ts())
    } else {
        u128::deserialize(deserializer)
    }
}

fn deserialize_level<'de, D>(deserializer: D) -> Result<String, D::Error>
    where
        D: Deserializer<'de>
{
    let value = InnerLevel::deserialize(deserializer)?;
    Ok(value.consume())
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(untagged)]
enum InnerLevel {
    Numeral(i32),
    String(String),
}

impl InnerLevel {
    pub fn consume(self) -> String {
        match self {
            InnerLevel::Numeral(value) => match value {
                10 => "trace".to_string(),
                20 => "debug".to_string(),
                30 => "info".to_string(),
                40 => "warn".to_string(),
                50 => "error".to_string(),
                _ => "info".to_string(),
            },
            InnerLevel::String(value) => value,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogMessage {
    #[serde(deserialize_with = "deserialize_level")]
    pub level: String,
    #[serde(alias = "timestamp", alias = "time", rename(serialize = "timestamp"), deserialize_with = "deserialize_date")]
    pub date: u128,
    #[serde(alias = "module")]
    pub section: String,
    #[serde(alias = "msg", rename(serialize = "message"))]
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,

    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

impl LogMessage {
    pub fn raw(line: String) -> Self {
        Self {
            level: "fatal".to_string(),
            date: get_ts(),
            section: "".to_string(),
            id: None,
            extra: Default::default(),
            message: line,
        }
    }
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
