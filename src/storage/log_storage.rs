use storage::persistent::{KeyValueStoreWithSchema, KeyValueSchema};
use std::sync::Arc;
use rocksdb::{DB};
use std::sync::atomic::{AtomicU64, Ordering};
use crate::actors::logs_message::LogMessage;
use storage::{StorageError, IteratorMode, Direction};
use failure::Error;
use crate::storage::log_storage::secondary_indexes::{LevelIndex, TimeStampIndex, TimeStampLevelIndex};
use crate::storage::secondary_index::SecondaryIndex;

pub type LogStorageKV = dyn KeyValueStoreWithSchema<LogStorage> + Sync + Send;

#[derive(Clone)]
pub struct LogStorage {
    kv: Arc<LogStorageKV>,
    level_index: LevelIndex,
    timestamp_index: TimeStampIndex,
    timestamp_level_index: TimeStampLevelIndex,
    count: Arc<AtomicU64>,
    seq: Arc<AtomicU64>,
}

impl LogStorage {
    pub fn new(kv: Arc<DB>) -> Self {
        Self {
            kv: kv.clone(),
            level_index: LevelIndex::new(kv.clone()),
            timestamp_index: TimeStampIndex::new(kv.clone()),
            timestamp_level_index: TimeStampLevelIndex::new(kv.clone()),
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

    pub fn ts_index(&mut self) -> &mut TimeStampIndex {
        &mut self.timestamp_index
    }

    pub fn make_indexes(&mut self, primary_index: &u64, value: &LogMessage) -> Result<(), StorageError> {
        self.level_index.store_index(primary_index, value)?;
        self.timestamp_index.store_index(primary_index, value)?;
        self.timestamp_level_index.store_index(primary_index, value)
    }

    pub fn delete_indexes(&mut self, primary_index: &u64, value: &LogMessage) -> Result<(), StorageError> {
        self.level_index.delete_index(primary_index, value)?;
        self.timestamp_index.delete_index(primary_index, value)?;
        self.timestamp_level_index.delete_index(primary_index, value)
    }

    pub fn store_message(&mut self, msg: &mut LogMessage) -> Result<u64, StorageError> {
        let index = self.index_next();
        msg.id = Some(index);
        self.make_indexes(&index, &msg)?;
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

    pub fn get_timestamp_range(&self, timestamp: u128, count: usize) -> Result<Vec<LogMessage>, Error> {
        let mut iter = self.timestamp_index.get_prefix_iterator(timestamp)?;
        let index = iter.next();
        if let Some((_, index)) = index {
            let pk = index?;
            let mode = IteratorMode::From(&pk, Direction::Reverse);
            Ok(self.kv.iterator(mode)?.take(count)
                .fold(Vec::with_capacity(count), |mut acc, (_, value)| {
                    match value {
                        Ok(msg) => acc.push(msg),
                        Err(err) => log::error!("Failed to deserialize content of database: {}", err),
                    }
                    acc
                }))
        } else {
            Ok(Vec::new())
        }
    }

    pub fn get_level_range(&self, level: &str, offset: usize, count: usize) -> Result<Vec<LogMessage>, Error> {
        let level = level.parse()?;
        let idx = self.level_index.get_prefix_iterator(level)?
            .filter_map(|(_, val)| val.ok())
            .skip(offset);
        Ok(self.load_indexes(Box::new(idx), count as usize)
            .fold(Vec::with_capacity(count as usize), |mut acc, value| {
                acc.push(value);
                acc
            }))
    }

    pub fn get_timestamp_level_range(&self, level: &str, timestamp: u128, count: usize) -> Result<Vec<LogMessage>, Error> {
        let level = level.parse()?;
        let idx = self.timestamp_level_index.get_iterator(&std::u64::MAX, (level, timestamp), Direction::Reverse)?
            .filter_map(|(_, val)| val.ok());
        Ok(self.load_indexes(Box::new(idx), count as usize)
            .fold(Vec::with_capacity(count as usize), |mut acc, value| {
                acc.push(value);
                acc
            }))
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

    // TimeStamp Index

    pub type TimeStampIndexKV = dyn KeyValueStoreWithSchema<TimeStampIndex> + Sync + Send;

    #[derive(Clone)]
    pub struct TimeStampIndex {
        kv: Arc<TimeStampIndexKV>
    }

    impl TimeStampIndex {
        pub fn new(kv: Arc<DB>) -> Self {
            Self { kv }
        }
    }

    impl AsRef<(dyn KeyValueStoreWithSchema<TimeStampIndex> + 'static)> for TimeStampIndex {
        fn as_ref(&self) -> &(dyn KeyValueStoreWithSchema<TimeStampIndex> + 'static) {
            self.kv.as_ref()
        }
    }

    impl KeyValueSchema for TimeStampIndex {
        type Key = TimeStampKey;
        type Value = <LogStorage as KeyValueSchema>::Key;

        fn descriptor() -> ColumnFamilyDescriptor {
            let mut cf_opts = Options::default();
            cf_opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(std::mem::size_of::<u128>()));
            cf_opts.set_memtable_prefix_bloom_ratio(0.2);
            ColumnFamilyDescriptor::new(Self::name(), cf_opts)
        }

        fn name() -> &'static str {
            "log_ts_index"
        }
    }

    impl SecondaryIndex<LogStorage> for TimeStampIndex {
        type FieldType = u128;

        fn accessor(value: &<LogStorage as KeyValueSchema>::Value) -> Option<Self::FieldType> {
            Some(value.date)
        }

        fn make_index(key: &<LogStorage as KeyValueSchema>::Key, value: Self::FieldType) -> TimeStampKey {
            TimeStampKey::new(value, key.clone())
        }

        fn make_prefix_index(value: Self::FieldType) -> TimeStampKey {
            TimeStampKey::prefix(value)
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct TimeStampKey {
        pub timestamp: u128,
        pub index: u64,
    }

    impl TimeStampKey {
        pub fn new(timestamp: u128, index: u64) -> Self {
            Self {
                timestamp,
                index: std::u64::MAX.saturating_sub(index),
            }
        }

        pub fn prefix(timestamp: u128) -> Self {
            Self {
                timestamp,
                index: 0,
            }
        }
    }

    impl BincodeEncoded for TimeStampKey {}

    // Combined index

    pub type TimeStampLevelIndexKV = dyn KeyValueStoreWithSchema<TimeStampLevelIndex> + Sync + Send;

    #[derive(Clone)]
    pub struct TimeStampLevelIndex {
        kv: Arc<TimeStampLevelIndexKV>
    }

    impl TimeStampLevelIndex {
        pub fn new(kv: Arc<DB>) -> Self {
            Self { kv }
        }
    }

    impl AsRef<(dyn KeyValueStoreWithSchema<TimeStampLevelIndex> + 'static)> for TimeStampLevelIndex {
        fn as_ref(&self) -> &(dyn KeyValueStoreWithSchema<TimeStampLevelIndex> + 'static) {
            self.kv.as_ref()
        }
    }

    impl KeyValueSchema for TimeStampLevelIndex {
        type Key = TimeStampLevelKey;
        type Value = <LogStorage as KeyValueSchema>::Key;

        fn descriptor() -> ColumnFamilyDescriptor {
            let mut cf_opts = Options::default();
            cf_opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(std::mem::size_of::<LogLevel>()));
            cf_opts.set_memtable_prefix_bloom_ratio(0.2);
            ColumnFamilyDescriptor::new(Self::name(), cf_opts)
        }

        fn name() -> &'static str {
            "log_level_ts_index"
        }
    }

    impl SecondaryIndex<LogStorage> for TimeStampLevelIndex {
        type FieldType = (LogLevel, u128);

        fn accessor(value: &<LogStorage as KeyValueSchema>::Value) -> Option<Self::FieldType> {
            match value.level.parse() {
                Ok(level) => Some((level, value.date)),
                Err(_) => {
                    log::warn!("Got invalid log level {}", value.level);
                    None
                }
            }
        }

        fn make_index(key: &<LogStorage as KeyValueSchema>::Key, value: Self::FieldType) -> TimeStampLevelKey {
            TimeStampLevelKey::new(value.0, value.1, key.clone())
        }

        fn make_prefix_index(value: Self::FieldType) -> TimeStampLevelKey {
            TimeStampLevelKey::prefix(value.0, value.1)
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct TimeStampLevelKey {
        pub level: LogLevel,
        pub ts: u128,
        pub index: u64,
    }

    impl TimeStampLevelKey {
        pub fn new(level: LogLevel, ts: u128, index: u64) -> Self {
            Self {
                level,
                ts,
                index: std::u64::MAX.saturating_sub(index),
            }
        }

        pub fn prefix(level: LogLevel, ts: u128) -> Self {
            Self {
                level,
                ts,
                index: 0,
            }
        }
    }

    impl BincodeEncoded for TimeStampLevelKey {}
}