use storage::persistent::{KeyValueStoreWithSchema, KeyValueSchema};
use std::sync::Arc;
use rocksdb::{DB};
use std::sync::atomic::{AtomicU64, Ordering};
use crate::actors::logs_message::LogMessage;
use storage::{StorageError, IteratorMode, Direction};
use crate::storage::log_storage::secondary_indexes::{LevelIndex, LogLevel};
use crate::storage::secondary_index::SecondaryIndex;
use crate::storage::sorted_intersect::sorted_intersect;

pub type LogStorageKV = dyn KeyValueStoreWithSchema<LogStorage> + Sync + Send;

#[derive(Debug, Default, Clone)]
pub struct LogFilters {
    pub level: Option<LogLevel>,
}

impl LogFilters {
    pub fn empty(&self) -> bool {
        self.level.is_none()
    }
}

#[derive(Clone)]
pub struct LogStorage {
    kv: Arc<LogStorageKV>,
    level_index: LevelIndex,
    count: Arc<AtomicU64>,
    seq: Arc<AtomicU64>,
}

impl LogStorage {
    pub fn new(kv: Arc<DB>) -> Self {
        Self {
            kv: kv.clone(),
            level_index: LevelIndex::new(kv),
            count: Arc::new(AtomicU64::new(0)),
            seq: Arc::new(AtomicU64::new(0)),
        }
    }

    fn index(&self) -> u64 {
        self.seq.load(Ordering::SeqCst)
    }

    fn inc_count(&self) {
        self.count.fetch_add(1, Ordering::SeqCst);
    }

    pub fn reserve_index(&self) -> u64 {
        self.seq.fetch_add(1, Ordering::SeqCst)
    }

    pub fn make_indexes(&self, _primary_index: u64, _value: &LogMessage) -> Result<(), StorageError> {
        Ok(())
    }

    pub fn delete_indexes(&self, _primary_index: u64, _value: &LogMessage) -> Result<(), StorageError> {
        Ok(())
    }

    pub fn put_message(&self, index: u64, msg: &LogMessage) -> Result<(), StorageError> {
        if self.kv.contains(&index)? {
            self.kv.merge(&index, &msg)?;
        } else {
            self.kv.put(&index, &msg)?;
            self.inc_count();
        }
        self.make_indexes(index, msg)?;
        Ok(())
    }

    pub fn store_message(&self, msg: &LogMessage) -> Result<u64, StorageError> {
        let index = self.reserve_index();
        self.kv.put(&index, &msg)?;
        self.make_indexes(index, &msg)?;
        self.inc_count();
        Ok(index)
    }

    pub fn get_cursor(&self, cursor_index: Option<u64>, limit: usize, filters: LogFilters) -> Result<Vec<LogMessage>, StorageError> {
        let mut ret = Vec::with_capacity(limit);
        if filters.empty() {
            ret.extend(self.cursor_iterator(cursor_index)?.map(|(_, v)| v).take(limit));
        } else {
            let mut iters: Vec<Box<dyn Iterator<Item=u64>>> = Default::default();
            if let Some(level) = filters.level {
                iters.push(self.level_iterator(cursor_index, level)?);
            }
            ret.extend(self.load_indexes(sorted_intersect(iters, limit).into_iter()));
        }
        Ok(ret)
    }

    fn cursor_iterator<'a>(&'a self, cursor_index: Option<u64>) -> Result<Box<dyn 'a + Iterator<Item=(u64, LogMessage)>>, StorageError> {
        Ok(Box::new(self.kv.iterator(IteratorMode::From(&cursor_index.unwrap_or(std::u64::MAX), Direction::Reverse))?
            .filter_map(|(k, v)| {
                k.ok().and_then(|key| Some((key, v.ok()?)))
            })))
    }

    pub fn level_iterator<'a>(&'a self, cursor_index: Option<u64>, level: LogLevel) -> Result<Box<dyn 'a + Iterator<Item=u64>>, StorageError> {
        Ok(Box::new(self.level_index.get_concrete_prefix_iterator(&cursor_index.unwrap_or(std::u64::MAX), level)?
            .filter_map(|(_, value)| {
                value.ok()
            })))
    }

    fn load_indexes<Iter: 'static + Iterator<Item=u64>>(&self, indexes: Iter) -> impl Iterator<Item=LogMessage> + 'static {
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
        })
    }
}

