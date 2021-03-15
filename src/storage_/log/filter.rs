use std::{sync::Arc, mem};
use storage::{persistent::{KeyValueSchema, KeyValueStoreWithSchema}, StorageError};
use rocksdb::{DB, ColumnFamilyDescriptor, Options, SliceTransform, Cache};
use super::{
    message::Schema,
    SecondaryIndex,
    SecondaryIndices,
    indices::{NodeNameKey, NodeName, LogLevelKey, LogLevel, TimestampKey},
    sorted_intersect::sorted_intersect,
    KeyValueSchemaExt,
    ColumnFamilyDescriptorExt,
};

pub struct NodeNameSchema;

impl KeyValueSchema for NodeNameSchema {
    type Key = NodeNameKey;
    type Value = u64;

    fn descriptor(_cache: &Cache) -> ColumnFamilyDescriptor {
        let mut cf_opts = Options::default();
        cf_opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(mem::size_of::<u16>()));
        cf_opts.set_memtable_prefix_bloom_ratio(0.2);
        ColumnFamilyDescriptor::new(Self::name(), cf_opts)
    }

    fn name() -> &'static str {
        "log_node_name_index"
    }
}

impl KeyValueSchemaExt for NodeNameSchema {
    fn short_id() -> u16 {
        0x0010
    }
}

pub struct LogLevelSchema;

impl KeyValueSchema for LogLevelSchema {
    type Key = LogLevelKey;
    type Value = u64;

    fn descriptor(_cache: &Cache) -> ColumnFamilyDescriptor {
        let mut cf_opts = Options::default();
        cf_opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(mem::size_of::<u8>()));
        cf_opts.set_memtable_prefix_bloom_ratio(0.2);
        ColumnFamilyDescriptor::new(Self::name(), cf_opts)
    }

    fn name() -> &'static str {
        "log_level_index"
    }
}

impl KeyValueSchemaExt for LogLevelSchema {
    fn short_id() -> u16 {
        0x0011
    }
}

pub struct LogTimestampSchema;

impl KeyValueSchema for LogTimestampSchema {
    type Key = TimestampKey;
    type Value = u64;

    fn descriptor(_cache: &Cache) -> ColumnFamilyDescriptor {
        let mut cf_opts = Options::default();
        cf_opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(mem::size_of::<u128>()));
        cf_opts.set_memtable_prefix_bloom_ratio(0.2);
        ColumnFamilyDescriptor::new(Self::name(), cf_opts)
    }

    fn name() -> &'static str {
        "log_timestamp_index"
    }
}

impl KeyValueSchemaExt for LogTimestampSchema {
    fn short_id() -> u16 {
        0x0012
    }
}

/// Allowed filters for log message store
#[derive(Debug, Default, Clone)]
pub struct Filters {
    pub node_name: Option<NodeName>,
    pub log_level: Vec<LogLevel>,
    pub date: Option<u128>,
}

pub struct Indices<KvStorage>
where
    KvStorage: AsRef<DB>
        + KeyValueStoreWithSchema<NodeNameSchema>
        + KeyValueStoreWithSchema<LogLevelSchema>
        + KeyValueStoreWithSchema<LogTimestampSchema>,
{
    node_name_index: SecondaryIndex<KvStorage, Schema, NodeNameSchema, NodeName>,
    log_level_index: SecondaryIndex<KvStorage, Schema, LogLevelSchema, LogLevel>,
    timestamp_index: SecondaryIndex<KvStorage, Schema, LogTimestampSchema, u128>,
}

impl<KvStorage> Clone for Indices<KvStorage>
where
    KvStorage: AsRef<DB>
        + KeyValueStoreWithSchema<NodeNameSchema>
        + KeyValueStoreWithSchema<LogLevelSchema>
        + KeyValueStoreWithSchema<LogTimestampSchema>,
{
    fn clone(&self) -> Self {
        Indices {
            node_name_index: self.node_name_index.clone(),
            log_level_index: self.log_level_index.clone(),
            timestamp_index: self.timestamp_index.clone(),
        }
    }
}

