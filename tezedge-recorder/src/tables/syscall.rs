// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::convert::TryFrom;
use storage::persistent::{
    KeyValueSchema, Encoder, Decoder, SchemaError, database::RocksDbKeyValueSchema,
};
use super::connection;

/// Represent a result of syscall execution
/// Close syscall cannot fail
/// Connect or Accept syscall yield a socket address of a new connection or error code
/// Read or Write syscall give a data or error code
#[derive(Debug, Clone)]
pub enum Item {
    Close(connection::Key),
    Connect(Result<connection::Key, i32>),
    Accept(Result<connection::Key, i32>),
    Write(Result<DataRef, i32>),
    Read(Result<DataRef, i32>),
}

#[derive(Debug, Clone)]
pub struct DataRef {
    pub cn: connection::Key,
    pub offset: DataOffset,
    // encoded as 3 bytes
    pub length: u32,
}

#[derive(Debug, Clone)]
pub struct DataOffset {
    // encoded as 6 bytes
    pub chunk_number: u64,
    pub offset_in_chunk: u16,
}

impl Encoder for DataOffset {
    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        let i = self.chunk_number << 16 + (self.offset_in_chunk as u64);
        Ok(i.to_be_bytes().as_ref().to_vec())
    }
}

impl Decoder for DataOffset {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        if bytes.len() != 8 {
            return Err(SchemaError::DecodeError);
        }

        let i = u64::from_be_bytes(TryFrom::try_from(bytes).unwrap());
        Ok(DataOffset {
            chunk_number: i >> 16,
            offset_in_chunk: (i & 0xffff) as u16,
        })
    }
}

impl Item {
    pub fn cn_id(&self) -> connection::Key {
        // TODO: store cn id even if syscall failed
        match self {
            | &Item::Close(ref cn)
            | &Item::Connect(Ok(ref cn))
            | &Item::Accept(Ok(ref cn))=> cn.clone(),
            | &Item::Connect(Err(_))
            | &Item::Accept(Err(_)) => unimplemented!(),
            | &Item::Write(Ok(DataRef { ref cn, .. }))
            | &Item::Read(Ok(DataRef { ref cn, .. })) => cn.clone(),
            | &Item::Write(Err(_))
            | &Item::Read(Err(_)) => unimplemented!(),
        }
    }

    fn incoming(&self) -> bool {
        match self {
            &Item::Close(_) => false,
            &Item::Connect(_) | &Item::Write(_) => false,
            &Item::Accept(_) | &Item::Read(_) => true,
        }
    }

    fn data(&self) -> bool {
        match self {
            &Item::Close(_) => false,
            &Item::Connect(_) | &Item::Accept(_) => false,
            &Item::Write(_) | &Item::Read(_) => true,
        }
    }

    fn err(&self) -> bool {
        match self {
            &Item::Close(_) => false,
            &Item::Connect(ref r) | &Item::Accept(ref r) => r.is_err(),
            &Item::Write(ref r) | &Item::Read(ref r) => r.is_err(),
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
    fn discriminant(&self) -> u8 {
        match self {
            &Item::Close(_) => 0b0111,
            _ => 8 + u8::from(self.err()) * 4 + u8::from(self.data()) * 2 + u8::from(self.incoming())
        }
    }
}

impl Encoder for Item {
    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        let mut v = Vec::with_capacity(24);
        match self {
            | &Item::Close(ref cn)
            | &Item::Connect(Ok(ref cn))
            | &Item::Accept(Ok(ref cn)) => {
                v.push(self.discriminant());
                v.extend_from_slice(&[0, 0, 0]);
                v.append(&mut cn.encode()?);
                v.extend_from_slice(&0u64.to_ne_bytes());
            },
            | &Item::Write(Ok(ref data))
            | &Item::Read(Ok(ref data)) => {
                let &DataRef { ref cn, ref offset, length } = data;
                v.push(self.discriminant());
                v.extend_from_slice(&length.to_be_bytes()[1..4]);
                v.append(&mut cn.encode()?);
                v.append(&mut offset.encode()?);
            },
            | &Item::Connect(Err(code))
            | &Item::Accept(Err(code))
            | &Item::Write(Err(code))
            | &Item::Read(Err(code)) => {
                v.push(self.discriminant());
                v.extend_from_slice(&(-code).to_be_bytes()[1..4]);
                v.extend_from_slice(&0u64.to_ne_bytes());
                v.extend_from_slice(&0u32.to_ne_bytes());
                v.extend_from_slice(&0u64.to_ne_bytes());
            },
        }

        Ok(v)
    }
}

impl Decoder for Item {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        if bytes.len() != 24 {
            return Err(SchemaError::DecodeError);
        }

        let discriminant = bytes[0];
        let code = u32::from_be_bytes([0, bytes[1], bytes[2], bytes[3]]);
        let data_ref = DataRef {
            cn: connection::Key::decode(&bytes[4..16])?,
            offset: DataOffset::decode(&bytes[16..24])?,
            length: code,
        };
        match discriminant {
            0b0111 => Ok(Item::Close(data_ref.cn)),
            0b1000 => Ok(Item::Connect(Ok(data_ref.cn))),
            0b1001 => Ok(Item::Accept(Ok(data_ref.cn))),
            0b1010 => Ok(Item::Write(Ok(data_ref))),
            0b1011 => Ok(Item::Read(Ok(data_ref))),
            0b1100 => Ok(Item::Connect(Err(-(code as i32)))),
            0b1101 => Ok(Item::Accept(Err(-(code as i32)))),
            0b1110 => Ok(Item::Write(Err(-(code as i32)))),
            0b1111 => Ok(Item::Read(Err(-(code as i32)))),
            _ => Err(SchemaError::DecodeError),
        }
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
