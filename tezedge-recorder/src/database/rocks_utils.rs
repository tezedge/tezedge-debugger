// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{time::Duration, net::SocketAddr};
use storage::{
    Direction,
    IteratorMode,
    persistent::{
        DBError,
        KeyValueStoreBackend,
        database::{KeyValueStoreWithSchemaIterator, IteratorWithSchema},
    },
};
use super::{common::Sender, syscall, connection, chunk, rocks::{Db, DbError}};

pub struct SyscallMetadata {
    // unix time
    pub timestamp: Duration,
    pub socket_address: SocketAddr,
    pub inner: SyscallKind,
}

pub enum SyscallKind {
    Close,
    Connect(Result<(), i32>),
    Accept(Result<(), i32>),
    Write(Result<Vec<u8>, i32>),
    Read(Result<Vec<u8>, i32>),
}

pub struct SyscallMetadataIterator<'a> {
    db: &'a Db,
    iter: IteratorWithSchema<'a, syscall::Schema>,
}

impl<'a> SyscallMetadataIterator<'a> {
    pub(super) fn new(db: &'a Db, iter: IteratorWithSchema<'a, syscall::Schema>) -> Self {
        SyscallMetadataIterator { db, iter }
    }

    fn fetch_data(
        &self,
        cn_id: connection::Key,
        offset: syscall::DataOffset,
        length: u32,
        incoming: bool,
    ) -> Result<Vec<u8>, DbError> {
        let mut v = Vec::with_capacity(length as usize);

        let start = chunk::Key {
            cn_id,
            counter: offset.chunk_number,
            sender: Sender::new(incoming),
        };
        let mut chunks = self.db
            .as_kv::<chunk::Schema>()
            .iterator(IteratorMode::From(&start, Direction::Forward))?;

        let mut offset = offset.offset_in_chunk as usize;
        let mut length = length as usize;
        while let Some((_, chunk)) = chunks.next() {
            let chunk = chunk.map_err(|error| DBError::SchemaError { error })?;

            assert!(offset < chunk.bytes.len());
            let to_copy = chunk.bytes.len() - offset;
            if length == 0 {
                break;
            } else if to_copy < length {
                v.extend_from_slice(&chunk.bytes[offset..]);
            } else {
                let mut c = chunk.bytes;
                v.append(&mut c);
            }

            length -= to_copy;
            // after first iteration offset should be 0 always
            offset = 0;
        }

        Ok(v)
    }

    fn convert(&self, info: syscall::Item) -> Result<SyscallMetadata, DbError> {
        let cn_id = info.cn_id();
        let cn = self.db.as_kv::<connection::Schema>()
            .get(&cn_id)?.unwrap();
        let timestamp = Duration::from_secs(cn_id.ts) + Duration::from_nanos(cn_id.ts_nanos as u64);

        let inner = match info {
            syscall::Item::Close(_) => SyscallKind::Close,
            syscall::Item::Connect(r) => SyscallKind::Connect(r.map(|_| ())),
            syscall::Item::Accept(r) => SyscallKind::Accept(r.map(|_| ())),
            syscall::Item::Write(r) => {
                let r = match r {
                    Ok(dr) => Ok(self.fetch_data(cn_id, dr.offset, dr.length, false)?),
                    Err(code) => Err(code),
                };
                SyscallKind::Write(r)
            },
            syscall::Item::Read(r) => {
                let r = match r {
                    Ok(dr) => Ok(self.fetch_data(cn_id, dr.offset, dr.length, true)?),
                    Err(code) => Err(code),
                };
                SyscallKind::Read(r)
            },
        };

        Ok(SyscallMetadata {
            timestamp,
            socket_address: cn.remote_addr,
            inner,
        })
    }
}

impl<'a> Iterator for SyscallMetadataIterator<'a> {
    type Item = Result<SyscallMetadata, DbError>;

    fn next(&mut self) -> Option<Self::Item> {
        let (_, syscall_info) = self.iter.next()?;

        let r = syscall_info
            .map_err(|error| DbError::Rocksdb(DBError::SchemaError { error }))
            .and_then(|info| self.convert(info));

        Some(r)
    }
}
