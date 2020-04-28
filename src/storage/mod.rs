pub mod storage_message;
pub mod rpc_message;

pub use storage_message::*;

use rocksdb::{DB, ColumnFamilyDescriptor, Options, SliceTransform};
use std::sync::{Arc, atomic::{AtomicU64, Ordering}};
use failure::Error;
use storage::persistent::{KeyValueStoreWithSchema, KeyValueSchema, Decoder, SchemaError, Encoder};
use lazy_static::lazy_static;
use std::time::{SystemTime, UNIX_EPOCH};
use storage::StorageError;
use std::net::{SocketAddr, IpAddr};
use crate::storage::rpc_message::RpcMessage;

#[derive(Clone)]
pub struct MessageStore {
    p2p_db: P2PMessageStorage,
    rpc_db: RpcMessageStorage,
    raw_db: Arc<DB>,
}

impl MessageStore {
    pub fn new(db: Arc<DB>) -> Self {
        Self {
            p2p_db: P2PMessageStorage::new(db.clone()),
            rpc_db: RpcMessageStorage::new(db.clone()),
            raw_db: db,
        }
    }

    pub fn store_p2p_message(&mut self, data: &StoreMessage) -> Result<(), Error> {
        self.p2p_db.store_message(&data)
            .map_err(|e| e.into())
    }

    pub fn store_rpc_message(&mut self, data: &StoreMessage) -> Result<(), Error> {
        self.rpc_db.store_message(&data)
            .map_err(|e| e.into())
    }

    pub fn get_p2p_range(&mut self, offset: u64, count: u64) -> Result<Vec<RpcMessage>, Error> {
        Ok(self.p2p_db.get_range(offset, count)?)
    }

    pub fn get_p2p_host_range(&mut self, offset: u64, count: u64, host: SocketAddr) -> Result<Vec<RpcMessage>, Error> {
        Ok(self.p2p_db.get_host_range(offset, count, host)?)
    }

    pub fn get_rpc_range(&mut self, offset: u64, count: u64) -> Result<Vec<RpcMessage>, Error> {
        Ok(self.rpc_db.get_range(offset, count)?)
    }

    pub fn get_rpc_host_range(&mut self, offset: u64, count: u64, host: SocketAddr) -> Result<Vec<RpcMessage>, Error> {
        Ok(self.rpc_db.get_host_range(offset, count, host)?)
    }
}

lazy_static! {
    static ref P2P_COUNT: Arc<AtomicU64> = Arc::new(AtomicU64::new(0));
    static ref RPC_COUNT: Arc<AtomicU64> = Arc::new(AtomicU64::new(0));
    static ref P2P_SEQ: Arc<AtomicU64> = Arc::new(AtomicU64::new(std::u64::MAX));
    static ref RPC_SEQ: Arc<AtomicU64> = Arc::new(AtomicU64::new(std::u64::MAX));
}

fn get_ts() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
}

pub type P2PMessageStorageKV = dyn KeyValueStoreWithSchema<P2PMessageStorage> + Sync + Send;

#[derive(Clone)]
pub struct P2PMessageStorage {
    kv: Arc<P2PMessageStorageKV>,
    host_index: P2PMessageSecondaryIndex,
}

impl P2PMessageStorage {
    pub fn new(kv: Arc<DB>) -> Self {
        Self {
            kv: kv.clone(),
            host_index: P2PMessageSecondaryIndex::new(kv),
        }
    }

    fn count(&self) -> u64 {
        P2P_COUNT.load(Ordering::SeqCst)
    }

    fn start(&self) -> u64 {
        P2P_SEQ.load(Ordering::SeqCst).saturating_add(1)
    }

    fn inc_count(&self) {
        P2P_COUNT.fetch_add(1, Ordering::SeqCst);
    }

    fn index_next() -> u64 {
        P2P_SEQ.fetch_sub(1, Ordering::SeqCst)
    }

    fn index() -> u64 {
        P2P_SEQ.load(Ordering::SeqCst)
    }

    pub fn store_message(&mut self, msg: &StoreMessage) -> Result<(), StorageError> {
        let index = Self::index_next();
        let remote_addr = msg.remote_addr();
        let msg = RpcMessage::from_store(msg, index);

        self.host_index.put(remote_addr, index)?;
        self.kv.put(&index, &msg)?;
        Ok(self.inc_count())
    }

