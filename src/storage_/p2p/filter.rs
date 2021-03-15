use std::{net::SocketAddr, sync::Arc, mem};
use storage::{persistent::{KeyValueSchema, KeyValueStoreWithSchema}, StorageError};
use rocksdb::{DB, ColumnFamilyDescriptor, Options, SliceTransform, Cache};
use super::{
    message::Schema,
    SecondaryIndex,
    SecondaryIndices,
    indices::{RemoteAddrKey, P2pTypeKey, P2pType, SenderKey, Sender, InitiatorKey, Initiator, NodeNameKey, NodeName},
    sorted_intersect::sorted_intersect,
    KeyValueSchemaExt,
    ColumnFamilyDescriptorExt,
};

pub struct RemoteAddrSchema;

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

impl KeyValueSchemaExt for RemoteAddrSchema {
    fn short_id() -> u16 {
        0x0020
    }
}

pub struct P2pTypeSchema;

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

impl KeyValueSchemaExt for P2pTypeSchema {
    fn short_id() -> u16 {
        0x0021
    }
}

pub struct IncomingSchema;

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

impl KeyValueSchemaExt for IncomingSchema {
    fn short_id() -> u16 {
        0x0022
    }
}

pub struct SourceTypeSchema;

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

impl KeyValueSchemaExt for SourceTypeSchema {
    fn short_id() -> u16 {
        0x0023
    }
}

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
        "p2p_node_name_index"
    }
}

impl KeyValueSchemaExt for NodeNameSchema {
    fn short_id() -> u16 {
        0x0024
    }
}

/// Allowed filters for p2p message store
#[derive(Debug, Default, Clone)]
pub struct Filters {
    pub node_name: Option<NodeName>,
    pub remote_addr: Option<SocketAddr>,
    pub initiator: Option<Initiator>,
    pub sender: Option<Sender>,
    pub types: Vec<P2pType>,
}

pub struct Indices<KvStorage>
where
    KvStorage: AsRef<DB>
        + KeyValueStoreWithSchema<RemoteAddrSchema>
        + KeyValueStoreWithSchema<P2pTypeSchema>
        + KeyValueStoreWithSchema<IncomingSchema>
        + KeyValueStoreWithSchema<SourceTypeSchema>
        + KeyValueStoreWithSchema<NodeNameSchema>,
{
    remote_addr_index: SecondaryIndex<KvStorage, Schema, RemoteAddrSchema, SocketAddr>,
    type_index: SecondaryIndex<KvStorage, Schema, P2pTypeSchema, P2pType>,
    incoming_index: SecondaryIndex<KvStorage, Schema, IncomingSchema, Sender>,
    source_type_index: SecondaryIndex<KvStorage, Schema, SourceTypeSchema, Initiator>,
    node_name_index: SecondaryIndex<KvStorage, Schema, NodeNameSchema, NodeName>,
}

impl<KvStorage> Clone for Indices<KvStorage>
where
    KvStorage: AsRef<DB>
        + KeyValueStoreWithSchema<RemoteAddrSchema>
        + KeyValueStoreWithSchema<P2pTypeSchema>
        + KeyValueStoreWithSchema<IncomingSchema>
        + KeyValueStoreWithSchema<SourceTypeSchema>
        + KeyValueStoreWithSchema<NodeNameSchema>,
{
    fn clone(&self) -> Self {
        Indices {
            remote_addr_index: self.remote_addr_index.clone(),
            type_index: self.type_index.clone(),
            incoming_index: self.incoming_index.clone(),
            source_type_index: self.source_type_index.clone(),
            node_name_index: self.node_name_index.clone(),
        }
    }
}

impl<KvStorage> SecondaryIndices for Indices<KvStorage>
where
    KvStorage: AsRef<DB>
        + KeyValueStoreWithSchema<RemoteAddrSchema>
        + KeyValueStoreWithSchema<P2pTypeSchema>
        + KeyValueStoreWithSchema<IncomingSchema>
        + KeyValueStoreWithSchema<SourceTypeSchema>
        + KeyValueStoreWithSchema<NodeNameSchema>,
{
    type KvStorage = KvStorage;
    type PrimarySchema = Schema;
    type Filter = Filters;

    fn new(kv: &Arc<Self::KvStorage>) -> Self {
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

    fn schemas_ext() -> Vec<ColumnFamilyDescriptorExt> {
        vec![
            RemoteAddrSchema::descriptor_ext(),
            P2pTypeSchema::descriptor_ext(),
            IncomingSchema::descriptor_ext(),
            SourceTypeSchema::descriptor_ext(),
            NodeNameSchema::descriptor_ext(),
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
        use itertools::Itertools;

        let mut iters: Vec<Box<dyn Iterator<Item = <Self::PrimarySchema as KeyValueSchema>::Key>>> = Vec::with_capacity(5);

        if let Some(remote_addr) = &filter.remote_addr {
            let it = self.remote_addr_index.get_concrete_prefix_iterator(primary_key, remote_addr)?;
            iters.push(Box::new(it));
        }
        if !filter.types.is_empty() {
            let mut error = None::<StorageError>;
            let type_iters = filter.types
                .iter()
                .filter_map(|p2p_type| {
                    match self.type_index.get_concrete_prefix_iterator(primary_key, p2p_type) {
                        Ok(i) => Some(i),
                        Err(err) => {
                            error = Some(err);
                            None
                        },
                    }
                });
            iters.push(Box::new(type_iters.kmerge_by(|x, y| x > y)));
            if error.is_some() {
                drop(iters);
                return Err(error.unwrap());
            }
        }
        if let Some(sender) = &filter.sender {
            let it = self.incoming_index.get_concrete_prefix_iterator(primary_key, &sender)?;
            iters.push(Box::new(it));
        }
        if let Some(initiator) = &filter.initiator {
            let it = self.source_type_index.get_concrete_prefix_iterator(primary_key, initiator)?;
            iters.push(Box::new(it));
        }
        if let Some(node_name) = &filter.node_name {
            let it = self.node_name_index.get_concrete_prefix_iterator(primary_key, node_name)?;
            iters.push(Box::new(it));
        }

        if iters.is_empty() {
            Ok(None)
        } else {
            Ok(Some(sorted_intersect(iters.as_mut_slice(), limit)))
        }
    }
}
