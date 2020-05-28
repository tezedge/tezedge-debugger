pub mod p2p_store;
pub mod rpc_store;
pub mod log_store;

use rocksdb::{DB, ColumnFamilyDescriptor, Options};
use failure::Error;
use std::sync::Arc;
use std::path::Path;
use storage::persistent::{KeyValueSchema};
use crate::storage::p2p_store::P2pStore;
use crate::storage::rpc_store::RpcStore;
use crate::storage::log_store::LogStore;

#[derive(Clone)]
pub struct Storage {
    db: Arc<DB>,
    p2p: P2pStore,
    rpc: RpcStore,
    log: LogStore,
}

impl Storage {
    fn cfs() -> Vec<ColumnFamilyDescriptor> {
        vec![
            self::p2p_store::P2pStore::descriptor(),
            self::rpc_store::RpcStore::descriptor(),
            self::log_store::LogStore::descriptor(),
        ]
    }

    pub fn open<T: AsRef<Path>>(path: T) -> Result<Self, Error> {
        let mut opts = Options::default();
        opts.create_missing_column_families(true);
        opts.create_if_missing(true);
        let db = Arc::new(DB::open_cf_descriptors(&opts, path, Self::cfs())?);
        Ok(Self {
            db: db.clone(),
            p2p: P2pStore::new(db.clone()),
            rpc: RpcStore::new(db.clone()),
            log: LogStore::new(db.clone()),
        })
    }

    pub fn kv(&self) -> Arc<DB> {
        self.db.clone()
    }

    pub fn p2p_store(&self) -> P2pStore { self.p2p.clone() }
    pub fn rpc_store(&self) -> RpcStore { self.rpc.clone() }
    pub fn log_store(&self) -> LogStore { self.log.clone() }
}