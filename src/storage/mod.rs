pub mod p2p_store;
pub mod rpc_store;

use rocksdb::{DB, ColumnFamilyDescriptor, Options};
use failure::Error;
use std::sync::Arc;
use std::path::Path;
use storage::persistent::{KeyValueSchema};
use crate::storage::p2p_store::P2PStore;
use crate::storage::rpc_store::RPCStore;

#[derive(Clone)]
pub struct Storage {
    db: Arc<DB>
}

impl Storage {
    fn cfs() -> Vec<ColumnFamilyDescriptor> {
        vec![
            self::p2p_store::P2PStore::descriptor(),
        ]
    }

    pub fn open<T: AsRef<Path>>(path: T) -> Result<Self, Error> {
        let mut opts = Options::default();
        opts.create_missing_column_families(true);
        opts.create_if_missing(true);
        let db = Arc::new(DB::open_cf_descriptors(&opts, path, Self::cfs())?);
        Ok(Self {
            db
        })
    }

    pub fn kv(&self) -> Arc<DB> {
        self.db.clone()
    }

    pub fn p2p_store(&self) -> P2PStore { P2PStore::new(self.kv()) }
    pub fn rpc_store(&self) -> RPCStore { RPCStore::new(self.kv()) }
}