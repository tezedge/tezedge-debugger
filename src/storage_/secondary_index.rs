use std::{
    sync::Arc,
    marker::PhantomData,
};
use rocksdb::DB;
use storage::{
    StorageError,
    persistent::{
        codec::Codec,
        KeyValueSchema,
        KeyValueStoreWithSchema,
        database::IteratorWithSchema,
    },
};

pub trait FilterField<PrimarySchema>
where
    Self: Sized,
    PrimarySchema: KeyValueSchema,
{
    type Key: Codec;

    fn accessor(value: &PrimarySchema::Value) -> Option<Self>;
    fn make_index(&self, primary_key: &PrimarySchema::Key) -> Self::Key;
}

/// generic secondary index store
#[derive(Clone)]
pub struct SecondaryIndex<PrimarySchema, Schema, Field>
where
    PrimarySchema: KeyValueSchema,
    Schema: KeyValueSchema<Key = Field::Key, Value = PrimarySchema::Key>,
    Field: FilterField<PrimarySchema>,
{
    kv: Arc<DB>,
    phantom_data: PhantomData<(PrimarySchema, Schema, Field)>,
}

impl<PrimarySchema, Schema, Field> SecondaryIndex<PrimarySchema, Schema, Field>
where
    PrimarySchema: KeyValueSchema,
    Schema: KeyValueSchema<Key = Field::Key, Value = PrimarySchema::Key>,
    Field: FilterField<PrimarySchema>,
{
    pub fn new(kv: &Arc<DB>) -> Self {
        SecondaryIndex {
            kv: kv.clone(),
            phantom_data: PhantomData,
        }
    }

    fn inner(&self) -> &impl KeyValueStoreWithSchema<Schema> {
        self.kv.as_ref()
    }

    /// Build new index for given value and store it.
    pub fn store_index(&self, primary_key: &PrimarySchema::Key, value: &PrimarySchema::Value) -> Result<(), StorageError> {
        if let Some(field) = Field::accessor(value) {
            let key = field.make_index(primary_key);
            self.inner().put(&key, primary_key).map_err(Into::into)
        } else {
            Ok(())
        }
    }

    /// Delete secondary index for primary key - value
    pub fn delete_index(&self, primary_key: &PrimarySchema::Key, value: &PrimarySchema::Value) -> Result<(), StorageError> {
        if let Some(field) = Field::accessor(value) {
            let key = field.make_index(primary_key);
            self.inner().delete(&key).map_err(Into::into)
        } else {
            Ok(())
        }
    }

    /// Get iterator starting from specific secondary index build from primary key and field value
    pub fn get_concrete_prefix_iterator(&self, primary_key: &PrimarySchema::Key, field: Field) -> Result<IteratorWithSchema<Schema>, StorageError> {
        let index = field.make_index(primary_key);
        self.inner().prefix_iterator(&index).map_err(Into::into)
    }
}

pub trait SecondaryIndices {
    type PrimarySchema: KeyValueSchema;
    type Filter;

    fn new(kv: &Arc<DB>) -> Self;

    fn store_indices(
        &self,
        primary_key: &<Self::PrimarySchema as KeyValueSchema>::Key,
        value: &<Self::PrimarySchema as KeyValueSchema>::Value,
    ) -> Result<(), StorageError>;

    fn delete_indices(
        &self,
        primary_key: &<Self::PrimarySchema as KeyValueSchema>::Key,
        value: &<Self::PrimarySchema as KeyValueSchema>::Value,
    ) -> Result<(), StorageError>;

    fn filter_iterator(
        &self,
        primary_key: &<Self::PrimarySchema as KeyValueSchema>::Key,
        limit: usize,
        filter: Self::Filter,
    ) -> Result<Option<Vec<<Self::PrimarySchema as KeyValueSchema>::Key>>, StorageError>;
}
