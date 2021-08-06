// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{time::Duration, convert::TryFrom};
use storage::persistent::{
    KeyValueSchema, Encoder, Decoder, SchemaError, database::RocksDbKeyValueSchema,
};
use super::connection;

/// Represent a result of syscall execution
/// Close syscall cannot fail
/// Connect or Accept syscall yield a socket address of a new connection or error code
/// Read or Write syscall give a data or error code
#[derive(Debug, Clone)]
pub struct Item {
    pub cn_id: connection::Key,
    pub timestamp: Duration,
    pub inner: ItemInner,
}

#[derive(Debug, Clone)]
pub enum ItemInner {
    Close,
    Connect(Result<(), i32>),
    Accept(Result<(), i32>),
    Write(Result<DataRef, i32>),
    Read(Result<DataRef, i32>),
}

#[derive(Debug, Clone)]
pub struct DataRef {
    pub offset: u64,
    pub length: u32,
}

impl Item {
    fn incoming(&self) -> bool {
        match &self.inner {
            &ItemInner::Close => false,
            &ItemInner::Connect(_) | &ItemInner::Write(_) => false,
            &ItemInner::Accept(_) | &ItemInner::Read(_) => true,
        }
    }

    fn data(&self) -> bool {
        match &self.inner {
            &ItemInner::Close => false,
            &ItemInner::Connect(_) | &ItemInner::Accept(_) => false,
            &ItemInner::Write(_) | &ItemInner::Read(_) => true,
        }
    }

    fn err(&self) -> bool {
        match &self.inner {
            &ItemInner::Close => false,
            &ItemInner::Connect(ref r) | &ItemInner::Accept(ref r) => r.is_err(),
            &ItemInner::Write(ref r) | &ItemInner::Read(ref r) => r.is_err(),
        }
    }

    // discriminant:
    // 0b0111 -- `Close`
    // 0b1000 -- `Connect` ok
    // 0b1001 -- `Accept` ok
    // 0b1010 -- `Write` ok
    // 0b1011 -- `Read` ok
    // 0b1100 -- `Connect` err
    // 0b1101 -- `Accept` err
    // 0b1110 -- `Write` err
    // 0b1111 -- `Read` err
    fn discriminant(&self) -> u32 {
        match &self.inner {
            &ItemInner::Close => 0b0111,
            _ => 8 + u32::from(self.err()) * 4 + u32::from(self.data()) * 2 + u32::from(self.incoming())
        }
    }
}

impl Encoder for Item {
    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        let mut v = Vec::with_capacity(40);
        v.extend_from_slice(&self.discriminant().to_be_bytes());
        v.append(&mut self.cn_id.encode()?);
        v.extend_from_slice(&self.timestamp.as_secs().to_be_bytes());
        v.extend_from_slice(&self.timestamp.subsec_nanos().to_be_bytes());

        match &self.inner {
            | &ItemInner::Close
            | &ItemInner::Connect(Ok(()))
            | &ItemInner::Accept(Ok(())) => (),
            | &ItemInner::Write(Ok(ref data))
            | &ItemInner::Read(Ok(ref data)) => {
                let &DataRef { offset, length } = data;
                v.extend_from_slice(&offset.to_be_bytes());
                v.extend_from_slice(&length.to_be_bytes());
            },
            | &ItemInner::Connect(Err(code))
            | &ItemInner::Accept(Err(code))
            | &ItemInner::Write(Err(code))
            | &ItemInner::Read(Err(code)) => {
                v.extend_from_slice(&0u64.to_ne_bytes());
                v.extend_from_slice(&code.to_be_bytes());
            },
        }

        Ok(v)
    }
}

impl Decoder for Item {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        if bytes.len() != 40 {
            return Err(SchemaError::DecodeError);
        }

        let discriminant = u32::from_be_bytes(TryFrom::try_from(&bytes[0..4]).unwrap());
        let cn_id = connection::Key::decode(&bytes[4..16])?;
        let secs = u64::from_be_bytes(TryFrom::try_from(&bytes[16..24]).unwrap());
        let nanos = u32::from_be_bytes(TryFrom::try_from(&bytes[24..28]).unwrap());
        let timestamp = Duration::from_secs(secs) + Duration::from_nanos(nanos as u64);

        let data_ref = DataRef {
            offset: u64::from_be_bytes(TryFrom::try_from(&bytes[28..36]).unwrap()),
            length: u32::from_be_bytes(TryFrom::try_from(&bytes[28..36]).unwrap()),
        };
        let inner = match discriminant {
            0b0111 => Ok(ItemInner::Close),
            0b1000 => Ok(ItemInner::Connect(Ok(()))),
            0b1001 => Ok(ItemInner::Accept(Ok(()))),
            0b1010 => Ok(ItemInner::Write(Ok(data_ref))),
            0b1011 => Ok(ItemInner::Read(Ok(data_ref))),
            0b1100 => Ok(ItemInner::Connect(Err(data_ref.length as i32))),
            0b1101 => Ok(ItemInner::Accept(Err(data_ref.length as i32))),
            0b1110 => Ok(ItemInner::Write(Err(data_ref.length as i32))),
            0b1111 => Ok(ItemInner::Read(Err(data_ref.length as i32))),
            _ => Err(SchemaError::DecodeError),
        }?;

        Ok(Item {
            cn_id,
            timestamp,
            inner,
        })
    }
}

pub struct Schema;

impl KeyValueSchema for Schema {
    type Key = u64;
    type Value = Item;
}

impl RocksDbKeyValueSchema for Schema {
    fn name() -> &'static str {
        "syscall_storage"
    }
}
