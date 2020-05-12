use storage::persistent::{KeyValueStoreWithSchema, KeyValueSchema};
use std::sync::Arc;
use rocksdb::{DB};
use std::sync::atomic::{AtomicU64, Ordering};
use crate::actors::logs_message::LogMessage;
use storage::{StorageError, IteratorMode, Direction};

pub type LogStorageKV = dyn KeyValueStoreWithSchema<LogStorage> + Sync + Send;

#[derive(Clone)]
pub struct LogStorage {
    kv: Arc<LogStorageKV>,
    count: Arc<AtomicU64>,
    seq: Arc<AtomicU64>,
}

impl LogStorage {
    pub fn new(kv: Arc<DB>) -> Self {
        Self {
            kv,
            count: Arc::new(AtomicU64::new(0)),
            seq: Arc::new(AtomicU64::new(0)),
        }
    }

    fn count(&self) -> u64 {
        self.count.load(Ordering::SeqCst)
    }

    fn start(&self) -> u64 {
        self.seq.load(Ordering::SeqCst).saturating_add(1)
    }

    fn inc_count(&self) {
        self.count.fetch_add(1, Ordering::SeqCst);
    }

    fn index_next(&self) -> u64 {
        self.seq.fetch_add(1, Ordering::SeqCst)
    }

    fn index(&self) -> u64 {
        self.seq.load(Ordering::SeqCst)
    }

    pub fn store_message(&mut self, msg: &mut LogMessage) -> Result<u64, StorageError> {
        let index = self.index_next();
        msg.id = Some(index);
        self.kv.put(&index, &msg)?;
        self.inc_count();
        Ok(index)
    }

    pub fn get_reverse_range(&self, offset_id: u64, count: usize) -> Result<Vec<LogMessage>, StorageError> {
        let offset = std::u64::MAX.saturating_sub(offset_id);
        let mode = IteratorMode::From(&offset, Direction::Reverse);
        let ret = self.kv.iterator(mode)?.take(count)
            .fold(Vec::with_capacity(count), |mut acc, (_, value)| {
                match value {
                    Ok(msg) => acc.push(msg),
                    Err(err) => log::error!("Failed to deserialize content of database: {}", err),
                }
                acc
            });
        Ok(ret)
    }

    fn load_indexes<'a>(&self, indexes: Box<dyn Iterator<Item=u64> + 'a>, limit: usize) -> impl Iterator<Item=LogMessage> + 'a {
        let kv = self.kv.clone();
        let mut count = 0;
        indexes.filter_map(move |index| {
            match kv.get(&index) {
                Ok(Some(value)) => {
                    count += 1;
                    Some(value)
                }
                Ok(None) => {
                    log::info!("No value at index: {}", index);
                    None
                }
                Err(err) => {
                    log::warn!("Failed to load value at index {}: {}",index, err);
                    None
                }
            }
        }).take(limit)
    }
}

impl KeyValueSchema for LogStorage {
    type Key = u64;
    type Value = LogMessage;

    fn name() -> &'static str { "log_storage" }
}