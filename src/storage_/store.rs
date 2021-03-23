use std::sync::{Arc, atomic::{Ordering, AtomicU64}};
use rocksdb::{Cache, ColumnFamilyDescriptor, DB};
use storage::{
    Direction,
    IteratorMode,
    StorageError,
    persistent::{BincodeEncoded, KeyValueStoreWithSchema, KeyValueSchema},
};
use super::{secondary_index::SecondaryIndices, remote::{KeyValueSchemaExt, ColumnFamilyDescriptorExt}};

pub trait MessageHasId {
    fn set_id(&mut self, id: u64);
}

pub trait StoreCollector<Message>
where
    Message: MessageHasId,
{
    fn store_message(&self, msg: Message) -> Result<u64, StorageError>;

    fn store_at(&self, index: u64, msg: Message) -> Result<(), StorageError>;

    /// Deletes the message and corresponding secondary indices.
    fn delete_message(&self, index: u64) -> Result<(), StorageError>;
}

type Message<Indices> = <<Indices as SecondaryIndices>::PrimarySchema as KeyValueSchema>::Value;

/// generic message store
pub struct Store<Indices>
where
    Indices: SecondaryIndices,
    Indices::KvStorage: KeyValueStoreWithSchema<Indices::PrimarySchema> + AsRef<DB>,
    Indices::PrimarySchema: KeyValueSchemaExt<Key = u64>,
    Message<Indices>: BincodeEncoded + MessageHasId,
{
    kv: Arc<Indices::KvStorage>,
    seq: Arc<AtomicU64>,
    limit: u64,
    indices: Indices,
}

impl<Indices> Clone for Store<Indices>
where
    Indices: SecondaryIndices + Clone,
    Indices::KvStorage: KeyValueStoreWithSchema<Indices::PrimarySchema> + AsRef<DB>,
    Indices::PrimarySchema: KeyValueSchemaExt<Key = u64>,
    Message<Indices>: BincodeEncoded + MessageHasId,
{
    fn clone(&self) -> Self {
        Store {
            kv: self.kv.clone(),
            seq: self.seq.clone(),
            limit: self.limit,
            indices: self.indices.clone(),
        }
    }
}

impl<Indices> Store<Indices>
where
    Indices: SecondaryIndices,
    Indices::KvStorage: KeyValueStoreWithSchema<Indices::PrimarySchema> + AsRef<DB>,
    Indices::PrimarySchema: KeyValueSchemaExt<Key = u64>,
    Message<Indices>: BincodeEncoded + MessageHasId,
{
    pub fn new(kv: &Arc<Indices::KvStorage>, indices: Indices, limit: u64) -> Self {
        Store {
            kv: kv.clone(),
            seq: Arc::new(AtomicU64::new(0)),
            limit,
            indices,
        }
    }

    pub fn schemas(cache: &Cache) -> impl Iterator<Item = ColumnFamilyDescriptor> {
        use std::iter;

        Indices::schemas(cache).into_iter()
            .chain(iter::once(<Indices as SecondaryIndices>::PrimarySchema::descriptor(cache)))
    }

    pub fn schemas_ext() -> impl Iterator<Item = ColumnFamilyDescriptorExt> {
        use std::iter;

        Indices::schemas_ext().into_iter()
            .chain(iter::once(<Indices as SecondaryIndices>::PrimarySchema::descriptor_ext()))
    }

    fn inner(&self) -> &impl KeyValueStoreWithSchema<Indices::PrimarySchema> {
        self.kv.as_ref()
    }

    // Reserve new index for later use.
    fn reserve_index(&self) -> u64 {
        self.seq.fetch_add(1, Ordering::SeqCst)
    }

    pub fn get(&self, index: u64) -> Result<Option<Message<Indices>>, StorageError> {
        self.kv.get(&index).map_err(Into::into)
    }

    pub fn get_all(&self) -> Result<Vec<Message<Indices>>, StorageError> {
        Ok(self.inner().iterator(IteratorMode::Start)?
            .filter_map(|(k, v)| {
                match (k, v) {
                    (Ok(_), Ok(v)) => Some(v),
                    (Ok(index), Err(err)) => {
                        tracing::warn!("Failed to load value at index {}: {}", index, err);
                        None
                    },
                    (Err(err), _) => {
                        tracing::warn!("Failed to load index: {}", err);
                        None
                    },
                }
            })
            .collect())
    }

    /// Create iterator ending on given index. If no value is provided
    /// start at the end
    pub fn get_cursor(
        &self,
        cursor_index: Option<u64>,
        limit: usize,
        filter: &Indices::Filter,
    ) -> Result<Vec<Message<Indices>>, StorageError> {
        let cursor_index = cursor_index.unwrap_or(u64::MAX);
        let ret = if let Some(keys) = self.indices.filter_iterator(&cursor_index, limit, &filter)? {
            keys.iter()
                .filter_map(move |index| {
                    match self.kv.get(&index) {
                        Ok(Some(value)) => {
                            Some(value)
                        },
                        Ok(None) => {
                            tracing::info!("No value at index: {}", index);
                            None
                        },
                        Err(err) => {
                            tracing::warn!("Failed to load value at index {}: {}", index, err);
                            None
                        },
                    }
                })
                .collect()
        } else {
            self.inner()
                .iterator(IteratorMode::From(&cursor_index, Direction::Reverse))?
                .filter_map(|(k, v)| {
                    match (k, v) {
                        (Ok(_), Ok(v)) => Some(v),
                        (Ok(index), Err(err)) => {
                            tracing::warn!("Failed to load value at index {}: {}", index, err);
                            None
                        },
                        (Err(err), _) => {
                            tracing::warn!("Failed to load index: {}", err);
                            None
                        },
                    }
                })
                .take(limit)
                .collect()
        };

        Ok(ret)
    }
}

impl<M, Indices> StoreCollector<M> for Store<Indices>
where
    Indices: SecondaryIndices,
    Indices::KvStorage: KeyValueStoreWithSchema<Indices::PrimarySchema> + AsRef<DB>,
    Indices::PrimarySchema: KeyValueSchemaExt<Key = u64, Value = M>,
    Message<Indices>: BincodeEncoded + MessageHasId,
{
    fn store_message(&self, msg: M) -> Result<u64, StorageError> {
        let mut msg = msg;
        let index = self.reserve_index();
        if index >= self.limit {
            self.delete_message(index - self.limit)?;
        }
        msg.set_id(index);
        self.kv.put(&index, &msg)?;
        self.indices.store_indices(&index, &msg)?;
        Ok(index)
    }

    fn store_at(&self, index: u64, msg: M) -> Result<(), StorageError> {
        let _ = self.kv.delete(&index);
        self.kv.put(&index, &msg)?;
        Ok(())
    }

    fn delete_message(&self, index: u64) -> Result<(), StorageError> {
        if let Some(value) = self.kv.get(&index)? {
            self.indices.delete_indices(&index, &value)?;
            self.kv.delete(&index)?;
        }
        Ok(())
    }
}