    pub fn get_range(&self, offset: u64, count: u64) -> Result<Vec<RpcMessage>, StorageError> {
        let count = std::cmp::max(count, 100);
        let mut ret = Vec::with_capacity(count as usize);
        let end: u64 = Self::index();
        let start = end.saturating_add(offset.saturating_add(1));
        let end = start.saturating_add(count);
        for index in start..=end {
            match self.kv.get(&index) {
                Ok(Some(value)) => ret.push(value.into()),
                Ok(None) => {
                    log::info!("No value at index: {}", index);
                    continue;
                }
                Err(err) => {
                    log::info!("Failed to load value at index {}: {}",index, err)
                }
            }
        }
        Ok(ret)
    }

    pub fn get_host_range(&self, offset: u64, count: u64, host: SocketAddr) -> Result<Vec<RpcMessage>, StorageError> {
        let idx = self.host_index.get_for_host(host, offset, count)?;
        let mut ret = Vec::with_capacity(idx.len());
        for index in idx.iter() {
            match self.kv.get(index) {
                Ok(Some(value)) => ret.push(value.into()),
                Ok(None) => {
                    log::info!("No value at index: {}", index);
                    continue;
                }
                Err(err) => {
                    log::info!("Failed to load value at index {}: {}",index, err)
                }
            }
        }
        Ok(ret)
    }
}

impl KeyValueSchema for P2PMessageStorage {
    type Key = u64;
    type Value = RpcMessage;

    fn name() -> &'static str { "p2p_message_storage" }
}

pub type P2PMessageSecondaryIndexKV = dyn KeyValueStoreWithSchema<P2PMessageSecondaryIndex> + Sync + Send;

#[derive(Clone)]
pub struct P2PMessageSecondaryIndex {
    kv: Arc<P2PMessageSecondaryIndexKV>,
}

impl P2PMessageSecondaryIndex {
    pub fn new(kv: Arc<DB>) -> Self {
        Self { kv }
    }

    #[inline]
    pub fn put(&mut self, sock_addr: SocketAddr, index: u64) -> Result<(), StorageError> {
        let key = P2PMessageSecondaryKey::new(sock_addr, index);
        Ok(self.kv.put(&key, &index)?)
    }

    pub fn get(&self, sock_addr: SocketAddr, index: u64) -> Result<Option<u64>, StorageError> {
        let key = P2PMessageSecondaryKey::new(sock_addr, index);
        Ok(self.kv.get(&key)?)
    }

    pub fn get_for_host(&self, sock_addr: SocketAddr, offset: u64, limit: u64) -> Result<Vec<u64>, StorageError> {
        use circular_queue::CircularQueue;
        let key = P2PMessageSecondaryKey::new(sock_addr, offset as u64);
        let (offset, limit) = (offset as usize, limit as usize);

        let mut ret = Vec::with_capacity(limit);

        let mut queue: CircularQueue<u64> = CircularQueue::with_capacity(offset + limit);
        for index in self.kv.prefix_iterator(&key)?.map(|(_, val)| val) {
            queue.push(index?);
        }

        for index in queue.iter().skip(offset) {
            ret.push(*index)
        }

        Ok(ret)
    }
}

impl KeyValueSchema for P2PMessageSecondaryIndex {
    type Key = P2PMessageSecondaryKey;
    type Value = u64;

    fn descriptor() -> ColumnFamilyDescriptor {
        let mut cf_opts = Options::default();
        cf_opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(16 + 2));
        cf_opts.set_memtable_prefix_bloom_ratio(0.2);
        ColumnFamilyDescriptor::new(Self::name(), cf_opts)
    }

    fn name() -> &'static str {
        "p2p_message_secondary_index"
    }
}

#[derive(Debug, Clone)]
pub struct P2PMessageSecondaryKey {
    pub addr: u128,
    pub port: u16,
    pub index: u64,
}

impl P2PMessageSecondaryKey {
    pub fn new(sock_addr: SocketAddr, index: u64) -> Self {
        let addr = sock_addr.ip();
        let port = sock_addr.port();
        Self {
            addr: encode_address(&addr),
            port,
            index,
        }
    }

    pub fn prefix(sock_addr: SocketAddr) -> Self {
        Self::new(sock_addr, 0)
    }
}

fn encode_address(addr: &IpAddr) -> u128 {
    match addr {
        &IpAddr::V6(addr) => u128::from(addr),
        &IpAddr::V4(addr) => u32::from(addr) as u128,
    }
}

