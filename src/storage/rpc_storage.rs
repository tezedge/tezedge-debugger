// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use storage::{StorageError, persistent::{KeyValueStoreWithSchema, KeyValueSchema}, IteratorMode, Direction};
use rocksdb::DB;
use std::{
    sync::{
        Arc, atomic::{AtomicU64, Ordering},
    }, net::{SocketAddr},
};
use tracing::{info, warn, field::{display, debug}};
use secondary_indexes::RemoteAddrIndex;
use crate::storage::secondary_index::SecondaryIndex;
use crate::storage::sorted_intersect::sorted_intersect;
use crate::messages::rpc_message::RpcMessage;

#[derive(Debug, Default, Clone)]
pub struct RpcFilters {
    pub remote_addr: Option<SocketAddr>,
}

impl RpcFilters {
    pub fn empty(&self) -> bool {
        self.remote_addr.is_none()
    }
}

pub type RpcMessageStorageKV = dyn KeyValueStoreWithSchema<RpcStore> + Sync + Send;

#[derive(Clone)]
pub struct RpcStore {
    kv: Arc<RpcMessageStorageKV>,
    remote_addr_index: RemoteAddrIndex,
    count: Arc<AtomicU64>,
    seq: Arc<AtomicU64>,
}

#[allow(dead_code)]
impl RpcStore {
    pub fn new(kv: Arc<DB>) -> Self {
        Self {
            kv: kv.clone(),
            remote_addr_index: RemoteAddrIndex::new(kv),
            count: Arc::new(AtomicU64::new(0)),
            seq: Arc::new(AtomicU64::new(0)),
        }
    }

    fn index(&self) -> u64 {
        self.seq.load(Ordering::SeqCst)
    }

    fn inc_count(&self) {
        self.count.fetch_add(1, Ordering::SeqCst);
    }

    pub fn reserve_index(&self) -> u64 {
        self.seq.fetch_add(1, Ordering::SeqCst)
    }

    pub fn make_indexes(&self, primary_index: u64, value: &RpcMessage) -> Result<(), StorageError> {
        self.remote_addr_index.store_index(&primary_index, value)
    }

    pub fn delete_indexes(&self, primary_index: u64, value: &RpcMessage) -> Result<(), StorageError> {
        self.remote_addr_index.delete_index(&primary_index, value)
    }

    pub fn put_message(&self, index: u64, msg: &mut RpcMessage) -> Result<(), StorageError> {
        msg.id = index;
        if self.kv.contains(&index)? {
            self.kv.merge(&index, &msg)?;
        } else {
            self.kv.put(&index, &msg)?;
            self.inc_count();
        }
        self.make_indexes(index, msg)?;
        Ok(())
    }

    pub fn store_message(&self, msg: &mut RpcMessage) -> Result<u64, StorageError> {
        let index = self.reserve_index();
        msg.id = index;
        self.kv.put(&index, &msg)?;
        self.make_indexes(index, &msg)?;
        self.inc_count();
        Ok(index)
    }

    pub fn get_cursor(&self, cursor_index: Option<u64>, limit: usize, filters: RpcFilters) -> Result<Vec<RpcMessage>, StorageError> {
        let mut ret = Vec::with_capacity(limit);
        if filters.empty() {
            ret.extend(self.cursor_iterator(cursor_index)?.take(limit).map(|(_key, value)| value));
        } else {
            let mut iters: Vec<Box<dyn Iterator<Item=u64>>> = Default::default();
            if let Some(remote_addr) = filters.remote_addr {
                iters.push(self.remote_addr_iterator(cursor_index, remote_addr)?);
            }
            ret.extend(self.load_indexes(sorted_intersect(iters, limit).into_iter()));
        }
        Ok(ret)
    }

    fn cursor_iterator<'a>(&'a self, cursor_index: Option<u64>) -> Result<Box<dyn 'a + Iterator<Item=(u64, RpcMessage)>>, StorageError> {
        Ok(Box::new(self.kv.iterator(IteratorMode::From(&cursor_index.unwrap_or(std::u64::MAX), Direction::Reverse))?
            .filter_map(|(k, v)| {
                k.ok().and_then(|key| Some((key, v.ok()?)))
            })))
    }

    fn remote_addr_iterator<'a>(&'a self, cursor_index: Option<u64>, remote_addr: SocketAddr) -> Result<Box<dyn 'a + Iterator<Item=u64>>, StorageError> {
        Ok(Box::new(self.remote_addr_index.get_concrete_prefix_iterator(&cursor_index.unwrap_or(std::u64::MAX), remote_addr)?
            .filter_map(|(_, value)| {
                value.ok()
            })))
    }

    fn load_indexes<Iter: 'static + Iterator<Item=u64>>(&self, indexes: Iter) -> impl Iterator<Item=RpcMessage> + 'static {
        let kv = self.kv.clone();
        indexes.filter_map(move |index| {
            match kv.get(&index) {
                Ok(Some(value)) => {
                    Some(value)
                }
                Ok(None) => {
                    info!("No value at index: {}", index);
                    None
                }
                Err(err) => {
                    warn!("Failed to load value at index {}: {}", index, err);
                    None
                }
            }
        })
    }
}

