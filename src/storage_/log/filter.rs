use std::{sync::Arc, mem};
use storage::{persistent::KeyValueSchema, StorageError};
use rocksdb::{DB, ColumnFamilyDescriptor, Options, SliceTransform, Cache};
use super::{
    message::Schema,
    SecondaryIndex,
    SecondaryIndices,
    indices::{NodeNameKey, NodeName},
    sorted_intersect::sorted_intersect,
};

struct NodeNameSchema;

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

/// Allowed filters for log message store
#[derive(Debug, Default, Clone)]
pub struct Filters {
    pub node_name: Option<NodeName>,
}

#[derive(Clone)]
pub struct Indices {
    node_name_index: SecondaryIndex<Schema, NodeNameSchema, NodeName>,
}

impl SecondaryIndices for Indices {
    type PrimarySchema = Schema;
    type Filter = Filters;

    fn new(kv: &Arc<DB>) -> Self {
        Indices {
            node_name_index: SecondaryIndex::new(kv),
        }
    }

    fn schemas(cache: &Cache) -> Vec<ColumnFamilyDescriptor> {
        vec![
            NodeNameSchema::descriptor(cache),
        ]
    }

    fn store_indices(
        &self,
        primary_key: &<Self::PrimarySchema as KeyValueSchema>::Key,
        value: &<Self::PrimarySchema as KeyValueSchema>::Value,
    ) -> Result<(), StorageError> {
        self.node_name_index.store_index(primary_key, value)?;
        Ok(())
    }

    fn delete_indices(
        &self,
        primary_key: &<Self::PrimarySchema as KeyValueSchema>::Key,
        value: &<Self::PrimarySchema as KeyValueSchema>::Value,
    ) -> Result<(), StorageError> {
        self.node_name_index.delete_index(primary_key, value)?;
        Ok(())
    }

    fn filter_iterator(
        &self,
        primary_key: &<Self::PrimarySchema as KeyValueSchema>::Key,
        limit: usize,
        filter: &Self::Filter,
    ) -> Result<Option<Vec<<Self::PrimarySchema as KeyValueSchema>::Key>>, StorageError> {
        let mut iters = Vec::with_capacity(1);

        if let Some(node_name) = &filter.node_name {
            let it = self.node_name_index.get_concrete_prefix_iterator(primary_key, node_name)?;
            iters.push(it);
        }

        if iters.is_empty() {
            Ok(None)
        } else {
            Ok(Some(sorted_intersect(iters.as_mut_slice(), limit)))
        }
    }
}