#[allow(dead_code)]
fn decode_address(value: u128) -> IpAddr {
    use std::net::{Ipv4Addr, Ipv6Addr};
    if value & 0xFFFFFFFFFFFFFFFFFFFFFFFF00000000 == 0 {
        IpAddr::V4(Ipv4Addr::from(value as u32))
    } else {
        IpAddr::V6(Ipv6Addr::from(value))
    }
}

/// * bytes layout: `[address(16)][port(2)][index(8)]`
impl Decoder for P2PMessageSecondaryKey {
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
impl Encoder for P2PMessageSecondaryKey {
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


pub type RpcMessageStorageKV = dyn KeyValueStoreWithSchema<RpcMessageStorage> + Sync + Send;

#[derive(Clone)]
pub struct RpcMessageStorage {
    kv: Arc<RpcMessageStorageKV>,
    host_index: RpcMessageSecondaryIndex,
}

impl RpcMessageStorage {
    pub fn new(kv: Arc<DB>) -> Self {
        Self {
            kv: kv.clone(),
            host_index: RpcMessageSecondaryIndex::new(kv),
        }
    }

    fn count(&self) -> u64 {
        RPC_COUNT.load(Ordering::SeqCst)
    }

    fn start(&self) -> u64 {
        RPC_SEQ.load(Ordering::SeqCst).saturating_add(1)
    }

    fn inc_count(&self) {
        RPC_COUNT.fetch_add(1, Ordering::SeqCst);
    }

    fn index_next() -> u64 {
        RPC_SEQ.fetch_sub(1, Ordering::SeqCst)
    }

    fn index() -> u64 {
        RPC_SEQ.load(Ordering::SeqCst)
    }

    pub fn store_message(&mut self, msg: &StoreMessage) -> Result<(), StorageError> {
        let index = Self::index_next();
        let remote_addr = msg.remote_addr();
        let rpc_msg = RpcMessage::from_store(msg, index);


        self.host_index.put(remote_addr, index)?;
        self.kv.put(&index, &rpc_msg)?;
        Ok(self.inc_count())
    }

    pub fn get_range(&self, offset: u64, count: u64) -> Result<Vec<RpcMessage>, StorageError> {
        let count = std::cmp::max(count, 100);
        let mut ret = Vec::with_capacity(count as usize);
        let end: u64 = Self::index();
        let start = end.saturating_add(offset.saturating_add(1));
        let end = start.saturating_add(count);
        for index in start..=end {
            match self.kv.get(&index) {
                Ok(Some(value)) => ret.push(value.into()),
                Ok(None) => {
                    log::info!("No value at index: {}", index);
                    continue;
                }
                Err(err) => {
                    log::info!("Failed to load value at index {}: {}",index, err)
                }
            }
        }
        Ok(ret)
    }

    pub fn get_host_range(&self, offset: u64, count: u64, host: SocketAddr) -> Result<Vec<RpcMessage>, StorageError> {
        let idx = self.host_index.get_for_host(host, offset, count)?;
        let mut ret = Vec::with_capacity(idx.len());
        for index in idx.iter() {
            match self.kv.get(index) {
                Ok(Some(value)) => ret.push(value.into()),
                Ok(None) => {
                    log::info!("No value at index: {}", index);
                    continue;
                }
                Err(err) => {
                    log::info!("Failed to load value at index {}: {}",index, err)
                }
            }
        }
        Ok(ret)
    }
}

impl KeyValueSchema for RpcMessageStorage {
    type Key = u64;
    type Value = RpcMessage;

    fn name() -> &'static str { "rpc_message_storage" }
}

pub type RpcMessageSecondaryIndexKV = dyn KeyValueStoreWithSchema<RpcMessageSecondaryIndex> + Sync + Send;

#[derive(Clone)]
pub struct RpcMessageSecondaryIndex {
    kv: Arc<RpcMessageSecondaryIndexKV>,
}

impl RpcMessageSecondaryIndex {
    pub fn new(kv: Arc<DB>) -> Self {
        Self { kv }
    }

    #[inline]
    pub fn put(&mut self, sock_addr: SocketAddr, index: u64) -> Result<(), StorageError> {
        let key = RpcMessageSecondaryKey::new(sock_addr, index);
        Ok(self.kv.put(&key, &index)?)
    }

    pub fn get(&self, sock_addr: SocketAddr, index: u64) -> Result<Option<u64>, StorageError> {
        let key = RpcMessageSecondaryKey::new(sock_addr, index);
        Ok(self.kv.get(&key)?)
    }

