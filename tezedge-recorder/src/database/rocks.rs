// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{Ordering, AtomicU64},
    },
};
use rocksdb::{Cache, DB};
use storage::{
    persistent::{self, DBError, DbConfiguration, KeyValueSchema, KeyValueStoreWithSchema},
    IteratorMode, Direction,
};
use thiserror::Error;
use super::{
    Database, DatabaseNew, DatabaseFetch, ConnectionsFilter, ChunksFilter, MessagesFilter, connection, chunk,
    message,
};

#[derive(Error, Debug)]
#[error("{}", _0)]
pub struct DbError(DBError);

impl From<DBError> for DbError {
    fn from(v: DBError) -> Self {
        DbError(v)
    }
}

pub struct Db {
    //_cache: Cache,
    message_counter: AtomicU64,
    inner: DB,
}

impl Db {
    fn as_kv<S>(&self) -> &impl KeyValueStoreWithSchema<S>
    where
        S: KeyValueSchema,
    {
        &self.inner
    }

    fn reserve_message_counter(&self) -> u64 {
        self.message_counter.fetch_add(1, Ordering::SeqCst)
    }
}

impl DatabaseNew for Db {
    type Error = DbError;

    fn open<P>(path: P) -> Result<Arc<Self>, Self::Error>
    where
        P: AsRef<Path>,
    {
        let cache = Cache::new_lru_cache(1).map_err(|error| DBError::RocksDBError { error })?;

        let cfs = vec![
            connection::Schema::descriptor(&cache),
            chunk::Schema::descriptor(&cache),
            message::Schema::descriptor(&cache),
        ];
        let inner = persistent::open_kv(path, cfs, &DbConfiguration::default())?;

        Ok(Arc::new(Db {
            //_cache: cache,
            message_counter: AtomicU64::new(0),
            inner,
        }))
    }
}

impl Database for Db {
    fn store_connection(&self, item: connection::Item) {
        let (key, value) = item.split();
        if let Err(error) = self.as_kv::<connection::Schema>().put(&key, &value) {
            // TODO: should panic/stop here?
            log::error!("database error: {}", error);
        }
    }

    fn store_chunk(&self, item: chunk::Item) {
        let (key, value) = item.split();
        if let Err(error) = self.as_kv::<chunk::Schema>().put(&key, &value) {
            log::error!("database error: {}", error);
        }
    }

    fn store_message(&self, item: message::Item) {
        let index = self.reserve_message_counter();
        if let Err(error) = self.as_kv::<message::Schema>().put(&index, &item) {
            log::error!("database error: {}", error);
        }
    }
}

// TODO: duplicated code
impl DatabaseFetch for Db {
    fn fetch_connections(
        &self,
        filter: &ConnectionsFilter,
        limit: usize,
    ) -> Result<Vec<(connection::Key, connection::Value)>, Self::Error> {
        let mode = if let Some(cursor) = &filter.cursor {
            IteratorMode::From(cursor, Direction::Reverse)
        } else {
            IteratorMode::End
        };
        let vec = self
            .as_kv::<connection::Schema>()
            .iterator(mode)?
            .filter_map(|(k, v)| match (k, v) {
                (Ok(key), Ok(value)) => Some((key, value)),
                (Ok(index), Err(err)) => {
                    log::warn!("Failed to load value at {:?}: {}", index, err);
                    None
                },
                (Err(err), _) => {
                    log::warn!("Failed to load index: {}", err);
                    None
                },
            })
            .take(limit)
            .collect();
        Ok(vec)
    }

    fn fetch_chunks(
        &self,
        filter: &ChunksFilter,
    ) -> Result<Vec<(chunk::Key, chunk::Value)>, Self::Error> {
        let limit = filter.limit.unwrap_or(100) as usize;
        let mut k = chunk::Key::from_cn_id(connection::Key { ts: 0, ts_nanos: 0 });
        let mode = if let (Some(ts), Some(ts_nanos)) = (filter.secs, filter.nanos) {
            k.cn_id = connection::Key { ts, ts_nanos };
            IteratorMode::From(&k, Direction::Reverse)
        } else {
            IteratorMode::End
        };
        let vec = self
            .as_kv::<chunk::Schema>()
            .iterator(mode)?
            .filter_map(|(k, v)| match (k, v) {
                (Ok(key), Ok(value)) => Some((key, value)),
                (Ok(index), Err(err)) => {
                    log::warn!("Failed to load value at {:?}: {}", index, err);
                    None
                },
                (Err(err), _) => {
                    log::warn!("Failed to load index: {}", err);
                    None
                },
            })
            .take(limit)
            .collect();
        Ok(vec)
    }

    fn fetch_chunks_truncated(
        &self,
        filter: &ChunksFilter,
    ) -> Result<Vec<(chunk::Key, chunk::ValueTruncated)>, Self::Error> {
        let limit = filter.limit.unwrap_or(100) as usize;
        let mut k = chunk::Key::from_cn_id(connection::Key { ts: 0, ts_nanos: 0 });
        let mode = if let (Some(ts), Some(ts_nanos)) = (filter.secs, filter.nanos) {
            k.cn_id = connection::Key { ts, ts_nanos };
            IteratorMode::From(&k, Direction::Reverse)
        } else {
            IteratorMode::End
        };
        let vec = self
            .as_kv::<chunk::Schema>()
            .iterator(mode)?
            .filter_map(|(k, v)| match (k, v) {
                (Ok(key), Ok(value)) => Some((key, chunk::ValueTruncated(value))),
                (Ok(index), Err(err)) => {
                    log::warn!("Failed to load value at {:?}: {}", index, err);
                    None
                },
                (Err(err), _) => {
                    log::warn!("Failed to load index: {}", err);
                    None
                },
            })
            .take(limit)
            .collect();
        Ok(vec)
    }

    fn fetch_messages(
        &self,
        filter: &MessagesFilter,
        limit: usize,
    ) -> Result<Vec<message::MessageFrontend>, Self::Error> {
        let mode = if let Some(cursor) = &filter.cursor {
            IteratorMode::From(cursor, Direction::Reverse)
        } else {
            IteratorMode::End
        };
        let vec = self
            .as_kv::<message::Schema>()
            .iterator(mode)?
            .filter_map(|(k, v)| match (k, v) {
                (Ok(key), Ok(value)) => Some(message::MessageFrontend::new(value, key)),
                (Ok(index), Err(err)) => {
                    log::warn!("Failed to load value at {:?}: {}", index, err);
                    None
                },
                (Err(err), _) => {
                    log::warn!("Failed to load index: {}", err);
                    None
                },
            })
            .take(limit)
            .collect();
        Ok(vec)
    }
}
