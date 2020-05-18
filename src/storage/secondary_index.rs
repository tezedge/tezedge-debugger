use storage::persistent::{KeyValueStoreWithSchema, KeyValueSchema};
use storage::{StorageError, Direction, IteratorMode};
use storage::persistent::database::IteratorWithSchema;

pub trait SecondaryIndex<PrimaryStoreSchema>
    where
        Self: KeyValueSchema + AsRef<(dyn KeyValueStoreWithSchema<Self> + 'static)> + Sized,
        PrimaryStoreSchema: KeyValueSchema<Key=<Self as KeyValueSchema>::Value>,
{
    type FieldType;
    fn accessor(value: &PrimaryStoreSchema::Value) -> Option<Self::FieldType>;
    fn make_index(key: &PrimaryStoreSchema::Key, value: Self::FieldType) -> Self::Key;
    fn make_prefix_index(value: Self::FieldType) -> Self::Key;

    fn store_index(&self, key: &PrimaryStoreSchema::Key, value: &PrimaryStoreSchema::Value) -> Result<(), StorageError> {
        let db = self.as_ref();
        if let Some(field) = Self::accessor(value) {
            let index = Self::make_index(key, field);
            Ok(db.put(&index, key)?)
        } else {
            Ok(())
        }
    }

    fn delete_index(&self, key: &PrimaryStoreSchema::Key, value: &PrimaryStoreSchema::Value) -> Result<(), StorageError> {
        let db = self.as_ref();
        if let Some(field) = Self::accessor(value) {
            let index = Self::make_index(key, field);
            Ok(db.delete(&index)?)
        } else {
            Ok(())
        }
    }

    fn get_index(&self, key: &PrimaryStoreSchema::Key, field: Self::FieldType) -> Result<Option<PrimaryStoreSchema::Key>, StorageError> {
        let db = self.as_ref();
        let index = Self::make_index(key, field);
        Ok(db.get(&index)?)
    }

    fn get_iterator(&self, key: &PrimaryStoreSchema::Key, field: Self::FieldType, direction: Direction) -> Result<IteratorWithSchema<Self>, StorageError> {
        let index = Self::make_index(key, field);
        Ok(self.kv().iterator(IteratorMode::From(&index, direction))?)
    }

    fn get_prefix_iterator(&self, field: Self::FieldType) -> Result<IteratorWithSchema<Self>, StorageError> {
        let prefix = Self::make_prefix_index(field);
        Ok(self.as_ref().prefix_iterator(&prefix)?)
    }

    fn get_concrete_prefix_iterator(&self, key: &PrimaryStoreSchema::Key, field: Self::FieldType) -> Result<IteratorWithSchema<Self>, StorageError> {
        let index = Self::make_index(key, field);
        Ok(self.as_ref().prefix_iterator(&index)?)
    }

    fn kv(&self) -> &(dyn KeyValueStoreWithSchema<Self>) {
        self.as_ref()
    }
}