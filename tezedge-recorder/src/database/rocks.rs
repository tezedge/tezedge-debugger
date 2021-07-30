// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    net::SocketAddr,
    ops::Add,
    path::{Path, PathBuf},
    sync::atomic::{Ordering, AtomicU64},
};
use rocksdb::{Cache, DB, ReadOptions};
use storage::{
    Direction, IteratorMode,
    persistent::{
        self, DBError, DbConfiguration, Decoder, Encoder, KeyValueSchema,
        KeyValueStoreWithSchemaIterator, KeyValueStoreBackend, SchemaError,
        database::RocksDbKeyValueSchema,
    },
};
use tantivy::TantivyError;
use anyhow::Result;
use thiserror::Error;
use itertools::Itertools;
use super::sorted_intersect::sorted_intersect;
#[rustfmt::skip]
use super::{
    // core traits
    Database, DatabaseNew, DatabaseFetch, search,
    // filters
    ConnectionsFilter, ChunksFilter, MessagesFilter, LogsFilter,
    // tables
    common, connection, chunk, message, node_log,
    // secondary indexes
    message_ty, message_sender, message_initiator, message_addr, log_level, timestamp,
};

#[derive(Error, Debug)]
pub enum DbError {
    #[error("rocksdb error: {}", _0)]
    Rocksdb(DBError),
    #[error("there is no log indexer")]
    NoLogIndexer,
    #[error("log indexer: {}", _0)]
    LogIndexer(TantivyError),
}

impl From<DBError> for DbError {
    fn from(v: DBError) -> Self {
        DbError::Rocksdb(v)
    }
}

impl From<TantivyError> for DbError {
    fn from(v: TantivyError) -> Self {
        DbError::LogIndexer(v)
    }
}

pub struct Db {
    //_cache: Cache,
    message_store_limit: Option<u64>,
    message_counter: AtomicU64,
    log_store_limit: Option<u64>,
    log_counter: AtomicU64,
    log_indexer: Option<search::LogIndexer>,
    inner: DB,
}

impl Db {
    fn as_kv<S>(&self) -> &(impl KeyValueStoreBackend<S> + KeyValueStoreWithSchemaIterator<S>)
    where
        S: KeyValueSchema + RocksDbKeyValueSchema,
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

    fn open<P>(
        path: P,
        log_full_text_index: bool,
        log_store_limit: Option<u64>,
        message_store_limit: Option<u64>,
    ) -> Result<Self, Self::Error>
    where
        P: AsRef<Path>,
    {
        let cache = Cache::new_lru_cache(1).map_err(|error| DBError::RocksDBError { error })?;

        let cfs = vec![
            connection::Schema::descriptor(&cache),
            chunk::Schema::descriptor(&cache),
            message::Schema::descriptor(&cache),
            node_log::Schema::descriptor(&cache),
            message_ty::Schema::descriptor(&cache),
            message_sender::Schema::descriptor(&cache),
            message_initiator::Schema::descriptor(&cache),
            message_addr::Schema::descriptor(&cache),
            timestamp::MessageSchema::descriptor(&cache),
            log_level::Schema::descriptor(&cache),
            timestamp::LogSchema::descriptor(&cache),
        ];
        let path = PathBuf::from(path.as_ref());
        let inner =
            persistent::database::open_kv(path.join("rocksdb"), cfs, &DbConfiguration::default())?;

        fn counter<S>(db: &DB) -> Option<S::Key>
        where
            S: RocksDbKeyValueSchema,
            S::Key: Add<u64, Output = S::Key>,
        {
            KeyValueStoreWithSchemaIterator::<S>::iterator(db, IteratorMode::End)
                .ok()?
                .next()?
                .0
                .ok()
                .map(|c| c + 1)
        }

        let log_indexer = if log_full_text_index {
            Some(search::LogIndexer::try_new(path.join("tantivy"))?)
        } else {
            None
        };

        Ok(Db {
            message_store_limit,
            message_counter: AtomicU64::new(counter::<message::Schema>(&inner).unwrap_or(0)),
            log_store_limit,
            log_counter: AtomicU64::new(counter::<node_log::Schema>(&inner).unwrap_or(0)),
            log_indexer,
            inner,
        })
    }
}

