// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

pub mod storage_message;
pub mod rpc_message;
mod p2p_storage;
mod rpc_storage;

pub use storage_message::*;
pub use p2p_storage::*;
pub use rpc_storage::*;

use rocksdb::{DB, WriteOptions};
use failure::Error;
use lazy_static::lazy_static;
use std::{
    path::Path,
    sync::{Arc, atomic::AtomicU64},
    time::{SystemTime, UNIX_EPOCH},
    net::{SocketAddr, IpAddr},
};
use crate::storage::rpc_message::RpcMessage;

#[derive(Clone)]
pub struct MessageStore {
    p2p_db: P2PMessageStorage,
    rpc_db: RpcMessageStorage,
    raw_db: Arc<DB>,
    max_db_size: Option<u64>,
}

impl MessageStore {
    pub fn new(db: Arc<DB>) -> Self {
        Self {
            p2p_db: P2PMessageStorage::new(db.clone()),
            rpc_db: RpcMessageStorage::new(db.clone()),
            raw_db: db,
            max_db_size: None,
        }
    }

    pub fn store_p2p_message(&mut self, data: &StoreMessage) -> Result<(), Error> {
        let ret = self.p2p_db.store_message(&data)
            .map_err(|e| e.into());
        ret
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

    pub fn get_rpc_host_range(&mut self, offset: u64, count: u64, host: IpAddr) -> Result<Vec<RpcMessage>, Error> {
        Ok(self.rpc_db.get_host_range(offset, count, host)?)
    }

    pub(crate) fn database_path(&self) -> &Path {
        self.raw_db.path()
    }

    pub(crate) fn database_size(&self) -> std::io::Result<u64> {
        dir_size(self.database_path())
    }

    pub(crate) fn reduce_db(&mut self) -> Result<(), Error> {
        // 1. Purge P2P storage
        self.p2p_db.reduce_db()?;
        Ok(())
    }
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

lazy_static! {
    static ref RPC_COUNT: Arc<AtomicU64> = Arc::new(AtomicU64::new(0));
    static ref RPC_SEQ: Arc<AtomicU64> = Arc::new(AtomicU64::new(std::u64::MAX));
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

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::Arc;
    use std::ops::Deref;
    use std::ops::DerefMut;
    use storage::persistent::{open_kv, KeyValueSchema};
    use crate::storage::storage_message::RESTMessage;
    use crate::storage::rpc_message::MappedRESTMessage;

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
        let schemas = vec![
            crate::storage::RpcMessageStorage::descriptor(),
            crate::storage::P2PMessageStorage::descriptor(),
            crate::storage::RpcMessageSecondaryIndex::descriptor(),
            crate::storage::P2PMessageSecondaryIndex::descriptor(),
        ];
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
        for (msg, idx) in msgs.iter().zip(9..=0) {
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