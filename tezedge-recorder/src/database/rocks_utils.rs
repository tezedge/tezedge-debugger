// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{collections::HashMap, fmt, net::SocketAddr, time::Duration};
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
    incoming_cache: HashMap<connection::Key, Cache>,
    outgoing_cache: HashMap<connection::Key, Cache>,
}

#[derive(Default, Clone)]
struct Cache {
    chunk: u64,
    offset: u64,
}

impl<'a> SyscallMetadataIterator<'a> {
    pub(super) fn new(db: &'a Db, iter: IteratorWithSchema<'a, syscall::Schema>) -> Self {
        SyscallMetadataIterator {
            db,
            iter,
            incoming_cache: Default::default(),
            outgoing_cache: Default::default(),
        }
    }

    fn cache(&self, cn_id: &connection::Key, incoming: bool) -> Cache {
        if incoming {
            self.incoming_cache.get(cn_id).cloned().unwrap_or_default()
        } else {
            self.outgoing_cache.get(cn_id).cloned().unwrap_or_default()
        }
    }

    fn update_cache(&mut self, cn_id: &connection::Key, incoming: bool, cache: Cache) {
        if incoming {
            let _ = self.incoming_cache.insert(cn_id.clone(), cache);
        } else {
            let _ = self.outgoing_cache.insert(cn_id.clone(), cache);
        }
    }

    fn fetch_data(
        &mut self,
        cn_id: connection::Key,
        offset: u64,
        length: u32,
        incoming: bool,
    ) -> Result<Vec<u8>, DbError> {
        let mut v = Vec::with_capacity(length as usize);

        let mut cache = self.cache(&cn_id, incoming);
        let start = chunk::Key {
            cn_id: cn_id.clone(),
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
        while let Some((c, bytes)) = chunks.next() {
            let len = bytes.len() as u64;
            if cache.offset + len > offset {
                let q = if cache.offset < offset {
                    offset - cache.offset
                } else {
                    0
                };
                let q = q as usize;
                let to_copy = (length as usize - v.len()).min(bytes.len() - q);
                v.extend_from_slice(&bytes[q..(q + to_copy)]);
            }
            if cache.offset + len >= offset + (length as u64) {
                /*self.cache(incoming).set(Cache {
                    chunk,
                    offset: skip_offset as u64,
                });*/
                cache.chunk = c;
                break;
            }

            cache.offset += len;
        }

        self.update_cache(&cn_id, incoming, cache);

        Ok(v)
    }

    fn convert(&mut self, info: syscall::Item) -> Result<SyscallMetadata, DbError> {
        let cn_id = info.cn_id.clone();
        let cn = self.db.as_kv::<connection::Schema>()
            .get(&cn_id)?.unwrap();

        let inner = match info.inner {
            syscall::ItemInner::Close => SyscallKind::Close,
            syscall::ItemInner::Connect(r) => SyscallKind::Connect(r),
            syscall::ItemInner::Accept(r) => SyscallKind::Accept(r),
            syscall::ItemInner::Write(r) => {
                let r = match r {
                    Ok(dr) => Ok(self.fetch_data(cn_id, dr.offset, dr.length, false)?),
                    Err(code) => Err(code),
                };
                SyscallKind::Write(r)
            },
            syscall::ItemInner::Read(r) => {
                let r = match r {
                    Ok(dr) => Ok(self.fetch_data(cn_id, dr.offset, dr.length, true)?),
                    Err(code) => Err(code),
                };
                SyscallKind::Read(r)
            },
        };

        Ok(SyscallMetadata {
            timestamp: info.timestamp,
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