impl Db {
    pub fn remove_message(&self, index: u64) -> Result<(), DbError> {
        if let Some(item) = self.as_kv::<message::Schema>().get(&index)? {
            let ty_index = message_ty::Item {
                ty: item.ty.clone(),
                index,
            };
            let sender_index = message_sender::Item {
                sender: item.sender.clone(),
                index,
            };
            let initiator_index = message_initiator::Item {
                initiator: item.initiator.clone(),
                index,
            };
            let addr_index = message_addr::Item {
                addr: item.remote_addr,
                index,
            };
            let timestamp_index = timestamp::Item {
                timestamp: item.timestamp,
                index,
            };

            for chunk_key in item.chunks() {
                self.as_kv::<chunk::Schema>().delete(&chunk_key)?;
            }

            self.as_kv::<message_ty::Schema>().delete(&ty_index)?;
            self.as_kv::<message_sender::Schema>()
                .delete(&sender_index)?;
            self.as_kv::<message_initiator::Schema>()
                .delete(&initiator_index)?;
            self.as_kv::<message_addr::Schema>().delete(&addr_index)?;
            self.as_kv::<timestamp::MessageSchema>()
                .delete(&timestamp_index)?;
            self.as_kv::<message::Schema>().delete(&index)?;
        }
        Ok(())
    }

    pub fn remove_log(&self, index: u64) -> Result<(), DbError> {
        if let Some(item) = self.as_kv::<node_log::Schema>().get(&index)? {
            let lv_index = log_level::Item {
                lv: item.level.clone(),
                index,
            };
            let timestamp_index = timestamp::Item {
                timestamp: (item.timestamp / 1_000_000) as u64,
                index,
            };

            self.as_kv::<log_level::Schema>().delete(&lv_index)?;
            self.as_kv::<timestamp::LogSchema>()
                .delete(&timestamp_index)?;
            self.as_kv::<node_log::Schema>().delete(&index)?;
        }
        Ok(())
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
        if let Some(store_limit) = self.message_store_limit {
            if index >= store_limit {
                if let Err(error) = self.remove_message(index - store_limit) {
                    log::error!("database error: {}", error);
                }
            }
        }

        let ty_index = message_ty::Item {
            ty: item.ty.clone(),
            index,
        };
        let sender_index = message_sender::Item {
            sender: item.sender.clone(),
            index,
        };
        let initiator_index = message_initiator::Item {
            initiator: item.initiator.clone(),
            index,
        };
        let addr_index = message_addr::Item {
            addr: item.remote_addr,
            index,
        };
        let timestamp_index = timestamp::Item {
            timestamp: item.timestamp,
            index,
        };
        let inner = || -> Result<(), DbError> {
            self.as_kv::<message_ty::Schema>().put(&ty_index, &())?;
            self.as_kv::<message_sender::Schema>()
                .put(&sender_index, &())?;
            self.as_kv::<message_initiator::Schema>()
                .put(&initiator_index, &())?;
            self.as_kv::<message_addr::Schema>().put(&addr_index, &())?;
            self.as_kv::<timestamp::MessageSchema>()
                .put(&timestamp_index, &())?;
            self.as_kv::<message::Schema>().put(&index, &item)?;
            Ok(())
        };
        if let Err(error) = inner() {
            log::error!("database error: {}", error);
        }
    }