impl KeyValueSchema for LogStorage {
    type Key = u64;
    type Value = LogMessage;

    fn name() -> &'static str { "log_storage" }
}

pub(crate) mod secondary_indexes {
    use storage::persistent::{KeyValueStoreWithSchema, KeyValueSchema, BincodeEncoded};
    use std::{
        sync::Arc,
        str::FromStr,
    };
    use rocksdb::{DB, ColumnFamilyDescriptor, Options, SliceTransform};
    use crate::storage::LogStorage;
    use serde::{Serialize, Deserialize};
    use failure::{Fail};
    use crate::storage::secondary_index::SecondaryIndex;

    pub type LevelIndexKV = dyn KeyValueStoreWithSchema<LevelIndex> + Sync + Send;

    #[derive(Clone)]
    pub struct LevelIndex {
        kv: Arc<LevelIndexKV>,
    }

    impl LevelIndex {
        pub fn new(kv: Arc<DB>) -> Self {
            Self { kv }
        }
    }

    impl AsRef<(dyn KeyValueStoreWithSchema<LevelIndex> + 'static)> for LevelIndex {
        fn as_ref(&self) -> &(dyn KeyValueStoreWithSchema<LevelIndex> + 'static) {
            self.kv.as_ref()
        }
    }

    impl KeyValueSchema for LevelIndex {
        type Key = LogLevelKey;
        type Value = <LogStorage as KeyValueSchema>::Key;

        fn descriptor() -> ColumnFamilyDescriptor {
            let mut cf_opts = Options::default();
            cf_opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(std::mem::size_of::<LogLevel>()));
            cf_opts.set_memtable_prefix_bloom_ratio(0.2);
            ColumnFamilyDescriptor::new(Self::name(), cf_opts)
        }

        fn name() -> &'static str {
            "log_level_index"
        }
    }

    impl SecondaryIndex<LogStorage> for LevelIndex {
        type FieldType = LogLevel;

        fn accessor(value: &<LogStorage as KeyValueSchema>::Value) -> Option<Self::FieldType> {
            match value.level.parse() {
                Ok(level) => Some(level),
                Err(_) => {
                    log::warn!("Got invalid log level {}", value.level);
                    None
                }
            }
        }

        fn make_index(key: &<LogStorage as KeyValueSchema>::Key, value: Self::FieldType) -> LogLevelKey {
            LogLevelKey::new(value, key.clone())
        }

        fn make_prefix_index(value: Self::FieldType) -> LogLevelKey {
            LogLevelKey::prefix(value)
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct LogLevelKey {
        pub level: LogLevel,
        pub index: u64,
    }

    impl LogLevelKey {
        pub fn new(level: LogLevel, index: u64) -> Self {
            Self {
                level,
                index: std::u64::MAX.saturating_sub(index),
            }
        }

        pub fn prefix(level: LogLevel) -> Self {
            Self {
                level,
                index: 0,
            }
        }
    }

    impl BincodeEncoded for LogLevelKey {}

    #[repr(u8)]
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum LogLevel {
        Trace = 0x1 << 0,
        Debug = 0x1 << 1,
        Info = 0x1 << 2,
        Notice = 0x1 << 3,
        Warning = 0x1 << 4,
        Error = 0x1 << 5,
        Fatal = 0x1 << 6,
    }

    #[derive(Debug, Fail)]
    #[fail(display = "Invalid log level {}", _0)]
    pub struct ParseLogLevel(String);

    impl FromStr for LogLevel {
        type Err = ParseLogLevel;

        fn from_str(level: &str) -> Result<Self, Self::Err> {
            let level = level.to_lowercase();
            Ok(match level.as_ref() {
                "trace" => Self::Trace,
                "debug" => Self::Debug,
                "info" => Self::Info,
                "notice" => Self::Notice,
                "warn" | "warning" => Self::Warning,
                "error" => Self::Error,
                "fatal" => Self::Fatal,
                _ => return Err(ParseLogLevel(level)),
            })
        }
    }
}