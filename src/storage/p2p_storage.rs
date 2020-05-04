use storage::{
    StorageError,
    persistent::{KeyValueSchema, KeyValueStoreWithSchema, SchemaError, Decoder, Encoder},
};
use rocksdb::{ColumnFamilyDescriptor, Options, SliceTransform, DB};
use std::{
    sync::{
        atomic::{Ordering, AtomicU64}, Arc,
    }, net::SocketAddr,
};
use crate::storage::{rpc_message::RpcMessage, StoreMessage, encode_address};


pub type P2PMessageStorageKV = dyn KeyValueStoreWithSchema<P2PMessageStorage> + Sync + Send;

#[derive(Clone)]
pub struct P2PMessageStorage {
    kv: Arc<P2PMessageStorageKV>,
    host_index: P2PMessageSecondaryIndex,
    count: Arc<AtomicU64>,
    seq: Arc<AtomicU64>,
}

impl P2PMessageStorage {
    pub fn new(kv: Arc<DB>) -> Self {
        Self {
            kv: kv.clone(),
            host_index: P2PMessageSecondaryIndex::new(kv),
            count: Arc::new(AtomicU64::new(0)),
            seq: Arc::new(AtomicU64::new(std::u64::MAX)),
        }
    }

    fn count(&self) -> u64 {
        self.count.load(Ordering::SeqCst)
    }

    fn start(&self) -> u64 {
        self.seq.load(Ordering::SeqCst).saturating_add(1)
    }

    fn inc_count(&self) {
        self.count.fetch_add(1, Ordering::SeqCst);
    }

    fn index_next(&self) -> u64 {
        self.seq.fetch_sub(1, Ordering::SeqCst)
    }

    fn index(&self) -> u64 {
        self.seq.load(Ordering::SeqCst)
    }

    pub fn store_message(&mut self, msg: &StoreMessage) -> Result<(), StorageError> {
        let index = self.index_next();
        let remote_addr = msg.remote_addr();

        self.host_index.put(remote_addr, index)?;
        self.kv.put(&index, &msg)?;
        Ok(self.inc_count())
    }

    pub fn get_range(&self, offset: u64, count: u64) -> Result<Vec<RpcMessage>, StorageError> {
        let mut ret = Vec::with_capacity(count as usize);
        let end: u64 = self.index();
        let start = end.saturating_add(offset.saturating_add(1));
        let end = start.saturating_add(count);
        for index in start..=end {
            match self.kv.get(&index) {
                Ok(Some(value)) => ret.push(RpcMessage::from_store(&value, index.clone())),
                Ok(None) => {
                    log::info!("No value at index: {}", index);
                    continue;
                }
                Err(err) => {
                    log::warn!("Failed to load value at index {}: {}",index, err)
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
                Ok(Some(value)) => ret.push(RpcMessage::from_store(&value, index.clone())),
                Ok(None) => {
                    log::info!("No value at index: {}", index);
                    continue;
                }
                Err(err) => {
                    log::warn!("Failed to load value at index {}: {}",index, err)
                }
            }
        }
        Ok(ret)
    }
}

impl KeyValueSchema for P2PMessageStorage {
    type Key = u64;
    type Value = StoreMessage;

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
        let key = P2PMessageSecondaryKey::new(sock_addr, 0);
        let (offset, limit) = (offset as usize, limit as usize);

        let mut ret = Vec::with_capacity(limit);

        for index in self.kv.prefix_iterator(&key)?.skip(offset).take(limit).map(|(_, val)| val) {
            ret.push(index?)
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