    fn store_log(&self, item: node_log::Item) {
        let index = self.reserve_log_counter();
        if let Some(store_limit) = self.log_store_limit {
            if index >= store_limit {
                if let Err(error) = self.remove_log(index - store_limit) {
                    log::error!("database error: {}", error);
                }
            }
        }

        let lv_index = log_level::Item {
            lv: item.level.clone(),
            index,
        };
        let timestamp_index = timestamp::Item {
            timestamp: (item.timestamp / 1_000_000) as u64,
            index,
        };
        let inner = || -> Result<(), DbError> {
            self.as_kv::<log_level::Schema>().put(&lv_index, &())?;
            self.as_kv::<timestamp::LogSchema>()
                .put(&timestamp_index, &())?;
            self.as_kv::<node_log::Schema>().put(&index, &item)?;
            Ok(())
        };
        if let Some(log_indexer) = &self.log_indexer {
            log_indexer.write(&item.message, index);
        }
        if let Err(error) = inner() {
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
            let k = chunk::Key::begin(cn_id);
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
            let it = self
                .as_kv::<chunk::Schema>()
                .iterator(IteratorMode::Start)?;
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

        let forward = filter.direction == Some("forward".to_string());
        let direction = || {
            if forward {
                Direction::Forward
            } else {
                Direction::Reverse
            }
        };

        if filter.remote_addr.is_none()
            && filter.source_type.is_none()
            && filter.incoming.is_none()
            && filter.types.is_none()
            && filter.from.is_none()
            && filter.to.is_none()
            && filter.timestamp.is_none()
        {
            let mode = if let Some(cursor) = &filter.cursor {
                IteratorMode::From(cursor, direction())
            } else {
                if forward {
                    IteratorMode::Start
                } else {
                    IteratorMode::End
                }
            };
            let v = self
                .as_kv::<message::Schema>()
                .iterator(mode)?
                .take(limit)
                .filter_map(|(k, v)| match (k, v) {
                    (Ok(key), Ok(value)) => {
                        let preview = match details(&value, key, self.as_kv()) {
                            Ok(details) => match details.json_string() {
                                Ok(p) => p.map(|mut s| {
                                    utf8_truncate(&mut s, 100);
                                    s
                                }),
                                Err(error) => {
                                    log::error!(
                                        "Failed to deserialize message {:?}, error: {}",
                                        value,
                                        error
                                    );
                                    None
                                },
                            },
                            Err(error) => {
                                log::error!("Failed to chunks for {:?}, error: {}", value, error);
                                None
                            },
                        };
                        Some(message::MessageFrontend::new(value, key, preview))
                    },
                    (Ok(index), Err(err)) => {
                        log::warn!("Failed to load value at {:?}: {}", index, err);
                        None
                    },
                    (Err(err), _) => {
                        log::warn!("Failed to load index: {}", err);
                        None
                    },
                })
                .collect();

            Ok(v)
        } else {
            let cursor = filter
                .cursor
                .clone()
                .unwrap_or(if forward { 0 } else { u64::MAX });
            let mut iters: Vec<Box<dyn Iterator<Item = u64>>> = Vec::with_capacity(5);
            if let Some(ty) = &filter.types {
                let mut tys = Vec::new();
                for ty in ty.split(',') {
                    let ty =
                        ty.parse::<common::MessageType>()
                            .map_err(|e| DBError::SchemaError {
                                error: SchemaError::DecodeValidationError(e.to_string()),
                            })?;
                    let key = message_ty::Item { ty, index: cursor };
                    let key = key
                        .encode()
                        .map_err(|error| DBError::SchemaError { error })?;
                    let mode = rocksdb::IteratorMode::From(&key, direction().into());
                    let cf = self
                        .inner
                        .cf_handle(message_ty::Schema::name())
                        .ok_or_else(|| DBError::MissingColumnFamily {
                            name: message_ty::Schema::name(),
                        })?;
                    let mut opts = ReadOptions::default();
                    opts.set_prefix_same_as_start(true);
                    let it = self
                        .inner
                        .iterator_cf_opt(cf, opts, mode)
                        .filter_map(|(k, _)| Some(message_ty::Item::decode(&k).ok()?.index));
                    tys.push(it);
                }
                iters.push(Box::new(tys.into_iter().kmerge_by(|x, y| x > y)));
            }
            if let Some(sender) = &filter.incoming {
                let sender = common::Sender::new(*sender);
                let key = message_sender::Item {
                    sender,
                    index: cursor,
                };
                let key = key
                    .encode()
                    .map_err(|error| DBError::SchemaError { error })?;
                let mode = rocksdb::IteratorMode::From(&key, direction().into());
                let cf = self
                    .inner
                    .cf_handle(message_sender::Schema::name())
                    .ok_or_else(|| DBError::MissingColumnFamily {
                        name: message_sender::Schema::name(),
                    })?;
                let mut opts = ReadOptions::default();
                opts.set_prefix_same_as_start(true);
                let it = self
                    .inner
                    .iterator_cf_opt(cf, opts, mode)
                    .filter_map(|(k, _)| Some(message_sender::Item::decode(&k).ok()?.index));
                iters.push(Box::new(it));
            }
            if let Some(initiator) = &filter.source_type {
                let key = message_initiator::Item {
                    initiator: initiator.clone(),
                    index: cursor,
                };
                let key = key
                    .encode()
                    .map_err(|error| DBError::SchemaError { error })?;
                let mode = rocksdb::IteratorMode::From(&key, direction().into());
                let cf = self
                    .inner
                    .cf_handle(message_initiator::Schema::name())
                    .ok_or_else(|| DBError::MissingColumnFamily {
                        name: message_initiator::Schema::name(),
                    })?;
                let mut opts = ReadOptions::default();
                opts.set_prefix_same_as_start(true);
                let it = self
                    .inner
                    .iterator_cf_opt(cf, opts, mode)
                    .filter_map(|(k, _)| Some(message_initiator::Item::decode(&k).ok()?.index));
                iters.push(Box::new(it));
            }
            if let Some(addr) = &filter.remote_addr {
                let addr = addr
                    .parse::<SocketAddr>()
                    .map_err(|e| DBError::SchemaError {
                        error: SchemaError::DecodeValidationError(e.to_string()),
                    })?;
                let key = message_addr::Item {
                    addr,
                    index: cursor,
                };
                let key = key
                    .encode()
                    .map_err(|error| DBError::SchemaError { error })?;
                let mode = rocksdb::IteratorMode::From(&key, direction().into());
                let cf = self
                    .inner
                    .cf_handle(message_addr::Schema::name())
                    .ok_or_else(|| DBError::MissingColumnFamily {
                        name: message_addr::Schema::name(),
                    })?;
                let mut opts = ReadOptions::default();
                opts.set_prefix_same_as_start(true);
                let it = self
                    .inner
                    .iterator_cf_opt(cf, opts, mode)
                    .filter_map(|(k, _)| Some(message_addr::Item::decode(&k).ok()?.index));
                iters.push(Box::new(it));
            }
            if filter.from.is_some() || filter.to.is_some() {
                let mut timestamp = timestamp::Item {
                    timestamp: u64::MAX,
                    index: u64::MAX,
                };
                let mode = if let Some(end) = filter.to {
                    timestamp.timestamp = end;
                    IteratorMode::From(&timestamp, direction())
                } else {
                    if forward {
                        IteratorMode::Start
                    } else {
                        IteratorMode::End
                    }
                };
                let it = self
                    .as_kv::<timestamp::MessageSchema>()
                    .iterator(mode)?
                    .filter_map(|(k, _)| k.ok());
                if let Some(begin) = filter.from {
                    let it = it
                        .take_while(move |k| (k.timestamp >= begin) ^ forward)
                        .map(|k| k.index);
                    iters.push(Box::new(it));
                } else {
                    iters.push(Box::new(it.map(|k| k.index)));
                }
            }
            if let Some(middle) = filter.timestamp {
                let middle = timestamp::Item {
                    timestamp: middle,
                    index: u64::MAX,
                };

                let it = self
                    .as_kv::<timestamp::MessageSchema>()
                    .iterator(IteratorMode::From(&middle, direction()))?
                    .filter_map(|(k, _)| k.ok())
                    .map(|k| k.index);
                iters.push(Box::new(it));
            }

            let v = sorted_intersect(iters.as_mut_slice(), limit, forward)
                .into_iter()
                .filter_map(
                    move |index| match self.as_kv::<message::Schema>().get(&index) {
                        Ok(Some(value)) => {
                            let preview = match details(&value, index, self.as_kv()) {
                                Ok(details) => match details.json_string() {
                                    Ok(p) => p.map(|mut s| {
                                        utf8_truncate(&mut s, 100);
                                        s
                                    }),
                                    Err(error) => {
                                        log::error!(
                                            "Failed to deserialize message {:?}, error: {}",
                                            value,
                                            error
                                        );
                                        None
                                    },
                                },
                                Err(error) => {
                                    log::error!(
                                        "Failed to chunks for {:?}, error: {}",
                                        value,
                                        error
                                    );
                                    None
                                },
                            };
                            Some(message::MessageFrontend::new(value, index, preview))
                        },
                        Ok(None) => {
                            log::info!("No value at index: {}", index);
                            None
                        },
                        Err(err) => {
                            log::warn!("Failed to load value at index {}: {}", index, err);
                            None
                        },
                    },
                )
                .collect();
            Ok(v)
        }
    }

    fn fetch_message(&self, id: u64) -> Result<Option<message::MessageDetails>, Self::Error> {
        if let Some(brief) = self.as_kv::<message::Schema>().get(&id)? {
            details(&brief, id, self.as_kv()).map(Some)
        } else {
            Ok(None)
        }
    }

    fn fetch_log(&self, filter: &LogsFilter) -> Result<Vec<node_log::ItemWithId>, Self::Error> {
        let limit = filter.limit.unwrap_or(100) as usize;

        let forward = filter.direction == Some("forward".to_string());
        let direction = || {
            if forward {
                Direction::Forward
            } else {
                Direction::Reverse
            }
        };

        if let Some(query) = &filter.query {
            let result = self
                .log_indexer
                .as_ref()
                .ok_or(DbError::NoLogIndexer)?
                .read(query, limit)?
                .filter_map(
                    |(_score, id)| match self.as_kv::<node_log::Schema>().get(&id) {
                        Ok(Some(value)) => Some(node_log::ItemWithId::new(value, id)),
                        Ok(None) => {
                            log::info!("No value at index: {}", id);
                            None
                        },
                        Err(err) => {
                            log::warn!("Failed to load value at index {}: {}", id, err);
                            None
                        },
                    },
                )
                .collect();
            return Ok(result);
        }

        if filter.log_level.is_none()
            && filter.from.is_none()
            && filter.to.is_none()
            && filter.timestamp.is_none()
        {
            let mode = if let Some(cursor) = &filter.cursor {
                IteratorMode::From(cursor, direction())
            } else {
                if forward {
                    IteratorMode::Start
                } else {
                    IteratorMode::End
                }
            };
            let vec = self
                .as_kv::<node_log::Schema>()
                .iterator(mode)?
                .filter_map(|(k, v)| match (k, v) {
                    (Ok(id), Ok(item)) => Some(node_log::ItemWithId::new(item, id)),
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
        } else {
            let mut iters: Vec<Box<dyn Iterator<Item = u64>>> = Vec::with_capacity(5);

            if let Some(lv) = &filter.log_level {
                let cursor = filter
                    .cursor
                    .clone()
                    .unwrap_or(if forward { 0 } else { u64::MAX });
                let mut lvs = Vec::new();
                for lv in lv.split(',') {
                    let lv =
                        lv.parse::<node_log::LogLevel>()
                            .map_err(|e| DBError::SchemaError {
                                error: SchemaError::DecodeValidationError(e.to_string()),
                            })?;
                    let key = log_level::Item { lv, index: cursor };
                    let key = key
                        .encode()
                        .map_err(|error| DBError::SchemaError { error })?;
                    let mode = rocksdb::IteratorMode::From(&key, direction().into());
                    let cf = self
                        .inner
                        .cf_handle(log_level::Schema::name())
                        .ok_or_else(|| DBError::MissingColumnFamily {
                            name: log_level::Schema::name(),
                        })?;
                    let mut opts = ReadOptions::default();
                    opts.set_prefix_same_as_start(true);
                    let it = self
                        .inner
                        .iterator_cf_opt(cf, opts, mode)
                        .filter_map(|(k, _)| Some(log_level::Item::decode(&k).ok()?.index));
                    lvs.push(it);
                }
                iters.push(Box::new(lvs.into_iter().kmerge_by(|x, y| x > y)));
            }
            if filter.from.is_some() || filter.to.is_some() {
                let mut timestamp = timestamp::Item {
                    timestamp: u64::MAX,
                    index: u64::MAX,
                };
                let mode = if let Some(end) = filter.to {
                    timestamp.timestamp = end;
                    IteratorMode::From(&timestamp, direction())
                } else {
                    if forward {
                        IteratorMode::Start
                    } else {
                        IteratorMode::End
                    }
                };
                let it = self
                    .as_kv::<timestamp::LogSchema>()
                    .iterator(mode)?
                    .filter_map(|(k, _)| k.ok());
                if let Some(begin) = filter.from {
                    let it = it
                        .take_while(move |k| (k.timestamp >= begin) ^ forward)
                        .map(|k| k.index);
                    iters.push(Box::new(it));
                } else {
                    iters.push(Box::new(it.map(|k| k.index)));
                }
            }
            if let Some(middle) = filter.timestamp {
                let middle = timestamp::Item {
                    timestamp: middle,
                    index: u64::MAX,
                };

                let it = self
                    .as_kv::<timestamp::LogSchema>()
                    .iterator(IteratorMode::From(&middle, direction()))?
                    .filter_map(|(k, _)| k.ok())
                    .map(|k| k.index);
                iters.push(Box::new(it));
            }

            let v = sorted_intersect(iters.as_mut_slice(), limit, forward)
                .into_iter()
                .filter_map(move |id| match self.as_kv::<node_log::Schema>().get(&id) {
                    Ok(Some(item)) => Some(node_log::ItemWithId::new(item, id)),
                    Ok(None) => {
                        log::info!("No value at index: {}", id);
                        None
                    },
                    Err(err) => {
                        log::warn!("Failed to load value at index {}: {}", id, err);
                        None
                    },
                })
                .collect();
            Ok(v)
        }
    }

    fn compact(&self) {
        let cf_names = [
            connection::Schema::name(),
            chunk::Schema::name(),
            message::Schema::name(),
            node_log::Schema::name(),
            message_ty::Schema::name(),
            message_sender::Schema::name(),
            message_initiator::Schema::name(),
            message_addr::Schema::name(),
            timestamp::MessageSchema::name(),
            log_level::Schema::name(),
            timestamp::LogSchema::name(),
        ];

        for cf_name in &cf_names {
            if let Some(cf) = self.inner.cf_handle(cf_name) {
                if let Ok(()) = self.inner.flush_cf(cf) {
                    self.inner.compact_range_cf::<[u8; 0], [u8; 0]>(cf, None, None);
                }
            }
        }

        log::info!("compact");
    }
}

fn details(
    message_item: &message::Item,
    id: u64,
    db: &(impl KeyValueStoreBackend<chunk::Schema> + KeyValueStoreWithSchemaIterator<chunk::Schema>),
) -> Result<message::MessageDetails, DbError> {
    let mut chunks = Vec::new();
    for key in message_item.chunks() {
        if let Some(c) = db.get(&key)? {
            chunks.push(c);
        } else {
            break;
        }
    }
    Ok(message::MessageDetails::new(id, &message_item.ty, &chunks))
}

fn utf8_truncate(input: &mut String, max_size: usize) {
    let mut m = max_size;
    while !input.is_char_boundary(m) {
        m -= 1;
    }
    input.truncate(m);
}
