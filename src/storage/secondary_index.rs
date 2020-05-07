use storage::persistent::{KeyValueStoreWithSchema, KeyValueSchema};
use storage::StorageError;
use storage::persistent::database::IteratorWithSchema;

pub trait SecondaryIndex<PrimaryStoreSchema>
    where
        Self: KeyValueSchema + AsRef<(dyn KeyValueStoreWithSchema<Self> + 'static)> + Sized,
        PrimaryStoreSchema: KeyValueSchema<Key=<Self as KeyValueSchema>::Value>,
{
    type FieldType;
    fn accessor(value: &PrimaryStoreSchema::Value) -> Self::FieldType;
    fn make_index(key: &PrimaryStoreSchema::Key, value: Self::FieldType) -> Self::Key;
    fn make_prefix_index(value: Self::FieldType) -> Self::Key;

    fn store_index(&self, key: &PrimaryStoreSchema::Key, value: &PrimaryStoreSchema::Value) -> Result<(), StorageError> {
        let index = Self::make_index(key, Self::accessor(value));
        let db = self.as_ref();
        Ok(db.put(&index, key)?)
    }

    fn delete_index(&self, key: &PrimaryStoreSchema::Key, value: &PrimaryStoreSchema::Value) -> Result<(), StorageError> {
        let index = Self::make_index(key, Self::accessor(value));
        let db = self.as_ref();
        Ok(db.delete(&index)?)
    }

    fn get_index(&self, key: &PrimaryStoreSchema::Key, value: &PrimaryStoreSchema::Value) -> Result<Option<PrimaryStoreSchema::Key>, StorageError> {
        let index = Self::make_index(key, Self::accessor(value));
        let db = self.as_ref();
        Ok(db.get(&index)?)
    }

    fn get_raw_prefix_iterator(&self, field: Self::FieldType) -> Result<IteratorWithSchema<Self>, StorageError> {
        let prefix = Self::make_prefix_index(field);

        Ok(self.as_ref().prefix_iterator(&prefix)?)
    }
}