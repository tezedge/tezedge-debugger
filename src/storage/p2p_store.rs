use storage::persistent::{KeyValueStoreWithSchema, KeyValueSchema};
use std::sync::{
    Arc,
    atomic::{Ordering, AtomicU64},
};
use rocksdb::DB;
use crate::utility::p2p_message::P2PMessage;
use storage::StorageError;

pub type P2PStoreKV = dyn KeyValueStoreWithSchema<P2PStore> + Sync + Send;


#[derive(Clone)]
pub struct P2PStore {
    kv: Arc<P2PStoreKV>,
    seq: Arc<AtomicU64>,
    count: Arc<AtomicU64>,
}

impl std::fmt::Debug for P2PStore {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "P2PStore")
    }
}

#[allow(dead_code)]
impl P2PStore {
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

    pub fn store_message(&self, msg: &P2PMessage) -> Result<u64, StorageError> {
        let index = self.fetch_index();
        self.kv.put(&index, &msg)?;
        self.inc_count();
        Ok(index)
    }
}

impl KeyValueSchema for P2PStore {
    type Key = u64;
    type Value = P2PMessage;

    fn name() -> &'static str {
        "p2p_store"
    }
}