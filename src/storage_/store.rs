use std::{
    sync::{Arc, atomic::{Ordering, AtomicU64}},
    marker::PhantomData,
};
use rocksdb::{Cache, ColumnFamilyDescriptor, DB};
use storage::{
    Direction,
    IteratorMode,
    StorageError,
    persistent::{BincodeEncoded, KeyValueSchema, KeyValueStoreWithSchema},
};
use super::secondary_index::SecondaryIndices;

/// generic message store
pub struct Store<Message, Schema, Indices>
where
    Message: BincodeEncoded,
    Schema: KeyValueSchema<Key = u64, Value = Message>,
    Indices: SecondaryIndices<PrimarySchema = Schema>,
{
    kv: Arc<DB>,
    count: Arc<AtomicU64>,
    seq: Arc<AtomicU64>,
    indices: Indices,
    phantom_data: PhantomData<(Message, Schema)>,
}

impl<Message, Schema, Indices> Clone for Store<Message, Schema, Indices>
where
    Message: BincodeEncoded,
    Schema: KeyValueSchema<Key = u64, Value = Message>,
    Indices: SecondaryIndices<PrimarySchema = Schema> + Clone,
{
    fn clone(&self) -> Self {
        Store {
            kv: self.kv.clone(),
            count: self.count.clone(),
            seq: self.seq.clone(),
            indices: self.indices.clone(),
            phantom_data: PhantomData,
        }
    }
}

impl<Message, Schema, Indices> Store<Message, Schema, Indices>
where
    Message: BincodeEncoded,
    Schema: KeyValueSchema<Key = u64, Value = Message>,
    Indices: SecondaryIndices<PrimarySchema = Schema>,
{
    pub fn new(kv: &Arc<DB>) -> Self {
        Store {
            kv: kv.clone(),
            count: Arc::new(AtomicU64::new(0)),
            seq: Arc::new(AtomicU64::new(0)),
            indices: Indices::new(kv),
            phantom_data: PhantomData,
        }
    }

    pub fn schemas(cache: &Cache) -> impl Iterator<Item = ColumnFamilyDescriptor> {
        use std::iter;

        Indices::schemas(cache).into_iter().chain(iter::once(Schema::descriptor(cache)))
    }

    fn inner(&self) -> &impl KeyValueStoreWithSchema<Schema> {
        self.kv.as_ref()
    }

    // Reserve new index for later use.
    // The index must be manually inserted with [Store::put_message]
    fn reserve_index(&self) -> u64 {
        self.seq.fetch_add(1, Ordering::SeqCst)
    }

    // Increment count of messages in the store
    fn inc_count(&self) {
        self.count.fetch_add(1, Ordering::SeqCst);
    }

    /// Create cursor into the database, allowing iteration over values matching given filters.
    /// Values are sorted by the index in descending order.
    /// * Arguments:
    /// - cursor_index: Index of start of the sequence (if no value provided, start at the end)
    /// - limit: Limit result to maximum of specified value
    /// - filters: Specified filters for values
    pub fn store_message(&self, msg: &Message) -> Result<u64, StorageError> {
        let index = self.reserve_index();
        self.inner().put(&index, msg)?;
        self.indices.store_indices(&index, msg)?;
        self.inc_count();
        Ok(index)
    }

    /// Deletes the message and corresponding secondary indices.
    pub fn delete_message(&self, index: u64) -> Result<(), StorageError> {
        if let Some(value) = self.inner().get(&index)? {
            self.indices.delete_indices(&index, &value)?;
            self.inner().delete(&index)?;
        }
        Ok(())
    }

    /// Create iterator ending on given index. If no value is provided
    /// start at the end
    pub fn get_cursor(&self, cursor_index: Option<u64>, limit: usize, filter: &Indices::Filter) -> Result<Vec<Message>, StorageError> {
        let cursor_index = cursor_index.unwrap_or(u64::MAX);
        let ret = if let Some(keys) = self.indices.filter_iterator(&cursor_index, limit, &filter)? {
            keys.iter()
                .filter_map(move |index| {
                    match self.inner().get(&index) {
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
