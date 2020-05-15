use storage::{StorageError, persistent::{KeyValueSchema, KeyValueStoreWithSchema}, IteratorMode, Direction};
use rocksdb::DB;
use std::{
    sync::{
        atomic::{Ordering, AtomicU64}, Arc,
    }, net::SocketAddr,
};
use crate::storage::{rpc_message::RpcMessage, secondary_index::SecondaryIndex, StoreMessage, dissect};
use secondary_indexes::*;
use itertools::{Itertools};

pub type P2PMessageStorageKV = dyn KeyValueStoreWithSchema<P2PMessageStorage> + Sync + Send;

#[derive(Clone)]
pub struct P2PMessageStorage {
    base_index: Arc<AtomicU64>,
    kv: Arc<P2PMessageStorageKV>,
    remote_index: RemoteReverseIndex,
    type_index: TypeIndex,
    request_index: RequestTrackingIndex,
    remote_type_index: RemoteTypeIndex,
    count: Arc<AtomicU64>,
    seq: Arc<AtomicU64>,
}

impl P2PMessageStorage {
    pub fn new(kv: Arc<DB>) -> Self {
        Self {
            base_index: Arc::new(AtomicU64::new(std::u64::MAX)),
            kv: kv.clone(),
            remote_index: RemoteReverseIndex::new(kv.clone()),
            type_index: TypeIndex::new(kv.clone()),
            request_index: RequestTrackingIndex::new(kv.clone()),
            remote_type_index: RemoteTypeIndex::new(kv.clone()),
            count: Arc::new(AtomicU64::new(0)),
            seq: Arc::new(AtomicU64::new(0)),
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
        self.seq.fetch_add(1, Ordering::SeqCst)
    }

    fn index(&self) -> u64 {
        self.seq.load(Ordering::SeqCst)
    }

    fn base_index(&self) -> u64 {
        self.base_index.load(Ordering::SeqCst)
    }

    fn make_indexes(&mut self, primary_index: &u64, value: &StoreMessage) -> Result<(), StorageError> {
        self.remote_index.store_index(primary_index, value)?;
        self.type_index.store_index(primary_index, value)?;
        self.remote_index.store_index(primary_index, value)?;
        self.remote_type_index.store_index(primary_index, value)
    }

    fn delete_indexes(&mut self, primary_index: &u64, value: &StoreMessage) -> Result<(), StorageError> {
        self.remote_index.delete_index(primary_index, value)?;
        self.type_index.delete_index(primary_index, value)?;
        self.remote_index.delete_index(primary_index, value)?;
        self.remote_type_index.delete_index(primary_index, value)
    }

    pub fn reserve_index(&mut self) -> u64 {
        self.index_next()
    }

    pub fn put_message(&mut self, index: u64, msg: &StoreMessage) -> Result<(), StorageError> {
        self.make_indexes(&index, &msg)?;
        if self.kv.contains(&index)? {
            self.kv.merge(&index, &msg)?;
        } else {
            self.kv.put(&index, &msg)?;
            self.inc_count()
        }
        Ok(())
    }

    pub fn store_message(&mut self, msg: &StoreMessage) -> Result<u64, StorageError> {
        let index = self.index_next();
        self.make_indexes(&index, &msg)?;
        self.kv.put(&index, &msg)?;
        self.inc_count();
        Ok(index)
    }

    pub fn delete_message(&mut self, index: u64) -> Result<(), StorageError> {
        if let Ok(Some(msg)) = self.kv.get(&index) {
            self.kv.delete(&index)?;
            self.delete_indexes(&index, &msg)?;
        }
        Ok(())
    }

    pub fn reduce_db(&mut self) -> Result<(), StorageError> {
        let base = self.base_index();
        let index = self.index();
        let count = (base - index) / 2;
        let start = base - count;
        for index in start..=base {
            let _ = self.delete_message(index);
        }
        Ok(())
    }

    pub fn get_reverse_range(&self, offset_id: u64, count: usize) -> Result<Vec<RpcMessage>, StorageError> {
        self._get_range(if offset_id == 0 { std::u64::MAX } else { 0 } , count, true)
    }

    fn _get_range(&self, offset_id: u64, count: usize, backwards: bool) -> Result<Vec<RpcMessage>, StorageError> {
        let mut ret = Vec::with_capacity(count);
        let mode = IteratorMode::From(&offset_id, if backwards { Direction::Reverse } else { Direction::Forward });
        let iter = self.kv.iterator(mode)?.take(count);
        for (key, value) in iter {
            let value = key.and_then(|id| value.map(|value| (id, value)));
            match value {
                Ok((id, value)) => {
                    ret.push(RpcMessage::from_store(&value, id));
                }
                Err(err) => {
                    log::warn!("Failed to load value from iterator: {}", err);
                }
            }
        }
        Ok(ret)
    }

    pub fn get_remote_range(&self, offset: u64, count: u64, host: SocketAddr) -> Result<Vec<RpcMessage>, StorageError> {
        let idx = self.remote_index.get_raw_prefix_iterator(host)?
            .filter_map(|(_, val)| val.ok())
            .skip(offset as usize);
        let ret = self.load_indexes(Box::new(idx), count as usize)
            .fold(Vec::with_capacity(count as usize), |mut acc, value| {
                acc.push(value);
                acc
            });
        Ok(ret)
    }

    pub fn get_types_range(&self, msg_types: u32, offset: usize, count: usize) -> Result<Vec<RpcMessage>, StorageError> {
        if msg_types == 0 {
            Ok(Default::default())
        } else {
            let (part, mut rest) = dissect(msg_types);
            let mut idxs = Vec::new();
            let filter = |(_, val): (_, Result<u64, _>)| val.ok();
            let cmp: for<'r, 's> fn(&'r u64, &'s u64) -> bool = |x, y| x > y;
            idxs.push(self.type_index.get_raw_prefix_iterator(part)?
                .filter_map(filter));
            while rest != 0 {
                let (part, step) = dissect(rest);
                rest = step;
                idxs.push(self.type_index.get_raw_prefix_iterator(part)?
                    .filter_map(filter));
            }
            let idx = idxs.into_iter()
                .kmerge_by(cmp)
                .skip(offset);
            Ok(self.load_indexes(Box::new(idx), count)
                .fold(Vec::with_capacity(count as usize), |mut acc, value| {
                    acc.push(value);
                    acc
                }))
        }
    }

    pub fn get_remote_type_range(&self, offset: usize, count: usize, remote_host: SocketAddr, types: u32) -> Result<Vec<RpcMessage>, StorageError> {
        if types == 0 {
            Ok(Default::default())
        } else {
            let (part, mut rest) = dissect(types);
            let mut idxs = Vec::new();
            let filter = |(_, val): (_, Result<u64, _>)| val.ok();
            let cmp: for<'r, 's> fn(&'r u64, &'s u64) -> bool = |x, y| x > y;
            idxs.push(self.remote_type_index.get_raw_prefix_iterator((remote_host, part))?
                .filter_map(filter));
            while rest != 0 {
                let (part, step) = dissect(rest);
                rest = step;
                idxs.push(self.remote_type_index.get_raw_prefix_iterator((remote_host, part))?
                    .filter_map(filter));
            }
            let idx = idxs.into_iter()
                .kmerge_by(cmp)
                .skip(offset);
            Ok(self.load_indexes(Box::new(idx), count)
                .fold(Vec::with_capacity(count), |mut acc, value| {
                    acc.push(value);
                    acc
                }))
        }
    }

    pub fn get_request_range(&self, request_id: u64, offset: usize, count: usize) -> Result<Vec<RpcMessage>, StorageError> {
        let idx = self.request_index.get_raw_prefix_iterator(request_id)?
            .filter_map(|(_, val)| val.ok())
            .skip(offset as usize);
        let ret = self.load_indexes(Box::new(idx), count as usize)
            .fold(Vec::with_capacity(count as usize), |mut acc, value| {
                acc.push(value);
                acc
            });
        Ok(ret)
    }

    fn load_indexes<'a>(&self, indexes: Box<dyn Iterator<Item=u64> + 'a>, limit: usize) -> impl Iterator<Item=RpcMessage> + 'a {
        let kv = self.kv.clone();
        let mut count = 0;
        indexes.filter_map(move |index| {
            match kv.get(&index) {
                Ok(Some(value)) => {
                    count += 1;
                    Some(RpcMessage::from_store(&value, index.clone()))
                }
                Ok(None) => {
                    log::info!("No value at index: {}", index);
                    None
                }
                Err(err) => {
                    log::warn!("Failed to load value at index {}: {}",index, err);
                    None
                }
            }
        })
            .take(limit)
    }
}

