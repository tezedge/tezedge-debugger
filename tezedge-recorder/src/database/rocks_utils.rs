// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{time::Duration, net::SocketAddr, fmt, cell::Cell};
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

#[derive(Debug)]
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

impl fmt::Debug for SyscallKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            &SyscallKind::Close => f.debug_struct("Close").finish(),
            &SyscallKind::Connect(Ok(())) => f.debug_tuple("Connect").finish(),
            &SyscallKind::Accept(Ok(())) => f.debug_tuple("Accept").finish(),
            &SyscallKind::Write(Ok(ref b)) => f.debug_tuple("Write").field(&hex::encode(b)).finish(),
            &SyscallKind::Read(Ok(ref b)) => f.debug_tuple("Read").field(&hex::encode(b)).finish(),
            &SyscallKind::Connect(Err(c)) => f.debug_tuple("ConnectError").field(&c).finish(),
            &SyscallKind::Accept(Err(c)) => f.debug_tuple("AcceptError").field(&c).finish(),
            &SyscallKind::Write(Err(c)) => f.debug_tuple("WriteError").field(&c).finish(),
            &SyscallKind::Read(Err(c)) => f.debug_tuple("ReadError").field(&c).finish(),
        }
    }
}

pub struct SyscallMetadataIterator<'a> {
    db: &'a Db,
    iter: IteratorWithSchema<'a, syscall::Schema>,
    incoming_cache: Cell<Cache>,
    outgoing_cache: Cell<Cache>,
}

#[derive(Default, Clone, Copy)]
struct Cache {
    chunk: u64,
    offset: u64,
}

impl<'a> SyscallMetadataIterator<'a> {
    pub(super) fn new(db: &'a Db, iter: IteratorWithSchema<'a, syscall::Schema>) -> Self {
        SyscallMetadataIterator {
            db,
            iter,
            incoming_cache: Cell::default(),
            outgoing_cache: Cell::default(),
        }
    }

    fn cache(&self, incoming: bool) -> &Cell<Cache> {
        if incoming {
            &self.incoming_cache
        } else {
            &self.outgoing_cache
        }
    }

    fn fetch_data(
        &self,
        cn_id: connection::Key,
        offset: u64,
        length: u32,
        incoming: bool,
    ) -> Result<Vec<u8>, DbError> {
        let mut v = Vec::with_capacity(length as usize);

        let cache = self.cache(incoming).get();
        let start = chunk::Key {
            cn_id,
            counter: cache.chunk,
            sender: Sender::new(incoming),
        };
        let mut chunks = self.db
            .as_kv::<chunk::Schema>()
            .iterator(IteratorMode::From(&start, Direction::Forward))?
            .filter_map(|(k, v)| {
                let k = k.ok()?;
                let bytes = v.ok()?.bytes;
                if k.sender.incoming() == incoming {
                    Some((k.counter, bytes))
                } else {
                    None
                }
            });
        let mut skip_offset = cache.offset as usize;
        while let Some((chunk, bytes)) = chunks.next() {
            if skip_offset + bytes.len() > offset as usize {
                let q = if skip_offset < offset as usize {
                    (offset as usize) - skip_offset
                } else {
                    0
                };
                let to_copy = (length as usize - v.len()).min(bytes.len() - q);
                v.extend_from_slice(&bytes[q..(q + to_copy)]);
            }
            if skip_offset + bytes.len() >= (offset as usize) + (length as usize) {
                self.cache(incoming).set(Cache {
                    chunk,
                    offset: skip_offset as u64,
                });
                break;
            }

            skip_offset += bytes.len();
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
