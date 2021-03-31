// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    path::Path,
    sync::atomic::{Ordering, AtomicU64},
};
use rocksdb::{Cache, DB, ReadOptions};
use storage::{
    Direction, IteratorMode,
    persistent::{
        self, DBError, DbConfiguration, Decoder, Encoder, KeyValueSchema, KeyValueStoreWithSchema,
        SchemaError,
    },
};
use thiserror::Error;
use super::{
    Database, DatabaseNew, DatabaseFetch,
    ConnectionsFilter, ChunksFilter, MessagesFilter, LogsFilter,
    connection, chunk, message, node_log,
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
    log_counter: AtomicU64,
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

    fn reserve_log_counter(&self) -> u64 {
        self.log_counter.fetch_add(1, Ordering::SeqCst)
    }
}

impl DatabaseNew for Db {
    type Error = DbError;

    fn open<P>(path: P) -> Result<Self, Self::Error>
    where
        P: AsRef<Path>,
    {
        let cache = Cache::new_lru_cache(1).map_err(|error| DBError::RocksDBError { error })?;

        let cfs = vec![
            connection::Schema::descriptor(&cache),
            chunk::Schema::descriptor(&cache),
            message::Schema::descriptor(&cache),
            node_log::Schema::descriptor(&cache),
        ];
        let inner = persistent::open_kv(path, cfs, &DbConfiguration::default())?;

        Ok(Db {
            //_cache: cache,
            message_counter: AtomicU64::new(0),
            log_counter: AtomicU64::new(0),
            inner,
        })
    }
}

impl Database for Db {
    fn store_connection(&self, item: connection::Item) {
        let (key, value) = item.split();
        if let Err(error) = self.as_kv::<connection::Schema>().put(&key, &value) {
            log::error!("database error: {}", error);
        }
    }

    fn update_connection(&self, item: connection::Item) {
        let (key, value) = item.split();
        let kv = self.as_kv::<connection::Schema>();
        if let Err(error) = kv.delete(&key).and_then(|()| kv.put(&key, &value)) {
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

    fn store_log(&self, item: node_log::Item) {
        let index = self.reserve_log_counter();
        if let Err(error) = self.as_kv::<node_log::Schema>().put(&index, &item) {
            log::error!("database error: {}", error);
        }
    }
}

// TODO: duplicated code
impl DatabaseFetch for Db {
    fn fetch_connections(
        &self,
        filter: &ConnectionsFilter,
    ) -> Result<Vec<(connection::Key, connection::Value)>, Self::Error> {
        let limit = filter.limit.unwrap_or(100) as usize;
        let mode = IteratorMode::Start;
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

    fn fetch_chunks_truncated(
        &self,
        filter: &ChunksFilter,
    ) -> Result<Vec<(chunk::Key, chunk::ValueTruncated)>, Self::Error> {
        type ItItem = (
            Result<chunk::Key, SchemaError>,
            Result<chunk::Value, SchemaError>,
        );

        fn collect_it(
            it: impl Iterator<Item = ItItem>,
            limit: usize,
        ) -> Vec<(chunk::Key, chunk::ValueTruncated)> {
            it.filter_map(|(k, v)| match (k, v) {
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
            .collect()
        }

        let limit = filter.limit.unwrap_or(100) as usize;
        if let Some(connection_id) = &filter.cn {
            let cn_id = connection_id
                .parse()
                .map_err(|e: connection::KeyFromStrError| DBError::SchemaError {
                    error: SchemaError::DecodeValidationError(e.to_string()),
                })?;
            let k = chunk::Key::end(cn_id);
            let k_bytes = k.encode().map_err(|error| DBError::SchemaError { error })?;
            let mode = rocksdb::IteratorMode::From(&k_bytes, rocksdb::Direction::Forward);
            let mut opts = ReadOptions::default();
            opts.set_prefix_same_as_start(true);
            let cf = self.inner.cf_handle(chunk::Schema::name()).ok_or(
                DBError::MissingColumnFamily {
                    name: chunk::Schema::name(),
                },
            )?;
            let it = self
                .inner
                .iterator_cf_opt(cf, opts, mode)
                .map(|(k, v)| (chunk::Key::decode(&k), chunk::Value::decode(&v)));
            Ok(collect_it(it, limit))
        } else {
            let it = self.as_kv::<chunk::Schema>().iterator(IteratorMode::Start)?;
            Ok(collect_it(it, limit))
        }
    }

    fn fetch_chunk(&self, key: &chunk::Key) -> Result<Option<chunk::Value>, Self::Error> {
        self.as_kv::<chunk::Schema>().get(&key).map_err(Into::into)
    }

    fn fetch_messages(
        &self,
        filter: &MessagesFilter,
    ) -> Result<Vec<message::MessageFrontend>, Self::Error> {
        let limit = filter.limit.unwrap_or(100) as usize;
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

    fn fetch_log(
        &self,
        filter: &LogsFilter,
    ) -> Result<Vec<node_log::Item>, Self::Error> {
        let limit = filter.limit.unwrap_or(100) as usize;
        let mode = if let Some(cursor) = &filter.cursor {
            IteratorMode::From(cursor, Direction::Reverse)
        } else {
            IteratorMode::End
        };
        let vec = self
            .as_kv::<node_log::Schema>()
            .iterator(mode)?
            .filter_map(|(k, v)| match (k, v) {
                (Ok(_), Ok(value)) => Some(value),
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
