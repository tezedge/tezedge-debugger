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
use crate::storage::sorted_intersect::sorted_intersect;

#[derive(Debug, Default, Clone)]
pub struct P2PFilters {
    pub remote_addr: Option<SocketAddr>,
    pub types: Option<u32>,
    pub request_id: Option<u64>,
    pub incoming: Option<bool>,
}

impl P2PFilters {
    pub fn empty(&self) -> bool {
        self.remote_addr.is_none() && self.types.is_none()
            && self.request_id.is_none() && self.incoming.is_none()
    }
}

pub type P2PMessageStorageKV = dyn KeyValueStoreWithSchema<P2PStorage> + Sync + Send;

#[derive(Clone)]
pub struct P2PStorage {
    kv: Arc<P2PMessageStorageKV>,
    remote_addr_index: RemoteAddrIndex,
    type_index: TypeIndex,
    tracking_index: RequestTrackingIndex,
    incoming_index: IncomingIndex,
    count: Arc<AtomicU64>,
    seq: Arc<AtomicU64>,
}

impl P2PStorage {
    pub fn new(kv: Arc<DB>) -> Self {
        Self {
            kv: kv.clone(),
            remote_addr_index: RemoteAddrIndex::new(kv.clone()),
            type_index: TypeIndex::new(kv.clone()),
            tracking_index: RequestTrackingIndex::new(kv.clone()),
            incoming_index: IncomingIndex::new(kv.clone()),
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

    pub fn make_indexes(&self, primary_index: u64, value: &StoreMessage) -> Result<(), StorageError> {
        self.remote_addr_index.store_index(&primary_index, value)?;
        self.type_index.store_index(&primary_index, value)?;
        self.tracking_index.store_index(&primary_index, value)?;
        self.incoming_index.store_index(&primary_index, value)
    }

    pub fn delete_indexes(&self, primary_index: u64, value: &StoreMessage) -> Result<(), StorageError> {
        self.remote_addr_index.delete_index(&primary_index, value)?;
        self.type_index.delete_index(&primary_index, value)?;
        self.tracking_index.delete_index(&primary_index, value)?;
        self.incoming_index.delete_index(&primary_index, value)
    }

    pub fn put_message(&self, index: u64, msg: &StoreMessage) -> Result<(), StorageError> {
        if self.kv.contains(&index)? {
            self.kv.merge(&index, &msg)?;
        } else {
            self.kv.put(&index, &msg)?;
            self.inc_count();
        }
        self.make_indexes(index, msg)?;
        Ok(())
    }

    pub fn store_message(&self, msg: &StoreMessage) -> Result<u64, StorageError> {
        let index = self.reserve_index();
        self.kv.put(&index, &msg)?;
        self.make_indexes(index, &msg)?;
        self.inc_count();
        Ok(index)
    }

    pub fn get_cursor(&self, cursor_index: Option<u64>, limit: usize, filters: P2PFilters) -> Result<Vec<RpcMessage>, StorageError> {
        let mut ret = Vec::with_capacity(limit);
        if filters.empty() {
            ret.extend(self.cursor_iterator(cursor_index)?.take(limit).map(|(key, value)| RpcMessage::from_store(&value, key)));
        } else {
            let mut iters: Vec<Box<dyn Iterator<Item=u64>>> = Default::default();
            if let Some(remote_addr) = filters.remote_addr {
                iters.push(self.remote_addr_iterator(cursor_index, remote_addr)?);
            }
            if let Some(types) = filters.types {
                iters.push(self.type_iterator(cursor_index, types)?);
            }
            if let Some(tracking) = filters.request_id {
                iters.push(self.tracking_iterator(cursor_index, tracking)?);
            }
            if let Some(incoming) = filters.incoming {
                iters.push(self.incoming_iterator(cursor_index, incoming)?);
            }
            ret.extend(self.load_indexes(sorted_intersect(iters, limit).into_iter()));
        }
        Ok(ret)
    }

    fn cursor_iterator<'a>(&'a self, cursor_index: Option<u64>) -> Result<Box<dyn 'a + Iterator<Item=(u64, StoreMessage)>>, StorageError> {
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

    fn type_iterator<'a>(&'a self, cursor_index: Option<u64>, types: u32) -> Result<Box<dyn 'a + Iterator<Item=u64>>, StorageError> {
        let types = dissect(types);
        let mut idxs = Vec::new();
        let filter = |(_, val): (_, Result<u64, _>)| val.ok();
        for r#type in types {
            idxs.push(self.type_index.get_concrete_prefix_iterator(&cursor_index.unwrap_or(std::u64::MAX), r#type)?
                .filter_map(filter))
        }
        let cmp: for<'r, 's> fn(&'r u64, &'s u64) -> bool = |x, y| x > y;
        Ok(Box::new(idxs.into_iter()
            .kmerge_by(cmp)))
    }

    fn tracking_iterator<'a>(&'a self, cursor_index: Option<u64>, request_id: u64) -> Result<Box<dyn 'a + Iterator<Item=u64>>, StorageError> {
        Ok(Box::new(self.tracking_index.get_concrete_prefix_iterator(&cursor_index.unwrap_or(std::u64::MAX), request_id)?
            .filter_map(|(_, value)| {
                value.ok()
            })))
    }

    fn incoming_iterator<'a>(&'a self, cursor_index: Option<u64>, is_incoming: bool) -> Result<Box<dyn 'a + Iterator<Item=u64>>, StorageError> {
        Ok(Box::new(self.incoming_index.get_concrete_prefix_iterator(&cursor_index.unwrap_or(std::u64::MAX), is_incoming)?
            .filter_map(|(_, value)| {
                value.ok()
            })))
    }

    fn load_indexes<Iter: 'static + Iterator<Item=u64>>(&self, indexes: Iter) -> impl Iterator<Item=RpcMessage> + 'static {
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
    }
}

