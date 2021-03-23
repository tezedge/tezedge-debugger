use std::{marker::PhantomData, sync::Arc};
use rocksdb::{Cache, ColumnFamilyDescriptor, DB, DBIterator, ReadOptions, IteratorMode, Direction};
use storage::{StorageError, persistent::{DBError, KeyValueSchema, KeyValueStoreWithSchema, codec::Codec, Encoder, Decoder}};
use super::remote::ColumnFamilyDescriptorExt;

pub trait FilterField<PrimarySchema>
where
    Self: Sized,
    PrimarySchema: KeyValueSchema,
{
    type Key: Codec;

    fn make_index(&self, primary_key: &PrimarySchema::Key) -> Self::Key;
}

pub trait Access<T> {
    fn accessor(&self) -> T;
}

pub struct SecondaryIndexIterator<'a, PrimarySchema>
where
    PrimarySchema: KeyValueSchema,
{
    inner: DBIterator<'a>,
    phantom_data: PhantomData<PrimarySchema>,
}

impl<'a, PrimarySchema> Iterator for SecondaryIndexIterator<'a, PrimarySchema>
where
    PrimarySchema: KeyValueSchema,
{
    type Item = PrimarySchema::Key;

    fn next(&mut self) -> Option<Self::Item> {
        let (_k, v) = self.inner.next()?;
        // safe to unwrap because iterator is statically typed
        Some(<PrimarySchema::Key as Decoder>::decode(v.as_ref()).unwrap())
    }
}

/// generic secondary index store
pub struct SecondaryIndex<KvStorage, PrimarySchema, Schema, Field>
where
    KvStorage: KeyValueStoreWithSchema<Schema> + AsRef<DB>,
    PrimarySchema: KeyValueSchema,
    Schema: KeyValueSchema<Key = Field::Key, Value = PrimarySchema::Key>,
    Field: FilterField<PrimarySchema>,
{
    kv: Arc<KvStorage>,
    phantom_data: PhantomData<(PrimarySchema, Schema, Field)>,
}

impl<KvStorage, PrimarySchema, Schema, Field> Clone for SecondaryIndex<KvStorage, PrimarySchema, Schema, Field>
where
    KvStorage: KeyValueStoreWithSchema<Schema> + AsRef<DB>,
    PrimarySchema: KeyValueSchema,
    PrimarySchema::Value: Access<Field>,
    Schema: KeyValueSchema<Key = Field::Key, Value = PrimarySchema::Key>,
    Field: FilterField<PrimarySchema>,
{
    fn clone(&self) -> Self {
        SecondaryIndex {
            kv: self.kv.clone(),
            phantom_data: PhantomData,
        }
    }
}

