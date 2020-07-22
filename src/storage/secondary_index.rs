// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use storage::persistent::{KeyValueStoreWithSchema, KeyValueSchema};
use storage::{StorageError, Direction, IteratorMode};
use storage::persistent::database::IteratorWithSchema;

/// Trait describing column family which purpose is to drive secondary index for some
/// other ColumnFamily
pub trait SecondaryIndex<PrimaryStoreSchema>
    where
        // Self itself is a KeyValueSchema
        Self: KeyValueSchema + AsRef<(dyn KeyValueStoreWithSchema<Self> + 'static)> + Sized,
        // Build on top of some primary schema, which key is value stored inside this store
        PrimaryStoreSchema: KeyValueSchema<Key=<Self as KeyValueSchema>::Value>,
{
    /// Field type, which should be extracted for indexing
    type FieldType;
    /// Extract value for indexing out of stored data
    fn accessor(value: &PrimaryStoreSchema::Value) -> Option<Self::FieldType>;
    /// Build index out of primary key and indexing value
    fn make_index(key: &PrimaryStoreSchema::Key, value: Self::FieldType) -> Self::Key;
    /// Make empty prefix key without primary key. Used for prefix iterators
    fn make_prefix_index(value: Self::FieldType) -> Self::Key;

    /// Build new index for given value and store it.
    fn store_index(&self, key: &PrimaryStoreSchema::Key, value: &PrimaryStoreSchema::Value) -> Result<(), StorageError> {
        let db = self.as_ref();
        if let Some(field) = Self::accessor(value) {
            let index = Self::make_index(key, field);
            Ok(db.put(&index, key)?)
        } else {
            Ok(())
        }
    }

    /// Delete secondary index for primary key - value
    fn delete_index(&self, key: &PrimaryStoreSchema::Key, value: &PrimaryStoreSchema::Value) -> Result<(), StorageError> {
        let db = self.as_ref();
        if let Some(field) = Self::accessor(value) {
            let index = Self::make_index(key, field);
            Ok(db.delete(&index)?)
        } else {
            Ok(())
        }
    }

    /// Load index for specific key == check if given key as specified property
    fn get_index(&self, key: &PrimaryStoreSchema::Key, field: Self::FieldType) -> Result<Option<PrimaryStoreSchema::Key>, StorageError> {
        let db = self.as_ref();
        let index = Self::make_index(key, field);
        Ok(db.get(&index)?)
    }

    /// Create iterator of values with specified property, starting on specified key
    fn get_iterator(&self, key: &PrimaryStoreSchema::Key, field: Self::FieldType, direction: Direction) -> Result<IteratorWithSchema<Self>, StorageError> {
        let index = Self::make_index(key, field);
        Ok(self.kv().iterator(IteratorMode::From(&index, direction))?)
    }

    /// Get all values with specific field value
    fn get_prefix_iterator(&self, field: Self::FieldType) -> Result<IteratorWithSchema<Self>, StorageError> {
        let prefix = Self::make_prefix_index(field);
        Ok(self.as_ref().prefix_iterator(&prefix)?)
    }

    /// Get iterator starting from specific secondary index build from primary key and field value
    fn get_concrete_prefix_iterator(&self, key: &PrimaryStoreSchema::Key, field: Self::FieldType) -> Result<IteratorWithSchema<Self>, StorageError> {
        let index = Self::make_index(key, field);
        Ok(self.as_ref().prefix_iterator(&index)?)
    }

    /// Get underlying key-value store
    fn kv(&self) -> &(dyn KeyValueStoreWithSchema<Self>) {
        self.as_ref()
    }
}