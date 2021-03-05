use std::{net::SocketAddr, sync::Arc};
use storage::{persistent::KeyValueSchema, StorageError};
use rocksdb::{DB, ColumnFamilyDescriptor, Options, SliceTransform, Cache};
use super::{
    message::Schema,
    SecondaryIndex,
    SecondaryIndices,
    indices::RemoteAddrKey,
    sorted_intersect::sorted_intersect,
};

struct RemoteAddrSchema;

impl KeyValueSchema for RemoteAddrSchema {
    type Key = RemoteAddrKey;
    type Value = u64;

    fn descriptor(_cache: &Cache) -> ColumnFamilyDescriptor {
        let mut cf_opts = Options::default();
        cf_opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(16 + 2));
        cf_opts.set_memtable_prefix_bloom_ratio(0.2);
        ColumnFamilyDescriptor::new(Self::name(), cf_opts)
    }

    fn name() -> &'static str {
        "p2p_reverse_remote_index"
    }
}

/// Allowed filters for p2p message store
#[derive(Debug, Default, Clone)]
pub struct Filters {
    pub remote_addr: Option<SocketAddr>,
    pub types: Option<u32>,
    pub incoming: Option<bool>,
    pub source_type: Option<bool>,
    pub node_name: Option<String>,
}

pub struct Indices {
    remote_addr_index: SecondaryIndex<Schema, RemoteAddrSchema, SocketAddr>,
    //type_index: TypeIndex,
    //incoming_index: IncomingIndex,
    //source_type_index: SourceTypeIndex,
    //node_name_index: NodeNameIndex,
}

impl SecondaryIndices for Indices {
    type PrimarySchema = Schema;
    type Filter = Option<SocketAddr>;

    fn new(kv: Arc<DB>) -> Self {
        Indices {
            remote_addr_index: SecondaryIndex::new(kv),
        }
    }

    fn store_indices(
        &self,
        primary_key: &<Self::PrimarySchema as KeyValueSchema>::Key,
        value: &<Self::PrimarySchema as KeyValueSchema>::Value,
    ) -> Result<(), StorageError> {
        self.remote_addr_index.store_index(primary_key, value)?;
        Ok(())
    }

    fn delete_indices(
        &self,
        primary_key: &<Self::PrimarySchema as KeyValueSchema>::Key,
        value: &<Self::PrimarySchema as KeyValueSchema>::Value,
    ) -> Result<(), StorageError> {
        self.remote_addr_index.delete_index(primary_key, value)?;
        Ok(())
    }

    fn filter_iterator(
        &self,
        primary_key: &<Self::PrimarySchema as KeyValueSchema>::Key,
        limit: usize,
        filter: Self::Filter,
    ) -> Result<Option<Vec<<Self::PrimarySchema as KeyValueSchema>::Key>>, StorageError> {
        let mut iters = Vec::with_capacity(6);
        if let Some(remote_addr) = filter {
            let it = self.remote_addr_index.get_concrete_prefix_iterator(primary_key, remote_addr)?
                .filter_map(|(_, v)| v.ok());
            iters.push(Box::new(it));
        }

        if iters.is_empty() {
            Ok(None)
        } else {
            Ok(Some(sorted_intersect(iters, limit)))
        }
    }
}
