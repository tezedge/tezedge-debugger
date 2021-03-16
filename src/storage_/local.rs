use rocksdb::{DB, WriteBatch};
use storage::{
    persistent::{KeyValueSchema, KeyValueStoreWithSchema, DBError, database::IteratorWithSchema},
    IteratorMode,
};

pub struct LocalDb(DB);

impl LocalDb {
    pub fn new(inner: DB) -> Self {
        LocalDb(inner)
    }

    fn inner<'a, S>(&'a self) -> &'a impl KeyValueStoreWithSchema<S>
    where
        S: KeyValueSchema + 'a,
    {
        &self.0
    }
}

impl AsRef<DB> for LocalDb {
    fn as_ref(&self) -> &DB {
        &self.0
    }
}

impl<S> KeyValueStoreWithSchema<S> for LocalDb
where
    S: KeyValueSchema + 'static,
{
    fn put(&self, key: &S::Key, value: &S::Value) -> Result<(), DBError> {
        self.inner::<S>().put(key, value)
    }

    fn delete(&self, key: &S::Key) -> Result<(), DBError> {
        self.inner::<S>().delete(key)
    }

    fn merge(&self, key: &S::Key, value: &S::Value) -> Result<(), DBError> {
        self.inner::<S>().merge(key, value)
    }

    fn get(&self, key: &S::Key) -> Result<Option<S::Value>, DBError> {
        self.inner::<S>().get(key)
    }

    fn iterator(&self, mode: IteratorMode<S>) -> Result<IteratorWithSchema<S>, DBError> {
        self.inner::<S>().iterator(mode)
    }

    fn prefix_iterator(&self, key: &S::Key) -> Result<IteratorWithSchema<S>, DBError> {
        self.inner::<S>().prefix_iterator(key)
    }

    fn contains(&self, key: &S::Key) -> Result<bool, DBError> {
        self.inner::<S>().contains(key)
    }

    fn put_batch(
        &self,
        batch: &mut WriteBatch,
        key: &S::Key,
        value: &S::Value,
    ) -> Result<(), DBError> {
        self.inner::<S>().put_batch(batch, key, value)
    }

    fn write_batch(&self, batch: WriteBatch) -> Result<(), DBError> {
        self.inner::<S>().write_batch(batch)
    }
}
