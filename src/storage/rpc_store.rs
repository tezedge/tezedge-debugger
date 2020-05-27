use storage::persistent::{KeyValueStoreWithSchema, KeyValueSchema};
use std::sync::{
    Arc,
    atomic::{Ordering, AtomicU64},
};
use rocksdb::DB;
use crate::utility::p2p_message::P2PMessage;
use storage::StorageError;
use failure::_core::fmt::Formatter;
use crate::utility::http_message::{HttpMessage, RPCMessage};

pub type RPCStoreKV = dyn KeyValueStoreWithSchema<RPCStore> + Sync + Send;


#[derive(Clone)]
pub struct RPCStore {
    kv: Arc<RPCStoreKV>,
    seq: Arc<AtomicU64>,
    count: Arc<AtomicU64>,
}

impl std::fmt::Debug for RPCStore {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "RPCStore")
    }
}

impl RPCStore {
    pub fn new(kv: Arc<DB>) -> Self {
        Self {
            kv,
            seq: Arc::new(AtomicU64::new(0)),
            count: Arc::new(AtomicU64::new(0)),
        }
    }

    fn count(&self) -> u64 {
        self.count.load(Ordering::SeqCst)
    }

    fn inc_count(&self) -> u64 {
        self.count.fetch_add(1, Ordering::SeqCst)
    }

    fn index(&self) -> u64 {
        self.seq.load(Ordering::SeqCst)
    }

    fn fetch_index(&self) -> u64 {
        self.seq.fetch_add(1, Ordering::SeqCst)
    }

    pub fn store_message(&self, msg: &RPCMessage) -> Result<u64, StorageError> {
        let index = self.fetch_index();
        self.kv.put(&index, &msg)?;
        self.inc_count();
        Ok(index)
    }
}

impl KeyValueSchema for RPCStore {
    type Key = u64;
    type Value = RPCMessage;

    fn name() -> &'static str {
        "p2p_store"
    }
}