use std::{net::SocketAddr, sync::Arc, mem};
use storage::{persistent::KeyValueSchema, StorageError};
use rocksdb::{DB, ColumnFamilyDescriptor, Options, SliceTransform, Cache};
use super::{
    message::Schema,
    SecondaryIndex,
    SecondaryIndices,
    indices::{RemoteAddrKey, P2pTypeKey, P2pType, SenderKey, Sender, InitiatorKey, Initiator, NodeNameKey, NodeName},
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

struct P2pTypeSchema;

impl KeyValueSchema for P2pTypeSchema {
    type Key = P2pTypeKey;
    type Value = u64;

    fn descriptor(_cache: &Cache) -> ColumnFamilyDescriptor {
        let mut cf_opts = Options::default();
        cf_opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(mem::size_of::<u32>()));
        cf_opts.set_memtable_prefix_bloom_ratio(0.2);
        ColumnFamilyDescriptor::new(Self::name(), cf_opts)
    }

    fn name() -> &'static str {
        "p2p_type_index"
    }
}

struct IncomingSchema;

impl KeyValueSchema for IncomingSchema {
    type Key = SenderKey;
    type Value = u64;

    fn descriptor(_cache: &Cache) -> ColumnFamilyDescriptor {
        let mut cf_opts = Options::default();
        cf_opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(mem::size_of::<bool>()));
        cf_opts.set_memtable_prefix_bloom_ratio(0.2);
        ColumnFamilyDescriptor::new(Self::name(), cf_opts)
    }

    fn name() -> &'static str {
        "p2p_incoming_index"
    }
}

struct SourceTypeSchema;

impl KeyValueSchema for SourceTypeSchema {
    type Key = InitiatorKey;
    type Value = u64;

    fn descriptor(_cache: &Cache) -> ColumnFamilyDescriptor {
        let mut cf_opts = Options::default();
        cf_opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(mem::size_of::<bool>()));
        cf_opts.set_memtable_prefix_bloom_ratio(0.2);
        ColumnFamilyDescriptor::new(Self::name(), cf_opts)
    }

    fn name() -> &'static str {
        "p2p_source_type_index"
    }
}

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
        "p2p_node_name_index"
    }
}

/// Allowed filters for p2p message store
#[derive(Debug, Default, Clone)]
pub struct Filters {
    pub remote_addr: Option<SocketAddr>,
    pub types: Vec<P2pType>,
    pub sender: Option<Sender>,
    pub initiator: Option<Initiator>,
    pub node_name: Option<NodeName>,
}

#[derive(Clone)]
pub struct Indices {
    remote_addr_index: SecondaryIndex<Schema, RemoteAddrSchema, SocketAddr>,
    type_index: SecondaryIndex<Schema, P2pTypeSchema, P2pType>,
    incoming_index: SecondaryIndex<Schema, IncomingSchema, Sender>,
    source_type_index: SecondaryIndex<Schema, SourceTypeSchema, Initiator>,
    node_name_index: SecondaryIndex<Schema, NodeNameSchema, NodeName>,
}

impl SecondaryIndices for Indices {
    type PrimarySchema = Schema;
    type Filter = Filters;

    fn new(kv: &Arc<DB>) -> Self {
        Indices {
            remote_addr_index: SecondaryIndex::new(kv),
            type_index: SecondaryIndex::new(kv),
            incoming_index: SecondaryIndex::new(kv),
            source_type_index: SecondaryIndex::new(kv),
            node_name_index: SecondaryIndex::new(kv),
        }
    }

    fn schemas(cache: &Cache) -> Vec<ColumnFamilyDescriptor> {
        vec![
            RemoteAddrSchema::descriptor(cache),
            P2pTypeSchema::descriptor(cache),
            IncomingSchema::descriptor(cache),
            SourceTypeSchema::descriptor(cache),
            NodeNameSchema::descriptor(cache),
        ]
    }

    fn store_indices(
        &self,
        primary_key: &<Self::PrimarySchema as KeyValueSchema>::Key,
        value: &<Self::PrimarySchema as KeyValueSchema>::Value,
    ) -> Result<(), StorageError> {
        self.remote_addr_index.store_index(primary_key, value)?;
        self.type_index.store_index(primary_key, value)?;
        self.incoming_index.store_index(primary_key, value)?;
        self.source_type_index.store_index(primary_key, value)?;
        self.node_name_index.store_index(primary_key, value)?;
        Ok(())
    }

    fn delete_indices(
        &self,
        primary_key: &<Self::PrimarySchema as KeyValueSchema>::Key,
        value: &<Self::PrimarySchema as KeyValueSchema>::Value,
    ) -> Result<(), StorageError> {
        self.remote_addr_index.delete_index(primary_key, value)?;
        self.type_index.delete_index(primary_key, value)?;
        self.incoming_index.delete_index(primary_key, value)?;
        self.source_type_index.delete_index(primary_key, value)?;
        self.node_name_index.delete_index(primary_key, value)?;
        Ok(())
    }

    fn filter_iterator(
        &self,
        primary_key: &<Self::PrimarySchema as KeyValueSchema>::Key,
        limit: usize,
        filter: &Self::Filter,
    ) -> Result<Option<Vec<<Self::PrimarySchema as KeyValueSchema>::Key>>, StorageError> {
        let mut iters = Vec::with_capacity(filter.types.len() + 4);

        if let Some(remote_addr) = &filter.remote_addr {
            let it = self.remote_addr_index.get_concrete_prefix_iterator(primary_key, remote_addr)?;
            iters.push(it);
        }
        // TODO: fix using itertools
        //for p2p_type in &filter.types {
        //    let it = self.type_index.get_concrete_prefix_iterator(primary_key, p2p_type)?;
        //    iters.push(it);
        //}
        if let Some(sender) = &filter.sender {
            let it = self.incoming_index.get_concrete_prefix_iterator(primary_key, &sender)?;
            iters.push(it);
        }
        if let Some(initiator) = &filter.initiator {
            let it = self.source_type_index.get_concrete_prefix_iterator(primary_key, initiator)?;
            iters.push(it);
        }
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
