use std::{convert::TryFrom, str::FromStr};
use storage::persistent::{KeyValueSchema, Decoder, SchemaError, Encoder};
use serde::{Serialize, Deserialize};
use failure::Fail;
use super::FilterField;

#[repr(u8)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogLevel {
    Trace = 0x1 << 0,
    Debug = 0x1 << 1,
    Info = 0x1 << 2,
    Notice = 0x1 << 3,
    Warning = 0x1 << 4,
    Error = 0x1 << 5,
    Fatal = 0x1 << 6,
}

#[derive(Debug, Fail)]
pub enum ParseLogLevelError {
    #[fail(display = "Invalid log level name {}", _0)]
    InvalidName(String),
    #[fail(display = "Invalid log level value {}", _0)]
    InvalidValue(u8),
}

impl TryFrom<u8> for LogLevel {
    type Error = ParseLogLevelError;

    fn try_from(value: u8) -> Result<Self, ParseLogLevelError> {
        match value {
            x if x == Self::Trace as u8 => { Ok(Self::Trace) }
            x if x == Self::Debug as u8 => { Ok(Self::Debug) }
            x if x == Self::Info as u8 => { Ok(Self::Info) }
            x if x == Self::Notice as u8 => { Ok(Self::Notice) }
            x if x == Self::Warning as u8 => { Ok(Self::Warning) }
            x if x == Self::Error as u8 => { Ok(Self::Error) }
            x if x == Self::Fatal as u8 => { Ok(Self::Fatal) }
            x => Err(ParseLogLevelError::InvalidValue(x)),
        }
    }
}

impl FromStr for LogLevel {
    type Err = ParseLogLevelError;

    fn from_str(level: &str) -> Result<Self, Self::Err> {
        let level = level.to_lowercase();
        Ok(match level.as_ref() {
            "trace" => Self::Trace,
            "debug" => Self::Debug,
            "info" => Self::Info,
            "notice" => Self::Notice,
            "warn" | "warning" => Self::Warning,
            "error" => Self::Error,
            "fatal" => Self::Fatal,
            _ => return Err(ParseLogLevelError::InvalidName(level)),
        })
    }
}

impl<Schema> FilterField<Schema> for LogLevel
where
    Schema: KeyValueSchema<Key = u64>,
{
    type Key = LogLevelKey;

    fn make_index(&self, primary_key: &<Schema as KeyValueSchema>::Key) -> Self::Key {
        LogLevelKey {
            level: self.clone() as u8,
            index: primary_key.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LogLevelKey {
    level: u8,
    index: u64,
}

/// * bytes layout: `[level(1)][padding(7)][index(8)]`
impl Decoder for LogLevelKey {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        if bytes.len() != 16 {
            return Err(SchemaError::DecodeError);
        }

        Ok(LogLevelKey {
            level: bytes[0],
            index: {
                let mut b = [0; 8];
                b.clone_from_slice(&bytes[8..16]);
                u64::from_be_bytes(b)
            },
        })
    }
}

/// * bytes layout: `[level(1)][padding(7)][index(8)]`
impl Encoder for LogLevelKey {
    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        let mut buf = Vec::with_capacity(16);

        buf.extend_from_slice(&[self.level.clone()]);
        buf.extend_from_slice(&[0u8; 7]);
        buf.extend_from_slice(&self.index.to_be_bytes());

        if buf.len() != 16 {
            println!("{:?} - {:?}", self, buf);
            Err(SchemaError::EncodeError)
        } else {
            Ok(buf)
        }
    }
}
