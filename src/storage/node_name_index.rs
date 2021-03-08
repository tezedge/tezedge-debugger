use storage::persistent::{KeyValueStoreWithSchema, KeyValueSchema, Decoder, SchemaError, Encoder};
use std::sync::Arc;
use rocksdb::{DB, ColumnFamilyDescriptor, Options, SliceTransform, Cache};
use crate::storage::secondary_index::SecondaryIndex;
use serde::{Serialize, Deserialize};

pub trait HasNodeName {
    fn node_name(&self) -> u16;
}

type NodeNameIndexKV = dyn KeyValueStoreWithSchema<NodeNameIndex> + Sync + Send;

#[derive(Clone)]
pub struct NodeNameIndex {
    kv: Arc<NodeNameIndexKV>,
}

impl NodeNameIndex {
    pub fn new(kv: Arc<DB>) -> Self {
        Self { kv }
    }
}

impl AsRef<(dyn KeyValueStoreWithSchema<NodeNameIndex> + 'static)> for NodeNameIndex {
    fn as_ref(&self) -> &(dyn KeyValueStoreWithSchema<NodeNameIndex> + 'static) {
        self.kv.as_ref()
    }
}

impl KeyValueSchema for NodeNameIndex {
    type Key = NodeNameKey;
    type Value = u64;

    fn descriptor(_cache: &Cache) -> ColumnFamilyDescriptor {
        let mut cf_opts = Options::default();
        cf_opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(8));
        cf_opts.set_memtable_prefix_bloom_ratio(0.2);
        ColumnFamilyDescriptor::new(Self::name(), cf_opts)
    }

    fn name() -> &'static str {
        "node_name_index"
    }
}

impl<PrimaryStoreSchema> SecondaryIndex<PrimaryStoreSchema> for NodeNameIndex
where
    PrimaryStoreSchema: KeyValueSchema<Key=<Self as KeyValueSchema>::Value>,
    PrimaryStoreSchema::Value: HasNodeName,
{
    type FieldType = u16;

    fn accessor(value: &PrimaryStoreSchema::Value) -> Option<Self::FieldType> {
        Some(value.node_name())
    }

    fn make_index(key: &PrimaryStoreSchema::Key, value: Self::FieldType) -> NodeNameKey {
        NodeNameKey::new(value, key.clone())
    }

    fn make_prefix_index(value: Self::FieldType) -> NodeNameKey {
        NodeNameKey::prefix(value)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeNameKey {
    node_name: u16,
    index: u64,
}

impl NodeNameKey {
    fn new(node_name: u16, index: u64) -> Self {
        Self { node_name, index: std::u64::MAX.saturating_sub(index) }
    }

    fn prefix(node_name: u16) -> Self {
        Self { node_name, index: 0 }
    }
}

/// * bytes layout: `[node_name(2)][padding(6)][index(8)]`
impl Decoder for NodeNameKey {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        if bytes.len() != 16 {
            return Err(SchemaError::DecodeError);
        }

        let mut node_name_bytes = [0; 2];
        node_name_bytes.clone_from_slice(&bytes[0..2]);
        let mut index_bytes = [0; 8];
        index_bytes.clone_from_slice(&bytes[8..16]);
        Ok(NodeNameKey {
            node_name: u16::from_be_bytes(node_name_bytes),
            index: u64::from_be_bytes(index_bytes),
        })
    }
}

impl Encoder for NodeNameKey {
    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        let mut bytes = vec![0; 16];
        bytes[0..2].clone_from_slice(&self.node_name.to_be_bytes());
        bytes[8..16].clone_from_slice(&self.index.to_be_bytes());
        Ok(bytes)
    }
}