impl<KvStorage> SecondaryIndices for Indices<KvStorage>
where
    KvStorage: AsRef<DB>
        + KeyValueStoreWithSchema<NodeNameSchema>
        + KeyValueStoreWithSchema<LogLevelSchema>
        + KeyValueStoreWithSchema<LogTimestampSchema>,
{
    type KvStorage = KvStorage;
    type PrimarySchema = Schema;
    type Filter = Filters;

    fn new(kv: &Arc<Self::KvStorage>) -> Self {
        Indices {
            node_name_index: SecondaryIndex::new(kv),
            log_level_index: SecondaryIndex::new(kv),
            timestamp_index: SecondaryIndex::new(kv),
        }
    }

    fn schemas(cache: &Cache) -> Vec<ColumnFamilyDescriptor> {
        vec![
            NodeNameSchema::descriptor(cache),
            LogLevelSchema::descriptor(cache),
            LogTimestampSchema::descriptor(cache),
        ]
    }

    fn schemas_ext() -> Vec<ColumnFamilyDescriptorExt> {
        vec![
            NodeNameSchema::descriptor_ext(),
            LogLevelSchema::descriptor_ext(),
            LogTimestampSchema::descriptor_ext(),
        ]
    }

    fn store_indices(
        &self,
        primary_key: &<Self::PrimarySchema as KeyValueSchema>::Key,
        value: &<Self::PrimarySchema as KeyValueSchema>::Value,
    ) -> Result<(), StorageError> {
        self.node_name_index.store_index(primary_key, value)?;
        self.log_level_index.store_index(primary_key, value)?;
        self.timestamp_index.store_index(primary_key, value)?;
        Ok(())
    }

    fn delete_indices(
        &self,
        primary_key: &<Self::PrimarySchema as KeyValueSchema>::Key,
        value: &<Self::PrimarySchema as KeyValueSchema>::Value,
    ) -> Result<(), StorageError> {
        self.node_name_index.delete_index(primary_key, value)?;
        self.log_level_index.delete_index(primary_key, value)?;
        self.timestamp_index.delete_index(primary_key, value)?;
        Ok(())
    }

    fn filter_iterator(
        &self,
        primary_key: &<Self::PrimarySchema as KeyValueSchema>::Key,
        limit: usize,
        filter: &Self::Filter,
    ) -> Result<Option<Vec<<Self::PrimarySchema as KeyValueSchema>::Key>>, StorageError> {
        use itertools::Itertools;

        let mut iters: Vec<Box<dyn Iterator<Item = <Self::PrimarySchema as KeyValueSchema>::Key>>> = Vec::with_capacity(3);

        if let Some(node_name) = &filter.node_name {
            let it = self.node_name_index.get_concrete_prefix_iterator(primary_key, node_name)?;
            iters.push(Box::new(it));
        }
        if !filter.log_level.is_empty() {
            let mut error = None::<StorageError>;
            let level_iters = filter.log_level
                .iter()
                .filter_map(|p2p_type| {
                    match self.log_level_index.get_concrete_prefix_iterator(primary_key, p2p_type) {
                        Ok(i) => Some(i),
                        Err(err) => {
                            error = Some(err);
                            None
                        },
                    }
                });
            iters.push(Box::new(level_iters.kmerge_by(|x, y| x > y)));
            if error.is_some() {
                drop(iters);
                return Err(error.unwrap());
            }
        }
        if let Some(timestamp) = &filter.date {
            let it = self.timestamp_index.get_iterator(primary_key, timestamp)?;
            iters.push(Box::new(it));
        }

        if iters.is_empty() {
            Ok(None)
        } else {
            Ok(Some(sorted_intersect(iters.as_mut_slice(), limit)))
        }
    }
}
