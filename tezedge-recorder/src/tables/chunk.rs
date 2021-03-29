// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{fmt, convert::TryFrom};
use serde::{Serialize, ser::{self, SerializeStruct}};
use rocksdb::{Cache, ColumnFamilyDescriptor};
use storage::persistent::{KeyValueSchema, Encoder, Decoder, SchemaError};
use super::{common::Sender, connection};

#[derive(Clone)]
pub struct Item {
    cn_id: connection::Key,
    pub sender: Sender,
    pub counter: u64,
    timestamp: u64,
    pub bytes: Vec<u8>,
    pub plain: Vec<u8>,
}

impl Item {
    pub fn new(
        cn_id: connection::Key,
        sender: Sender,
        counter: u64,
        bytes: Vec<u8>,
        plain: Vec<u8>,
    ) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Item {
            cn_id,
            sender,
            counter,
            timestamp,
            bytes,
            plain,
        }
    }

    #[rustfmt::skip]
    pub fn split(self) -> (Key, Value) {
        let Item { cn_id, counter, sender, timestamp, bytes, plain } = self;
        (Key { cn_id, counter, sender }, Value { timestamp, bytes, plain })
    }
}

impl fmt::Debug for Item {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Item")
            .field("connection_id", &self.cn_id)
            .field("sender", &self.sender)
            .field("counter", &self.counter)
            .field("timestamp", &self.timestamp)
            .field("bytes", &hex::encode(&self.bytes))
            .field("plain", &hex::encode(&self.plain))
            .finish()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Key {
    pub cn_id: connection::Key,
    counter: u64,
    sender: Sender,
}

impl Key {
    pub fn from_cn_id(cn_id: connection::Key) -> Self {
        Key {
            cn_id,
            counter: u64::MAX / 2,
            sender: Sender::Remote,
        }
    }
}

impl Encoder for Key {
    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        let mut v = Vec::with_capacity(20);
        v.extend_from_slice(&self.cn_id.encode()?);
        let c = if self.sender.incoming() { self.counter * 2 + 1 } else { self.counter * 2 };
        v.extend_from_slice(&c.to_be_bytes());
        Ok(v)
    }
}

impl Decoder for Key {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        if bytes.len() != 20 {
            return Err(SchemaError::DecodeError);
        }

        let c;
        Ok(Key {
            cn_id: connection::Key::decode(&bytes[..12])?,
            counter: {
                c = u64::from_be_bytes(TryFrom::try_from(&bytes[12..]).unwrap());
                c / 2
            },
            sender: Sender::new(c & 1 != 0),
        })
    }
}

pub struct Value {
    timestamp: u64,
    bytes: Vec<u8>,
    plain: Vec<u8>,
}

pub struct ValueTruncated(pub Value);

impl Serialize for Value {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        let mut s = serializer.serialize_struct("Chunk", 3)?;
        s.serialize_field("timestamp", &self.timestamp)?;
        s.serialize_field("bytes", &hex::encode(&self.bytes))?;
        s.serialize_field("plain", &hex::encode(&self.plain))?;
        s.end()
    }
}

impl Serialize for ValueTruncated {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        let truncated_hex = |d: &[u8]| -> String {
            if d.len() > 0x10000 {
                format!(
                    "{}...truncated {} bytes",
                    hex::encode(&d[..0x10000]),
                    d.len() - 0x10000,
                )
            } else {
                hex::encode(d)
            }
        };

        let mut s = serializer.serialize_struct("Chunk", 3)?;
        s.serialize_field("timestamp", &self.0.timestamp)?;
        s.serialize_field("bytes", &hex::encode(&truncated_hex(&self.0.bytes)))?;
        s.serialize_field("plain", &hex::encode(&truncated_hex(&self.0.plain)))?;
        s.end()
    }
}

impl Encoder for Value {
    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        let mut v = Vec::with_capacity(self.bytes.len() + self.plain.len() + 16);
        v.extend_from_slice(&self.timestamp.to_le_bytes());
        v.extend_from_slice(&(self.bytes.len() as u64).to_le_bytes());
        v.extend_from_slice(&self.bytes);
        v.extend_from_slice(&self.plain);
        Ok(v)
    }
}

impl Decoder for Value {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        if bytes.len() < 16 {
            return Err(SchemaError::DecodeError);
        }

        let len;
        Ok(Value {
            timestamp: u64::from_le_bytes(TryFrom::try_from(&bytes[..8]).unwrap()),
            bytes: {
                len = u64::from_le_bytes(TryFrom::try_from(&bytes[8..16]).unwrap()) as usize;
                if bytes.len() < 16 + len {
                    return Err(SchemaError::DecodeError);
                }
                bytes[16..(16 + len)].to_vec()
            },
            plain: bytes[(16 + len)..].to_vec(),
        })
    }
}

pub struct Schema;

impl KeyValueSchema for Schema {
    type Key = Key;
    type Value = Value;

    fn descriptor(_cache: &Cache) -> ColumnFamilyDescriptor {
        use rocksdb::{Options, SliceTransform};

        let mut cf_opts = Options::default();
        cf_opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(12));
        cf_opts.set_memtable_prefix_bloom_ratio(0.2);
        ColumnFamilyDescriptor::new(Self::name(), cf_opts)
    }

    fn name() -> &'static str {
        "chunk_storage"
    }
}