impl KeyValueSchema for P2PStorage {
    type Key = u64;
    type Value = StoreMessage;

    fn name() -> &'static str { "p2p_message_storage" }
}

pub mod secondary_indexes {
    use storage::persistent::{KeyValueStoreWithSchema, KeyValueSchema, Decoder, SchemaError, Encoder, BincodeEncoded};
    use std::sync::Arc;
    use rocksdb::{DB, ColumnFamilyDescriptor, Options, SliceTransform};
    use std::net::SocketAddr;
    use crate::storage::{encode_address, P2PStorage, StoreMessage};
    use crate::storage::secondary_index::SecondaryIndex;
    use serde::{Serialize, Deserialize};
    use std::str::FromStr;
    use failure::{Fail, Error};

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
        type Value = <P2PStorage as KeyValueSchema>::Key;

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

    impl SecondaryIndex<P2PStorage> for RemoteAddrIndex {
        type FieldType = SocketAddr;

        fn accessor(value: &<P2PStorage as KeyValueSchema>::Value) -> Option<Self::FieldType> {
            Some(value.remote_addr())
        }

        fn make_index(key: &<P2PStorage as KeyValueSchema>::Key, value: Self::FieldType) -> <Self as KeyValueSchema>::Key {
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
        type Key = TypeKey;
        type Value = <P2PStorage as KeyValueSchema>::Key;

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

    impl SecondaryIndex<P2PStorage> for TypeIndex {
        type FieldType = u32;

        fn accessor(value: &<P2PStorage as KeyValueSchema>::Value) -> Option<Self::FieldType> {
            Some(Type::extract(value))
        }

        fn make_index(key: &<P2PStorage as KeyValueSchema>::Key, value: Self::FieldType) -> <Self as KeyValueSchema>::Key {
            TypeKey::new(value, key.clone())
        }

        fn make_prefix_index(value: Self::FieldType) -> <Self as KeyValueSchema>::Key {
            TypeKey::prefix(value)
        }
    }

    #[derive(Debug, Copy, Clone, Serialize, Deserialize)]
    pub struct TypeKey {
        pub r#type: u32,
        pub index: u64,
    }

    impl TypeKey {
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

    impl BincodeEncoded for TypeKey {}

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
        type Key = RequestKey;
        type Value = <P2PStorage as KeyValueSchema>::Key;

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

    impl SecondaryIndex<P2PStorage> for RequestTrackingIndex {
        type FieldType = u64;

        fn accessor(value: &<P2PStorage as KeyValueSchema>::Value) -> Option<Self::FieldType> {
            match value {
                StoreMessage::P2PMessage { request_id, .. } => request_id.clone(),
                _ => None
            }
        }

        fn make_index(key: &<P2PStorage as KeyValueSchema>::Key, value: Self::FieldType) -> <Self as KeyValueSchema>::Key {
            RequestKey::new(key.clone(), value)
        }

        fn make_prefix_index(value: Self::FieldType) -> <Self as KeyValueSchema>::Key {
            RequestKey::prefix(value)
        }
    }

    #[derive(Debug, Copy, Clone, Serialize, Deserialize)]
    pub struct RequestKey {
        pub request_index: u64,
        pub index: u64,
    }

    impl RequestKey {
        pub fn new(request_index: u64, index: u64) -> Self {
            Self { request_index, index: std::u64::MAX.saturating_sub(index) }
        }

        pub fn prefix(request_index: u64) -> Self {
            Self { request_index, index: 0 }
        }
    }

    impl BincodeEncoded for RequestKey {}

    // 4. Incoming Index
    pub type IncomingIndexKV = dyn KeyValueStoreWithSchema<IncomingIndex> + Sync + Send;

    #[derive(Clone)]
    pub struct IncomingIndex {
        kv: Arc<IncomingIndexKV>,
    }

    impl IncomingIndex {
        pub fn new(kv: Arc<DB>) -> Self {
            Self { kv }
        }
    }

    impl AsRef<(dyn KeyValueStoreWithSchema<IncomingIndex> + 'static)> for IncomingIndex {
        fn as_ref(&self) -> &(dyn KeyValueStoreWithSchema<IncomingIndex> + 'static) {
            self.kv.as_ref()
        }
    }

    impl KeyValueSchema for IncomingIndex {
        type Key = IncomingKey;
        type Value = <P2PStorage as KeyValueSchema>::Key;

        fn descriptor() -> ColumnFamilyDescriptor {
            let mut cf_opts = Options::default();
            cf_opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(std::mem::size_of::<bool>()));
            cf_opts.set_memtable_prefix_bloom_ratio(0.2);
            ColumnFamilyDescriptor::new(Self::name(), cf_opts)
        }

        fn name() -> &'static str {
            "p2p_incoming_index"
        }
    }

    impl SecondaryIndex<P2PStorage> for IncomingIndex {
        type FieldType = bool;

        fn accessor(value: &<P2PStorage as KeyValueSchema>::Value) -> Option<Self::FieldType> {
            Some(value.is_incoming())
        }

        fn make_index(key: &<P2PStorage as KeyValueSchema>::Key, value: Self::FieldType) -> IncomingKey {
            IncomingKey::new(value, key.clone())
        }

        fn make_prefix_index(value: Self::FieldType) -> IncomingKey {
            IncomingKey::prefix(value)
        }
    }

    #[derive(Debug, Copy, Clone, Serialize, Deserialize)]
    pub struct IncomingKey {
        pub is_incoming: bool,
        pub index: u64,
    }

    impl IncomingKey {
        pub fn new(is_incoming: bool, index: u64) -> Self {
            Self { is_incoming, index: std::u64::MAX.saturating_sub(index) }
        }

        pub fn prefix(is_incoming: bool) -> Self {
            Self { is_incoming, index: 0 }
        }
    }

    impl BincodeEncoded for IncomingKey {}
}