    pub fn get_for_host(&self, sock_addr: SocketAddr, offset: u64, limit: u64) -> Result<Vec<u64>, StorageError> {
        use circular_queue::CircularQueue;
        let key = RpcMessageSecondaryKey::new(sock_addr, offset as u64);
        let (offset, limit) = (offset as usize, limit as usize);

        let mut ret = Vec::with_capacity(limit);

        let mut queue: CircularQueue<u64> = CircularQueue::with_capacity(offset + limit);
        for index in self.kv.prefix_iterator(&key)?.map(|(_, val)| val) {
            queue.push(index?);
        }

        for index in queue.iter().skip(offset) {
            ret.push(*index)
        }

        Ok(ret)
    }
}

impl KeyValueSchema for RpcMessageSecondaryIndex {
    type Key = RpcMessageSecondaryKey;
    type Value = u64;

    fn descriptor() -> ColumnFamilyDescriptor {
        let mut cf_opts = Options::default();
        cf_opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(16 + 2));
        cf_opts.set_memtable_prefix_bloom_ratio(0.2);
        ColumnFamilyDescriptor::new(Self::name(), cf_opts)
    }

    fn name() -> &'static str {
        "rpc_message_secondary_index"
    }
}

#[derive(Debug, Clone)]
pub struct RpcMessageSecondaryKey {
    pub addr: u128,
    pub port: u16,
    pub index: u64,
}

impl RpcMessageSecondaryKey {
    pub fn new(sock_addr: SocketAddr, index: u64) -> Self {
        let addr = sock_addr.ip();
        let port = sock_addr.port();
        Self {
            addr: encode_address(&addr),
            port,
            index,
        }
    }

    pub fn prefix(sock_addr: SocketAddr) -> Self {
        Self::new(sock_addr, 0)
    }
}

/// * bytes layout: `[address(16)][port(2)][index(8)]`
impl Decoder for RpcMessageSecondaryKey {
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
impl Encoder for RpcMessageSecondaryKey {
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

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::Arc;
    use crate::storage::rpc_message::RESTMessage;
    use storage::persistent::open_kv;

    macro_rules! function {
    () => {{
        fn f() {}
        fn type_name_of<T>(_: T) -> &'static str {
            std::any::type_name::<T>()
        }
        let name = type_name_of(f);
        &name[..name.len() - 3]
    }}
}

    struct Store(pub MessageStore);

    impl Drop for Store {
        fn drop(&mut self) {
            use std::fs;
            let path = self.0.raw_db.path();
            fs::remove_dir_all(path).expect("failed to delete testing database");
        }
    }

    fn create_test_db<P: AsRef<Path>>(path: P) -> Store {
        let schemas = vec![
            crate::storage::RpcMessageStorage::descriptor(),
            crate::storage::P2PMessageStorage::descriptor(),
            crate::storage::RpcMessageSecondaryIndex::descriptor(),
            crate::storage::P2PMessageSecondaryIndex::descriptor(),
        ];
        let rocksdb = Arc::new(open_kv(path, schemas).expect("failed to open database"));
        Store(MessageStore::new(rocksdb))
    }

    #[test]
    fn test_create_db() {
        use std::path::Path;
        let path = function!();
        {
            let _ = create_test_db(path);
        }
        assert!(!Path::new(path).exists())
    }

    #[test]
    fn read_range() {
        let mut db = create_test_db(function!());
        let sock: SocketAddr = "0.0.0.0:1010".parse().unwrap();
        for x in 0usize..10 {
            let ret = db.0.store_rpc_message(
                &StoreMessage::RestMessage {
                    incoming: true,
                    remote_addr: sock,
                    payload: RESTMessage::Response {
                        status: "200".to_string(),
                        payload: format!("{}", x),
                    },
                });
            if ret.is_err() {
                assert!(false, "failed to store message: {}", ret.unwrap_err())
            }
        }
        let msgs = db.0.get_rpc_range(0, 10).unwrap();
        assert_eq!(msgs.len(), 10);
        for (msg, idx) in msgs.iter().zip(9..=0) {
            match msg {
                RpcMessage::RestMessage { message, .. } => {
                    match message {
                        RESTMessage::Response { payload, .. } => {
                            assert_eq!(payload, &format!("{}", idx));
                        }
                        _ => assert!(false, "Expected response message")
                    }
                }
                _ => assert!(false, "Expected rest message")
            }
        }
    }
}