impl KeyValueSchema for P2PMessageStorage {
    type Key = u64;
    type Value = StoreMessage;

    fn name() -> &'static str { "p2p_message_storage" }
}

pub mod secondary_indexes {
    use storage::persistent::{KeyValueStoreWithSchema, KeyValueSchema, Decoder, SchemaError, Encoder, BincodeEncoded};
    use std::sync::Arc;
    use rocksdb::{DB, ColumnFamilyDescriptor, Options, SliceTransform};
    use std::net::SocketAddr;
    use crate::storage::{encode_address, P2PMessageStorage, StoreMessage};
    use crate::storage::secondary_index::SecondaryIndex;
    use serde::{Serialize, Deserialize};
    use std::str::FromStr;
    use failure::{Fail, Error};

    pub type RemoteReverseIndexKV = dyn KeyValueStoreWithSchema<RemoteReverseIndex> + Sync + Send;

    // 1. Remote Reverse index for getting latest messages from specific host

    #[derive(Clone)]
    pub struct RemoteReverseIndex {
        kv: Arc<RemoteReverseIndexKV>,
    }

    impl RemoteReverseIndex {
        pub fn new(kv: Arc<DB>) -> Self {
            Self { kv }
        }
    }

    impl AsRef<(dyn KeyValueStoreWithSchema<RemoteReverseIndex> + 'static)> for RemoteReverseIndex {
        fn as_ref(&self) -> &(dyn KeyValueStoreWithSchema<RemoteReverseIndex> + 'static) {
            self.kv.as_ref()
        }
    }