impl KeyValueSchema for RpcStore {
    type Key = u64;
    type Value = RpcMessage;

    fn name() -> &'static str { "rpc_message_storage" }
}

pub(crate) mod secondary_indexes {
    use storage::persistent::{KeyValueStoreWithSchema, KeyValueSchema, Decoder, SchemaError, Encoder};
    use std::sync::Arc;
    use rocksdb::{DB, ColumnFamilyDescriptor, Options, SliceTransform};
    use crate::storage::{RpcStore, encode_address};
    use crate::storage::secondary_index::SecondaryIndex;
    use std::net::SocketAddr;

    pub type RemoteAddressIndexKV = dyn KeyValueStoreWithSchema<RemoteAddrIndex> + Sync + Send;

    // 1. Remote Reverse index for getting latest messages from specific host

    #[derive(Clone)]
    pub struct RemoteAddrIndex {
        kv: Arc<RemoteAddressIndexKV>,
    }

    impl RemoteAddrIndex {
        pub fn new(kv: Arc<DB>) -> Self {
            Self { kv }
        }
    }

    impl AsRef<(dyn KeyValueStoreWithSchema<RemoteAddrIndex> + 'static)> for RemoteAddrIndex {
        fn as_ref(&self) -> &(dyn KeyValueStoreWithSchema<RemoteAddrIndex> + 'static) {
            self.kv.as_ref()
        }
    }

    impl KeyValueSchema for RemoteAddrIndex {
        type Key = RemoteKey;
        type Value = <RpcStore as KeyValueSchema>::Key;

        fn descriptor() -> ColumnFamilyDescriptor {
            let mut cf_opts = Options::default();
            cf_opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(16 + 2));
            cf_opts.set_memtable_prefix_bloom_ratio(0.2);
            ColumnFamilyDescriptor::new(Self::name(), cf_opts)
        }

        fn name() -> &'static str {
            "rpc_reverse_remote_index"
        }
    }

    impl SecondaryIndex<RpcStore> for RemoteAddrIndex {
        type FieldType = SocketAddr;

        fn accessor(value: &<RpcStore as KeyValueSchema>::Value) -> Option<Self::FieldType> {
            Some(value.remote_addr)
        }

        fn make_index(key: &<RpcStore as KeyValueSchema>::Key, value: Self::FieldType) -> <Self as KeyValueSchema>::Key {
            RemoteKey::new(value, key.clone())
        }

        fn make_prefix_index(value: Self::FieldType) -> <Self as KeyValueSchema>::Key {
            RemoteKey::prefix(value)
        }
    }

    #[derive(Debug, Clone)]
    pub struct RemoteKey {
        pub addr: u128,
        pub port: u16,
        pub index: u64,
    }

    impl RemoteKey {
        pub fn new(sock_addr: SocketAddr, index: u64) -> Self {
            let addr = sock_addr.ip();
            let port = sock_addr.port();
            Self {
                addr: encode_address(&addr),
                port,
                index: std::u64::MAX.saturating_sub(index),
            }
        }

        pub fn prefix(sock_addr: SocketAddr) -> Self {
            let addr = sock_addr.ip();
            let port = sock_addr.port();
            Self {
                addr: encode_address(&addr),
                port,
                index: 0,
            }
        }
    }

    /// * bytes layout: `[address(16)][port(2)][index(8)]`
    impl Decoder for RemoteKey {
        #[inline]
        fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
            if bytes.len() != 26 {
                return Err(SchemaError::DecodeError);
            }
            let addr_value = &bytes[0..16];
            let port_value = &bytes[16..16 + 2];
            let index_value = &bytes[16 + 2..];
            // index
            let mut index = [0u8; 8];
            for (x, y) in index.iter_mut().zip(index_value) {
                *x = *y;
            }
            let index = u64::from_be_bytes(index);
            // port
            let mut port = [0u8; 2];
            for (x, y) in port.iter_mut().zip(port_value) {
                *x = *y;
            }
            let port = u16::from_be_bytes(port);
            // addr
            let mut addr = [0u8; 16];
            for (x, y) in addr.iter_mut().zip(addr_value) {
                *x = *y;
            }
            let addr = u128::from_be_bytes(addr);

            Ok(Self {
                addr,
                port,
                index,
            })
        }
    }

    /// * bytes layout: `[address(16)][port(2)][index(8)]`
    impl Encoder for RemoteKey {
        #[inline]
        fn encode(&self) -> Result<Vec<u8>, SchemaError> {
            let mut buf = Vec::with_capacity(26);
            buf.extend_from_slice(&self.addr.to_be_bytes());
            buf.extend_from_slice(&self.port.to_be_bytes());
            buf.extend_from_slice(&self.index.to_be_bytes());

            if buf.len() != 26 {
                println!("{:?} - {:?}", self, buf);
                Err(SchemaError::EncodeError)
            } else {
                Ok(buf)
            }
        }
    }
}