impl<KvStorage, PrimarySchema, Schema, Field> SecondaryIndex<KvStorage, PrimarySchema, Schema, Field>
where
    KvStorage: KeyValueStoreWithSchema<Schema> + AsRef<DB>,
    PrimarySchema: KeyValueSchema,
    PrimarySchema::Value: Access<Field>,
    Schema: KeyValueSchema<Key = Field::Key, Value = PrimarySchema::Key>,
    Field: FilterField<PrimarySchema>,
{
    pub fn new(kv: &Arc<KvStorage>) -> Self {
        SecondaryIndex {
            kv: kv.clone(),
            phantom_data: PhantomData,
        }
    }

    /// Build new index for given value and store it.
    pub fn store_index(&self, primary_key: &PrimarySchema::Key, value: &PrimarySchema::Value) -> Result<(), StorageError> {
        let field = value.accessor();
        let key = field.make_index(primary_key);
        self.kv.put(&key, primary_key).map_err(Into::into)
    }

    /// Delete secondary index for primary key - value
    pub fn delete_index(&self, primary_key: &PrimarySchema::Key, value: &PrimarySchema::Value) -> Result<(), StorageError> {
        let field = value.accessor();
        let key = field.make_index(primary_key);
        self.kv.delete(&key).map_err(Into::into)
    }

    /// Get iterator starting from specific secondary index build from primary key and field value
    pub fn get_concrete_prefix_iterator<'a, 'b>(
        &'a self,
        primary_key: &PrimarySchema::Key,
        field: &Field,
    ) -> Result<SecondaryIndexIterator<'b, PrimarySchema>, StorageError>
    where
        'a: 'b,
    {
        let key = field.make_index(primary_key);
        let key = key.encode()?;
        let cf = self
            .kv
            .as_ref()
            .as_ref()
            .cf_handle(Schema::name())
            .ok_or(DBError::MissingColumnFamily { name: Schema::name() })?;
        let mut opts = ReadOptions::default();
        opts.set_prefix_same_as_start(true);

        Ok(SecondaryIndexIterator {
            inner: self.kv.as_ref().as_ref().iterator_cf_opt(cf, opts, IteratorMode::From(&key, Direction::Reverse)),
            phantom_data: PhantomData,
        })
    }

    pub fn get_iterator<'a, 'b>(
        &'a self,
        primary_key: &PrimarySchema::Key,
        field: &Field,
    ) -> Result<SecondaryIndexIterator<'b, PrimarySchema>, StorageError>
    where
        'a: 'b,
    {
        let key = field.make_index(primary_key);
        let key = key.encode()?;
        let cf = self
            .kv
            .as_ref()
            .as_ref()
            .cf_handle(Schema::name())
            .ok_or(DBError::MissingColumnFamily { name: Schema::name() })?;

        Ok(SecondaryIndexIterator {
            inner: self.kv.as_ref().as_ref().iterator_cf(cf, IteratorMode::From(&key, Direction::Reverse)),
            phantom_data: PhantomData,
        })
    }
}

pub trait SecondaryIndices {
    type KvStorage;
    type PrimarySchema: KeyValueSchema;
    type Filter;

    fn new(kv: &Arc<Self::KvStorage>) -> Self;

    fn schemas(cache: &Cache) -> Vec<ColumnFamilyDescriptor>;

    fn schemas_ext() -> Vec<ColumnFamilyDescriptorExt>;

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
        filter: &Self::Filter,
    ) -> Result<Option<Vec<<Self::PrimarySchema as KeyValueSchema>::Key>>, StorageError>;
}

impl<KvStorage, PrimarySchema> SecondaryIndices for PhantomData<(KvStorage, PrimarySchema)>
where
    KvStorage: AsRef<DB>,
    PrimarySchema: KeyValueSchema,
{
    type KvStorage = KvStorage;
    type PrimarySchema = PrimarySchema;
    type Filter = ();

    fn new(kv: &Arc<Self::KvStorage>) -> Self {
        let _ = kv;
        PhantomData
    }

    fn schemas(cache: &Cache) -> Vec<ColumnFamilyDescriptor> {
        let _ = cache;
        vec![]
    }

    fn schemas_ext() -> Vec<ColumnFamilyDescriptorExt> {
        vec![]
    }

    fn store_indices(
        &self,
        primary_key: &<Self::PrimarySchema as KeyValueSchema>::Key,
        value: &<Self::PrimarySchema as KeyValueSchema>::Value,
    ) -> Result<(), StorageError> {
        let _ = (primary_key, value);
        Ok(())
    }

    fn delete_indices(
        &self,
        primary_key: &<Self::PrimarySchema as KeyValueSchema>::Key,
        value: &<Self::PrimarySchema as KeyValueSchema>::Value,
    ) -> Result<(), StorageError> {
        let _ = (primary_key, value);
        Ok(())
    }

    fn filter_iterator(
        &self,
        primary_key: &<Self::PrimarySchema as KeyValueSchema>::Key,
        limit: usize,
        filter: &Self::Filter,
    ) -> Result<Option<Vec<<Self::PrimarySchema as KeyValueSchema>::Key>>, StorageError> {
        let _ = (primary_key, limit, filter);
        Ok(None)
    }
}
