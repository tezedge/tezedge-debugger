use storage::persistent::{KeyValueStoreWithSchema, KeyValueSchema};
use std::sync::{
    Arc,
    atomic::{Ordering, AtomicU64},
};
use rocksdb::DB;
use storage::{StorageError, IteratorMode, Direction};
use crate::utility::http_message::RPCMessage;
use failure::Error;

pub type RPCStoreKV = dyn KeyValueStoreWithSchema<RpcStore> + Sync + Send;


#[derive(Clone)]
pub struct RpcStore {
    kv: Arc<RPCStoreKV>,
    seq: Arc<AtomicU64>,
    count: Arc<AtomicU64>,
}

impl std::fmt::Debug for RpcStore {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "RPCStore")
    }
}

#[allow(dead_code)]
impl RpcStore {
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

    pub fn get_range(&self, offset_id: Option<u64>, count: usize) -> Result<Vec<RPCMessage>, Error> {
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

impl KeyValueSchema for RpcStore {
    type Key = u64;
    type Value = RPCMessage;

    fn name() -> &'static str {
        "rpc_store"
    }
}