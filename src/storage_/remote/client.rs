use std::{io::{self, Read, Write}, os::unix::net::UnixStream, path::Path, sync::Mutex};
use storage::{
    persistent::{KeyValueSchema, KeyValueStoreWithSchema, DBError, database::IteratorWithSchema, Encoder, Decoder},
    IteratorMode,
};
use rocksdb::{DB, WriteBatch};
use super::common::{DbRemoteOperation, KEY_SIZE_LIMIT, VALUE_SIZE_LIMIT};

pub trait KeyValueSchemaExt
where
    Self: KeyValueSchema,
{
    fn short_id() -> u16;

    fn descriptor_ext() -> ColumnFamilyDescriptorExt {
        ColumnFamilyDescriptorExt {
            short_id: Self::short_id(),
            name: Self::name(),
        }
    }
}

pub struct ColumnFamilyDescriptorExt {
    pub short_id: u16,
    pub name: &'static str,
}

pub struct DbClient {
    stream: Mutex<UnixStream>,
}

impl DbClient {
    pub fn connect<P>(path: P) -> io::Result<Self>
    where
        P: AsRef<Path>,
    {
        let stream = Mutex::new(UnixStream::connect(path)?);
        Ok(DbClient { stream })
    }
}

impl AsRef<DB> for DbClient {
    fn as_ref(&self) -> &DB {
        unimplemented!()
    }
}

impl<S> KeyValueStoreWithSchema<S> for DbClient
where
    S: KeyValueSchemaExt,
{
    fn put(&self, key: &S::Key, value: &S::Value) -> Result<(), DBError> {
        // 2 (column_index) + 2 (op) + 4 (key_size) + 4 (value_size) = 12
        let mut header = [0; 12];

        let column_index = S::short_id();
        header[0..2].clone_from_slice(column_index.to_ne_bytes().as_ref());

        let op = DbRemoteOperation::Put;
        header[2..4].clone_from_slice((op as u16).to_ne_bytes().as_ref());

        let key = key.encode()?;
        let key_size = key.len();
        if key_size > KEY_SIZE_LIMIT {
            let name = format!("key too big {}, limit: {}", key_size, KEY_SIZE_LIMIT);
            Err(DBError::DatabaseIncompatibility { name })?;
        }
        header[4..8].clone_from_slice((key_size as u32).to_ne_bytes().as_ref());

        let value = value.encode()?;
        let value_size = value.len();
        if value_size > VALUE_SIZE_LIMIT {
            let name = format!("value too big {}, limit: {}", value_size, VALUE_SIZE_LIMIT);
            Err(DBError::DatabaseIncompatibility { name })?;
        }
        header[8..12].clone_from_slice((value_size as u32).to_ne_bytes().as_ref());

        let mut stream = self.stream.lock().unwrap();
        let mut to_write = Vec::with_capacity(12 + key_size + value_size);
        to_write.extend_from_slice(&header);
        to_write.extend_from_slice(&key);
        to_write.extend_from_slice(&value);
        stream.write_all(&to_write)
            .map_err(|e| DBError::DatabaseIncompatibility { name: e.to_string() })
    }

    fn delete(&self, key: &S::Key) -> Result<(), DBError> {
        // 2 (column_index) + 2 (op) + 4 (key_size) = 8
        let mut header = [0; 8];

        let column_index = S::short_id();
        header[0..2].clone_from_slice(column_index.to_ne_bytes().as_ref());

        let op = DbRemoteOperation::Delete;
        header[2..4].clone_from_slice((op as u16).to_ne_bytes().as_ref());

        let key = key.encode()?;
        let key_size = key.len();
        if key_size > KEY_SIZE_LIMIT {
            let name = format!("key too big {}, limit: {}", key_size, KEY_SIZE_LIMIT);
            Err(DBError::DatabaseIncompatibility { name })?;
        }
        header[4..8].clone_from_slice((key_size as u32).to_ne_bytes().as_ref());

        let mut stream = self.stream.lock().unwrap();
        let mut to_write = Vec::with_capacity(8 + key_size);
        to_write.extend_from_slice(&header);
        to_write.extend_from_slice(&key);
        stream.write_all(&to_write)
            .map_err(|e| DBError::DatabaseIncompatibility { name: e.to_string() })
    }

    fn merge(&self, key: &S::Key, value: &S::Value) -> Result<(), DBError> {
        let _ = (key, value);
        unimplemented!()
    }

    fn get(&self, key: &S::Key) -> Result<Option<S::Value>, DBError> {
        // 2 (column_index) + 2 (op) + 4 (key_size) = 8
        let mut header = [0; 8];

        let column_index = S::short_id();
        header[0..2].clone_from_slice(column_index.to_ne_bytes().as_ref());

        let op = DbRemoteOperation::Get;
        header[2..4].clone_from_slice((op as u16).to_ne_bytes().as_ref());

        let key = key.encode()?;
        let key_size = key.len();
        if key_size > KEY_SIZE_LIMIT {
            let name = format!("key too big {}, limit: {}", key_size, KEY_SIZE_LIMIT);
            Err(DBError::DatabaseIncompatibility { name })?;
        }
        header[4..8].clone_from_slice((key_size as u32).to_ne_bytes().as_ref());

        let mut stream = self.stream.lock().unwrap();
        let mut to_write = Vec::with_capacity(8 + key_size);
        to_write.extend_from_slice(&header);
        to_write.extend_from_slice(&key);
        stream.write_all(&to_write)
            .map_err(|e| DBError::DatabaseIncompatibility { name: e.to_string() })?;

        let mut read = [0; 4];
        stream.read_exact(read.as_mut())
            .map_err(|e| DBError::DatabaseIncompatibility { name: e.to_string() })?;

        let value_size = u32::from_ne_bytes(read) as usize;
        if value_size == 0 {
            Ok(None)
        } else if value_size <= VALUE_SIZE_LIMIT {
            let mut value_bytes = vec![0; value_size];
            stream.read_exact(value_bytes.as_mut())
                .map_err(|e| DBError::DatabaseIncompatibility { name: e.to_string() })?;
            <S::Value as Decoder>::decode(&value_bytes).map(Some)
                .map_err(|error| DBError::SchemaError { error })
        } else {
            let name = format!("value too big {}, limit: {}", value_size, VALUE_SIZE_LIMIT);
            Err(DBError::DatabaseIncompatibility { name })
        }
    }

    fn iterator(&self, mode: IteratorMode<S>) -> Result<IteratorWithSchema<S>, DBError> {
        let _ = mode;
        unimplemented!()
    }

    fn prefix_iterator(&self, key: &S::Key) -> Result<IteratorWithSchema<S>, DBError> {
        let _ = key;
        unimplemented!()
    }

    fn contains(&self, key: &S::Key) -> Result<bool, DBError> {
        let _ = key;
        unimplemented!()
    }

    fn put_batch(
        &self,
        batch: &mut WriteBatch,
        key: &S::Key,
        value: &S::Value,
    ) -> Result<(), DBError> {
        let _ = (batch, key, value);
        unimplemented!()
    }

    fn write_batch(&self, batch: WriteBatch) -> Result<(), DBError> {
        let _ = batch;
        unimplemented!()
    }
}
