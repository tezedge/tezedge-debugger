// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

pub mod storage_message;
pub mod rpc_message;
mod p2p_storage;
mod rpc_storage;
mod log_storage;
mod secondary_index;

pub use storage_message::*;
pub use p2p_storage::*;
pub use rpc_storage::*;
pub use log_storage::*;
pub use p2p_storage::secondary_indexes as p2p_secondary_indexes;

use rocksdb::{DB, WriteOptions, ColumnFamilyDescriptor};
use std::{
    path::Path,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
    net::IpAddr,
};
use storage::persistent::KeyValueSchema;
// use std::fs::remove_dir_all;

#[derive(Clone)]
pub struct MessageStore {
    p2p_db: P2PStorage,
    rpc_db: RpcStorage,
    log_db: LogStorage,
    raw_db: Arc<DB>,
    max_db_size: Option<u64>,
}

impl MessageStore {
    pub fn new(db: Arc<DB>) -> Self {
        Self {
            p2p_db: P2PStorage::new(db.clone()),
            rpc_db: RpcStorage::new(db.clone()),
            log_db: LogStorage::new(db.clone()),
            raw_db: db,
            max_db_size: None,
        }
    }

    pub fn rpc(&self) -> &RpcStorage {
        &self.rpc_db
    }

    pub fn p2p(&self) -> &P2PStorage {
        &self.p2p_db
    }

    pub fn log(&self) -> &LogStorage {
        &self.log_db
    }

    pub(crate) fn database_path(&self) -> &Path {
        self.raw_db.path()
    }

    pub(crate) fn database_size(&self) -> std::io::Result<u64> {
        dir_size(self.database_path())
    }
}

pub(crate) fn cfs() -> Vec<ColumnFamilyDescriptor> {
    use crate::storage::p2p_storage::secondary_indexes as p2p_indexes;
    vec![
        RpcStorage::descriptor(),
        P2PStorage::descriptor(),
        LogStorage::descriptor(),
        p2p_indexes::RemoteAddrIndex::descriptor(),
        p2p_indexes::TypeIndex::descriptor(),
        p2p_indexes::RequestTrackingIndex::descriptor(),
        p2p_indexes::IncomingIndex::descriptor(),
    ]
}

pub(crate) fn default_write_options() -> WriteOptions {
    let mut opts = WriteOptions::default();
    opts.set_sync(false);
    opts
}

pub(crate) fn dir_size<P: AsRef<Path>>(path: P) -> std::io::Result<u64> {
    fn dir_size(mut dir: std::fs::ReadDir) -> std::io::Result<u64> {
        dir.try_fold(0, |acc, file| {
            let file = file?;
            let size = match file.metadata()? {
                data if data.is_dir() => dir_size(std::fs::read_dir(file.path())?)?,
                data => data.len(),
            };
            Ok(acc + size)
        })
    }

    dir_size(std::fs::read_dir(path)?)
}

pub fn get_ts() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
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

pub fn dissect(mut number: u32) -> Vec<u32> {
    let mut ret: Vec<u32> = Vec::with_capacity(number.count_ones() as usize);
    while number != 0 {
        let value = 0x1 << number.trailing_zeros();
        number = !value & number;
        ret.push(value);
    }
    ret
}

pub mod sorted_intersect {
    use std::cmp::Ordering;

    pub fn sorted_intersect<I>(mut iters: Vec<I>, limit: usize) -> Vec<I::Item>
        where
            I: Iterator,
            I::Item: Ord,
    {
        let mut ret = Default::default();
        let mut heap = Vec::with_capacity(iters.len());
        // Fill the heap with values
        if !fill_heap(iters.iter_mut(), &mut heap) {
            // Hit an exhausted iterator, finish
            return ret;
        }

        while ret.len() < limit {
            if is_hit(&heap) {
                // We hit intersected item
                if let Some((item, _)) = heap.pop() {
                    // Push it into the intersect values
                    ret.push(item);
                    // Clear the rest of the heap
                    heap.clear();
                    // Build a new heap from new values
                    if !fill_heap(iters.iter_mut(), &mut heap) {
                        // Hit an exhausted iterator, finish
                        return ret;
                    }
                } else {
                    // Hit an exhausted iterator, finish
                    return ret;
                }
            } else {
                // Remove max element from the heap
                if let Some((_, iter_num)) = heap.pop() {
                    if let Some(item) = iters[iter_num].next() {
                        // Insert replacement from the corresponding iterator to heap
                        heap.push((item, iter_num));
                        heapify(&mut heap);
                    } else {
                        // Hit an exhausted iterator, finish
                        return ret;
                    }
                } else {
                    // Hit an exhausted iterator, finish
                    return ret;
                }
            }
        }

        ret
    }