    impl KeyValueSchema for RemoteReverseIndex {
        type Key = RemoteIndex;
        type Value = <P2PMessageStorage as KeyValueSchema>::Key;

        fn descriptor() -> ColumnFamilyDescriptor {
            let mut cf_opts = Options::default();
            cf_opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(16 + 2));
            cf_opts.set_memtable_prefix_bloom_ratio(0.2);
            ColumnFamilyDescriptor::new(Self::name(), cf_opts)
        }

        fn name() -> &'static str {
            "p2p_reverse_remote_index"
        }
    }

    impl SecondaryIndex<P2PMessageStorage> for RemoteReverseIndex {
        type FieldType = SocketAddr;

        fn accessor(value: &<P2PMessageStorage as KeyValueSchema>::Value) -> Option<Self::FieldType> {
            Some(value.remote_addr())
        }

        fn make_index(key: &<P2PMessageStorage as KeyValueSchema>::Key, value: Self::FieldType) -> <Self as KeyValueSchema>::Key {
            RemoteIndex::new(value, key.clone())
        }

        fn make_prefix_index(value: Self::FieldType) -> <Self as KeyValueSchema>::Key {
            RemoteIndex::prefix(value)
        }
    }

    #[derive(Debug, Clone)]
    pub struct RemoteIndex {
        pub addr: u128,
        pub port: u16,
        pub index: u64,
    }

    impl RemoteIndex {
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
    impl Decoder for RemoteIndex {
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
    impl Encoder for RemoteIndex {
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

    // 2. Type index

    pub type TypeIndexKV = dyn KeyValueStoreWithSchema<TypeIndex> + Sync + Send;

    #[derive(Clone)]
    pub struct TypeIndex {
        kv: Arc<TypeIndexKV>,
    }

    impl TypeIndex {
        pub fn new(kv: Arc<DB>) -> Self {
            Self { kv }
        }
    }

    impl AsRef<(dyn KeyValueStoreWithSchema<TypeIndex> + 'static)> for TypeIndex {
        fn as_ref(&self) -> &(dyn KeyValueStoreWithSchema<TypeIndex> + 'static) {
            self.kv.as_ref()
        }
    }

    impl KeyValueSchema for TypeIndex {
        type Key = TypeId;
        type Value = <P2PMessageStorage as KeyValueSchema>::Key;

        fn descriptor() -> ColumnFamilyDescriptor {
            let mut cf_opts = Options::default();
            cf_opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(4));
            cf_opts.set_memtable_prefix_bloom_ratio(0.2);
            ColumnFamilyDescriptor::new(Self::name(), cf_opts)
        }

        fn name() -> &'static str {
            "p2p_type_index"
        }
    }

    impl SecondaryIndex<P2PMessageStorage> for TypeIndex {
        type FieldType = u32;

        fn accessor(value: &<P2PMessageStorage as KeyValueSchema>::Value) -> Option<Self::FieldType> {
            Some(Type::extract(value))
        }

        fn make_index(key: &<P2PMessageStorage as KeyValueSchema>::Key, value: Self::FieldType) -> <Self as KeyValueSchema>::Key {
            TypeId::new(value, key.clone())
        }

        fn make_prefix_index(value: Self::FieldType) -> <Self as KeyValueSchema>::Key {
            TypeId::prefix(value)
        }
    }

    #[derive(Debug, Copy, Clone, Serialize, Deserialize)]
    pub struct TypeId {
        pub r#type: u32,
        pub index: u64,
    }

    impl TypeId {
        pub fn new(r#type: u32, index: u64) -> Self {
            Self {
                r#type,
                index: std::u64::MAX.saturating_sub(index),
            }
        }

        pub fn prefix(r#type: u32) -> Self {
            Self {
                r#type,
                index: 0,
            }
        }
    }

    impl BincodeEncoded for TypeId {}

    #[repr(u32)]
    pub enum Type {
        // Base Types
        Tcp = 0x1 << 0,
        Metadata = 0x1 << 1,
        ConnectionMessage = 0x1 << 2,
        RestMessage = 0x1 << 3,
        // P2P messages
        P2PMessage = 0x1 << 4,
        Disconnect = 0x1 << 5,
        Advertise = 0x1 << 6,
        SwapRequest = 0x1 << 7,
        SwapAck = 0x1 << 8,
        Bootstrap = 0x1 << 9,
        GetCurrentBranch = 0x1 << 10,
        CurrentBranch = 0x1 << 11,
        Deactivate = 0x1 << 12,
        GetCurrentHead = 0x1 << 13,
        CurrentHead = 0x1 << 14,
        GetBlockHeaders = 0x1 << 15,
        BlockHeader = 0x1 << 16,
        GetOperations = 0x1 << 17,
        Operation = 0x1 << 18,
        GetProtocols = 0x1 << 19,
        Protocol = 0x1 << 20,
        GetOperationHashesForBlocks = 0x1 << 21,
        OperationHashesForBlock = 0x1 << 22,
        GetOperationsForBlocks = 0x1 << 23,
        OperationsForBlocks = 0x1 << 24,
    }

    impl Type {
        pub fn parse_tags(tags: &str) -> Result<u32, Error> {
            let tags = tags.split(',');
            let mut ret = 0x0;
            for tag in tags {
                let type_tag: Type = tag.parse()?;
                ret |= type_tag as u32;
            }
            Ok(ret)
        }

        pub fn extract(value: &StoreMessage) -> u32 {
            use tezos_messages::p2p::encoding::peer::PeerMessage::*;
            match value {
                StoreMessage::TcpMessage { .. } => Self::Tcp as u32,
                StoreMessage::Metadata { .. } => Self::Metadata as u32,
                StoreMessage::ConnectionMessage { .. } => Self::ConnectionMessage as u32,
                StoreMessage::RestMessage { timestamp: _, incoming: _, remote_addr: _, payload: _ } => Self::RestMessage as u32,
                StoreMessage::P2PMessage { payload, .. } => {
                    if let Some(msg) = payload.first() {
                        match msg {
                            Disconnect => Self::Bootstrap as u32,
                            Bootstrap => Self::Bootstrap as u32,
                            Advertise(..) => Self::Advertise as u32,
                            SwapRequest(..) => Self::SwapRequest as u32,
                            SwapAck(..) => Self::SwapAck as u32,
                            GetCurrentBranch(..) => Self::GetCurrentBranch as u32,
                            CurrentBranch(..) => Self::CurrentBranch as u32,
                            Deactivate(..) => Self::Deactivate as u32,
                            GetCurrentHead(..) => Self::GetCurrentHead as u32,
                            CurrentHead(..) => Self::CurrentHead as u32,
                            GetBlockHeaders(..) => Self::GetBlockHeaders as u32,
                            BlockHeader(..) => Self::BlockHeader as u32,
                            GetOperations(..) => Self::GetOperations as u32,
                            Operation(..) => Self::Operation as u32,
                            GetProtocols(..) => Self::GetProtocols as u32,
                            Protocol(..) => Self::Protocol as u32,
                            GetOperationHashesForBlocks(..) => Self::GetOperationHashesForBlocks as u32,
                            OperationHashesForBlock(..) => Self::OperationHashesForBlock as u32,
                            GetOperationsForBlocks(..) => Self::GetOperationsForBlocks as u32,
                            OperationsForBlocks(..) => Self::OperationsForBlocks as u32,
                        }
                    } else {
                        Self::P2PMessage as u32
                    }
                }
            }
        }
    }

    #[derive(Debug, Fail)]
    #[fail(display = "Invalid message type {}", _0)]
    pub struct ParseTypeError(String);

    impl<T: AsRef<str>> From<T> for ParseTypeError {
        fn from(value: T) -> Self {
            Self(value.as_ref().to_string())
        }
    }

    impl FromStr for Type {
        type Err = ParseTypeError;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            match s {
                "tcp" => Ok(Self::Tcp),
                "metadata" => Ok(Self::Metadata),
                "connection_message" => Ok(Self::ConnectionMessage),
                "rest_message" => Ok(Self::RestMessage),
                "p2p_message" => Ok(Self::P2PMessage),
                "disconnect" => Ok(Self::Disconnect),
                "advertise" => Ok(Self::Advertise),
                "swap_request" => Ok(Self::SwapRequest),
                "swap_ack" => Ok(Self::SwapAck),
                "bootstrap" => Ok(Self::Bootstrap),
                "get_current_branch" => Ok(Self::GetCurrentBranch),
                "current_branch" => Ok(Self::CurrentBranch),
                "deactivate" => Ok(Self::Deactivate),
                "get_current_head" => Ok(Self::GetCurrentHead),
                "current_head" => Ok(Self::CurrentHead),
                "get_block_header" => Ok(Self::GetBlockHeaders),
                "block_header" => Ok(Self::BlockHeader),
                "get_operations" => Ok(Self::GetOperations),
                "operation" => Ok(Self::Operation),
                "get_protocols" => Ok(Self::GetProtocols),
                "protocol" => Ok(Self::Protocol),
                "get_operation_hashes_for_blocks" => Ok(Self::GetOperationHashesForBlocks),
                "operation_hashes_for_block" => Ok(Self::OperationHashesForBlock),
                "get_operations_for_blocks" => Ok(Self::GetOperationsForBlocks),
                "operations_for_blocks" => Ok(Self::OperationsForBlocks),
                s => Err(s.into())
            }
        }
    }

    // 3. Request Stream Index
    pub type RequestTrackingIndexKV = dyn KeyValueStoreWithSchema<RequestTrackingIndex> + Sync + Send;

    #[derive(Clone)]
    pub struct RequestTrackingIndex {
        kv: Arc<RequestTrackingIndexKV>,
    }

    impl RequestTrackingIndex {
        pub fn new(kv: Arc<DB>) -> Self {
            Self { kv }
        }
    }

    impl AsRef<(dyn KeyValueStoreWithSchema<RequestTrackingIndex> + 'static)> for RequestTrackingIndex {
        fn as_ref(&self) -> &(dyn KeyValueStoreWithSchema<RequestTrackingIndex> + 'static) {
            self.kv.as_ref()
        }
    }

    impl KeyValueSchema for RequestTrackingIndex {
        type Key = RequestIndex;
        type Value = <P2PMessageStorage as KeyValueSchema>::Key;

        fn descriptor() -> ColumnFamilyDescriptor {
            let mut cf_opts = Options::default();
            cf_opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(8));
            cf_opts.set_memtable_prefix_bloom_ratio(0.2);
            ColumnFamilyDescriptor::new(Self::name(), cf_opts)
        }

        fn name() -> &'static str {
            "p2p_request_index"
        }
    }

    impl SecondaryIndex<P2PMessageStorage> for RequestTrackingIndex {
        type FieldType = u64;

        fn accessor(value: &<P2PMessageStorage as KeyValueSchema>::Value) -> Option<Self::FieldType> {
            match value {
                StoreMessage::P2PMessage { request_id, .. } => request_id.clone(),
                _ => None
            }
        }

        fn make_index(key: &<P2PMessageStorage as KeyValueSchema>::Key, value: Self::FieldType) -> <Self as KeyValueSchema>::Key {
            RequestIndex::new(key.clone(), value)
        }

        fn make_prefix_index(value: Self::FieldType) -> <Self as KeyValueSchema>::Key {
            RequestIndex::prefix(value)
        }
    }

    #[derive(Debug, Copy, Clone, Serialize, Deserialize)]
    pub struct RequestIndex {
        pub request_index: u64,
        pub index: u64,
    }

    impl RequestIndex {
        pub fn new(request_index: u64, index: u64) -> Self {
            Self { request_index, index: std::u64::MAX.saturating_sub(index) }
        }

        pub fn prefix(request_index: u64) -> Self {
            Self { request_index, index: 0 }
        }
    }

    impl BincodeEncoded for RequestIndex {}

    // 4. Remote (host) + Type index
    pub type RemoteTypeIndexKV = dyn KeyValueStoreWithSchema<RemoteTypeIndex> + Sync + Send;

    #[derive(Clone)]
    pub struct RemoteTypeIndex {
        kv: Arc<RemoteTypeIndexKV>
    }

    impl RemoteTypeIndex {
        pub fn new(kv: Arc<DB>) -> Self {
            Self { kv }
        }
    }

    impl AsRef<(dyn KeyValueStoreWithSchema<RemoteTypeIndex> + 'static)> for RemoteTypeIndex {
        fn as_ref(&self) -> &(dyn KeyValueStoreWithSchema<RemoteTypeIndex> + 'static) {
            self.kv.as_ref()
        }
    }

    impl KeyValueSchema for RemoteTypeIndex {
        type Key = RemoteTypeKey;
        type Value = <P2PMessageStorage as KeyValueSchema>::Key;

        fn descriptor() -> ColumnFamilyDescriptor {
            let mut cf_opts = Options::default();
            cf_opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(16 + 2 + 4));
            cf_opts.set_memtable_prefix_bloom_ratio(0.2);
            ColumnFamilyDescriptor::new(Self::name(), cf_opts)
        }

        fn name() -> &'static str {
            "p2p_remote_type_index"
        }
    }

    impl SecondaryIndex<P2PMessageStorage> for RemoteTypeIndex {
        type FieldType = (SocketAddr, u32);

        fn accessor(value: &<P2PMessageStorage as KeyValueSchema>::Value) -> Option<Self::FieldType> {
            Some((value.remote_addr(), Type::extract(value)))
        }

        fn make_index(key: &<P2PMessageStorage as KeyValueSchema>::Key, value: Self::FieldType) -> <Self as KeyValueSchema>::Key {
            let (addr, typ) = value;
            RemoteTypeKey::new(addr, typ, key.clone())
        }

        fn make_prefix_index(value: Self::FieldType) -> <Self as KeyValueSchema>::Key {
            let (addr, typ) = value;
            RemoteTypeKey::prefix(addr, typ)
        }
    }

    #[derive(Debug, Copy, Clone, Serialize, Deserialize)]
    pub struct RemoteTypeKey {
        pub remote_addr: u128,
        pub port: u16,
        pub r#type: u32,
        pub index: u64,
    }

    impl RemoteTypeKey {
        pub fn new(sock_addr: SocketAddr, r#type: u32, index: u64) -> Self {
            Self {
                remote_addr: encode_address(&sock_addr.ip()),
                port: sock_addr.port(),
                r#type,
                index: std::u64::MAX.saturating_sub(index),
            }
        }

        pub fn prefix(sock_addr: SocketAddr, r#type: u32) -> Self {
            Self {
                remote_addr: encode_address(&sock_addr.ip()),
                port: sock_addr.port(),
                r#type,
                index: 0,
            }
        }
    }

    impl Decoder for RemoteTypeKey {
        #[inline]
        fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
            if bytes.len() != 30 {
                Err(SchemaError::DecodeError)
            } else {
                let addr_value = &bytes[0..16];
                let port_value = &bytes[16..16 + 2];
                let type_value = &bytes[16 + 2..16 + 2 + 4];
                let index_value = &bytes[16 + 2 + 4..];
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
                let remote_addr = u128::from_be_bytes(addr);
                // type
                let mut typ = [0u8; 4];
                for (x, y) in typ.iter_mut().zip(type_value) {
                    *x = *y;
                }
                let r#type = u32::from_be_bytes(typ);

                Ok(Self {
                    remote_addr,
                    r#type,
                    port,
                    index,
                })
            }
        }
    }

    impl Encoder for RemoteTypeKey {
        #[inline]
        fn encode(&self) -> Result<Vec<u8>, SchemaError> {
            let mut buf = Vec::with_capacity(30);
            buf.extend_from_slice(&self.remote_addr.to_be_bytes());
            buf.extend_from_slice(&self.port.to_be_bytes());
            buf.extend_from_slice(&self.r#type.to_be_bytes());
            buf.extend_from_slice(&self.index.to_be_bytes());

            if buf.len() != 30 {
                println!("{:?} - {:?}", self, buf);
                Err(SchemaError::EncodeError)
            } else {
                Ok(buf)
            }
        }
    }
}
