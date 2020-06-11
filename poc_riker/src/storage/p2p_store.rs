use storage::persistent::{KeyValueStoreWithSchema, KeyValueSchema};
use std::sync::{
    Arc,
    atomic::{Ordering, AtomicU64},
};
use rocksdb::DB;
use storage::{StorageError, IteratorMode, Direction};
use crate::utility::p2p_message::P2pMessage;
use failure::Error;

pub type P2PStoreKV = dyn KeyValueStoreWithSchema<P2pStore> + Sync + Send;


#[derive(Clone)]
pub struct P2pStore {
    kv: Arc<P2PStoreKV>,
    seq: Arc<AtomicU64>,
    count: Arc<AtomicU64>,
}

impl std::fmt::Debug for P2pStore {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "P2PStore")
    }
}

#[allow(dead_code)]
impl P2pStore {
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

    pub fn store_message(&self, msg: &P2pMessage) -> Result<u64, StorageError> {
        let index = self.fetch_index();
        self.kv.put(&index, &msg)?;
        self.inc_count();
        Ok(index)
    }

    pub fn get_range(&self, offset_id: Option<u64>, count: usize) -> Result<Vec<P2pMessage>, Error> {
        let mut ret = Vec::with_capacity(count);
        let offset_id = offset_id.unwrap_or(std::u64::MAX);
        let mode = IteratorMode::From(&offset_id, Direction::Reverse);
        let iter = self.kv.iterator(mode)?
            .take(count);
        for (key, value) in iter {
            let (key, mut value) = (key?, value?);
            value.id = Some(key);
            ret.push(value);
        }
        Ok(ret)
    }
}

impl KeyValueSchema for P2pStore {
    type Key = u64;
    type Value = P2pMessage;

    fn name() -> &'static str {
        "p2p_store"
    }
}