    fn heapify<Item: Ord>(heap: &mut Vec<(Item, usize)>) {
        heap.sort_by(|(a, _), (b, _)| a.cmp(b));
    }

    fn fill_heap<'a, Item: Ord, Inner: 'a + Iterator<Item=Item>, Outer: Iterator<Item=&'a mut Inner>>(iters: Outer, heap: &mut Vec<(Inner::Item, usize)>) -> bool {
        for (i, iter) in iters.enumerate() {
            let value = iter.next();
            if let Some(value) = value {
                heap.push((value, i))
            } else {
                return false;
            }
        }
        heapify(heap);
        true
    }

    fn is_hit<Item: Ord>(heap: &Vec<(Item, usize)>) -> bool {
        let value = heap.iter().next().map(|(value, _)|
            heap.iter().fold((value, true), |(a, eq), (b, _)| {
                (b, eq & (a.cmp(b) == Ordering::Equal))
            })
        );

        if let Some((_, true)) = value {
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::Arc;
    use std::ops::Deref;
    use std::ops::DerefMut;
    use storage::persistent::open_kv;
    use crate::storage::storage_message::RESTMessage;
    use crate::storage::rpc_message::MappedRESTMessage;
    use crate::network::connection_message::ConnectionMessage;
    use tezos_messages::p2p::encoding::metadata::MetadataMessage;
    use crate::actors::logs_message::LogMessage;

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

    impl Deref for Store {
        type Target = MessageStore;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl DerefMut for Store {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.0
        }
    }

    fn create_test_db<P: AsRef<Path>>(path: P) -> Store {
        let schemas = cfs();
        Store(MessageStore::new(Arc::new(
            open_kv(path, schemas)
                .expect("failed to open database")
        )))
    }

    #[test]
    fn clean_test_db() {
        let path = function!();
        let db = create_test_db(path);
        assert!(Path::new(path).exists());
        drop(db);
        assert!(!Path::new(path).exists());
    }

    #[test]
    fn p2p_read_reverse_range() {
        let mut db = create_test_db(function!());
        let sock: SocketAddr = "0.0.0.0:1010".parse().unwrap();
        for x in 0usize..10 {
            let res = db.store_p2p_message(
                &StoreMessage::new_rest(sock, true, RESTMessage::Response {
                    status: "200".to_string(),
                    payload: format!("{}", x),
                })
            );
            assert!(res.is_ok());
        }

        let msgs = db.get_p2p_reverse_range(0, 10).unwrap();
        println!("{:?}", msgs);

        assert_eq!(msgs.len(), 10);
        for (id, msg) in msgs.iter().enumerate() {
            let id = 9 - id;
            match msg {
                RpcMessage::RestMessage { message, .. } => {
                    match message {
                        MappedRESTMessage::Response { payload, .. } => {
                            assert_eq!(payload, &format!("{}", id));
                        }
                        msg => panic!("Expected response got {:?}", msg)
                    }
                }
                msg => panic!("Expected rest message got: {:?}", msg),
            }
        }
    }

    #[test]
    fn p2p_get_types() {
        use crate::storage::p2p_storage::secondary_indexes::Type;
        let mut db = create_test_db(function!());
        let sock: SocketAddr = "0.0.0.0:1010".parse().unwrap();
        // Insert data
        let mut type_a = StoreMessage::new_connection(sock, true, &ConnectionMessage::new(0, "", "", &[], Default::default()));
        let res = db.store_p2p_message(&type_a);
        assert!(res.is_ok(), "Failed to store message: {:?}", res);
        type_a = StoreMessage::new_connection(sock, true, &ConnectionMessage::new(1, "", "", &[], Default::default()));
        let res = db.store_p2p_message(&type_a);
        assert!(res.is_ok(), "Failed to store message: {:?}", res);
        let type_b = StoreMessage::new_metadata(sock, true, MetadataMessage::new(false, false));
        let res = db.store_p2p_message(&type_b);
        assert!(res.is_ok(), "Failed to store message: {:?}", res);

        // Load simple data
        let msgs = db.p2p_db.get_types_range(Type::ConnectionMessage as u32, 0, 10).unwrap();
        assert_eq!(msgs.len(), 2);
        let msg = &msgs[0];
        assert_eq!(msg.id(), 1);
        let msg = &msgs[1];
        assert_eq!(msg.id(), 0);
        // Load multiple data
        let types = Type::ConnectionMessage as u32 | Type::Metadata as u32;
        let msgs = db.p2p_db.get_types_range(types, 0, 10).unwrap();
        assert_eq!(msgs.len(), 3);
        let msg = &msgs[0];
        assert_eq!(msg.id(), 2, "Expected index 2");
        let msg = &msgs[1];
        assert_eq!(msg.id(), 1, "Expected index 1");
        let msg = &msgs[2];
        assert_eq!(msg.id(), 0, "Expected index 0");
    }

    #[test]
    fn log_read_range() {
        let mut db = create_test_db(function!());
        let mut msg = LogMessage {
            level: "notice".to_string(),
            date: 0,
            section: "node.validator.bootstrap_pipeline".to_string(),
            id: None,
            file: None,
            line: None,
            column: None,
            extra: Default::default(),
        };

        for x in 0usize..10 {
            msg.extra.insert("message".to_string(), format!("{}", x));
            let res = db.log().store_message(&mut msg);
            if res.is_err() {
                assert!(false, "failed to store message: {}", res.unwrap_err())
            }
        }

        let msgs = db.log().get_reverse_range(0, 10).unwrap();
        assert_eq!(msgs.len(), 10);
        for (msg, id) in msgs.into_iter().zip((0..10).rev()) {
            let val = msg.extra.get("message").unwrap();
            assert_eq!(val, &format!("{}", id));
        }
    }

    #[test]
    fn log_read_ts() {
        let mut db = create_test_db(function!());
        let mut msg = LogMessage {
            level: "notice".to_string(),
            date: 0,
            section: "node.validator.bootstrap_pipeline".to_string(),
            id: None,
            file: None,
            line: None,
            column: None,
            extra: Default::default(),
        };

        for x in 0usize..10 {
            msg.extra.insert("message".to_string(), format!("{}", x));
            msg.date = x as u128;
            db.log().store_message(&mut msg).unwrap();
        }


        let msgs = db.log().get_timestamp_range(4, 10).unwrap();
        println!("{:?}", msgs);
        assert_eq!(msgs.len(), 5);
        for (msg, id) in msgs.into_iter().zip((0u128..5).rev()) {
            let val = msg.date;
            assert_eq!(val, id)
        }
    }

    fn log_read_ts_level() {
        let mut db = create_test_db(function!());
        let mut msg = LogMessage {
            level: "notice".to_string(),
            date: 0,
            section: "node.validator.bootstrap_pipeline".to_string(),
            id: None,
            file: None,
            line: None,
            column: None,
            extra: Default::default(),
        };

        for i in 0u128..5 {
            msg.date = i;
            db.log().store_message(&mut msg).unwrap();
        }

        msg.level = "warn".to_string();
        for i in 0u128..5 {
            msg.date = i;
            db.log().store_message(&mut msg).unwrap();
        }

        let msgs = db.log().get_timestamp_level_range("notice", 0, 10).unwrap();
        assert_eq!(msgs.len(), 5);
        for (msg, index) in msgs.into_iter().zip((0u128..5).rev()) {
            assert_eq!(msg.date, index)
        }

        let msgs = db.log().get_timestamp_level_range("warning", 0, 10).unwrap();
        assert_eq!(msgs.len(), 5);
        for (msg, index) in msgs.into_iter().zip((0u128..5).rev()) {
            assert_eq!(msg.date, index)
        }
    }

    #[test]
    fn rpc_read_range() {
        let mut db = create_test_db(function!());
        let sock: SocketAddr = "0.0.0.0:1010".parse().unwrap();
        for x in 0usize..10 {
            let ret = db.store_rpc_message(
                &StoreMessage::new_rest(sock, true, RESTMessage::Response {
                    status: "200".to_string(),
                    payload: format!("{}", x),
                })
            );
            if ret.is_err() {
                assert!(false, "failed to store message: {}", ret.unwrap_err())
            }
        }
        let msgs = db.get_rpc_range(0, 10).unwrap();
        assert_eq!(msgs.len(), 10);
        for (msg, idx) in msgs.iter().zip((0..=9).rev()) {
            match msg {
                RpcMessage::RestMessage { message, .. } => {
                    match message {
                        MappedRESTMessage::Response { payload, .. } => {
                            assert_eq!(payload, &format!("{}", idx));
                        }
                        _ => assert!(false, "Expected response message")
                    }
                }
                _ => assert!(false, "Expected rest message")
            }
        }
    }

    #[test]
    fn rpc_read_range_host() {
        let mut db = create_test_db(function!());
        let sock: SocketAddr = "0.0.0.0:1010".parse().unwrap();
        for x in 0usize..10 {
            let ret = db.store_rpc_message(
                &StoreMessage::new_rest(sock, true, RESTMessage::Response {
                    status: "200".to_string(),
                    payload: format!("{}", x),
                }));
            if ret.is_err() {
                assert!(false, "failed to store message: {}", ret.unwrap_err())
            }
        }
        let msgs = db.get_rpc_host_range(5, 10, "0.0.0.0".parse().unwrap()).unwrap();
        assert_eq!(msgs.len(), 5);
        for (msg, idx) in msgs.iter().zip(9..=5) {
            match msg {
                RpcMessage::RestMessage { message, .. } => {
                    match message {
                        MappedRESTMessage::Response { payload, .. } => {
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