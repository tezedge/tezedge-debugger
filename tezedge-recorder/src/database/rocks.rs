// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{path::Path, sync::Arc, iter};
use rocksdb::{DB, Cache};
use storage::persistent::{self, DBError, DbConfiguration, KeyValueSchema, KeyValueStoreWithSchema};
use thiserror::Error;
use super::{Database, DatabaseNew, connection, chunk, message};

#[derive(Error, Debug)]
#[error("{}", _0)]
pub struct DbError(DBError);

pub struct Db {
    _cache: Cache,
    inner: DB,
}

impl Db {
    fn as_kv<S>(&self) -> &impl KeyValueStoreWithSchema<S>
    where
        S: KeyValueSchema,
    {
        &self.inner
    }
}

impl DatabaseNew for Db {
    type Error = DbError;

    fn open<P>(path: P) -> Result<Arc<Self>, Self::Error>
    where
        P: AsRef<Path>,
    {
        let cache = Cache::new_lru_cache(1).map_err(Into::into).map_err(DbError)?;

        let cfs = iter::once(connection::Schema::descriptor(&cache));
        let inner = persistent::open_kv(path, cfs, &DbConfiguration::default()).map_err(DbError)?;

        Ok(Arc::new(Db { _cache: cache, inner }))
    }
}

impl Database for Db {
    fn store_connection(&self, item: connection::Item) {
        log::info!("connection: {:?}", item);

        let (key, value) = item.split();
        if let Err(error) = self.as_kv::<connection::Schema>().put(&key, &value) {
            log::error!("database error {}", error);
        }
    }

    fn store_chunk(&self, item: chunk::Item) {
        log::debug!("unimplemented {}", item.len());
    }

    fn store_message(&self, item: message::Item) {
        let _ = item;
    }
}
