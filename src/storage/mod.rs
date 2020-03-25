use rocksdb::DB;
use std::sync::{Arc, atomic::{AtomicU64, Ordering}};
use failure::Error;
use std::net::Ipv4Addr;
use tezos_messages::p2p::encoding::peer::{PeerMessage, PeerMessageResponse};
use serde::{Serialize, Deserialize};
use pnet::packet::Packet as _;
use tezos_messages::p2p::encoding::connection::ConnectionMessage;
use crate::actors::packet_orchestrator::Packet;

#[derive(Clone, Debug)]
pub struct MessageStore {
    db: Arc<DB>,
    counter: Arc<AtomicU64>,
}

/// Types of messages stored in database
#[derive(Debug, Serialize, Deserialize)]
pub enum StoreMessage {
    /// Raw Tcp message, part of tcp connection handling.
    /// Not part of tezos node communication, but internet working.
    TcpMessage {
        source: Ipv4Addr,
        destination: Ipv4Addr,
        packet: Vec<u8>,
    },

    /// Unencrypted message, which is part of tezos communication handshake
    ConnectionMessage {
        source: Ipv4Addr,
        destination: Ipv4Addr,
        payload: ConnectionMessage,
    },

    /// Actual deciphered P2P message sent by some tezos node
    P2PMessage {
        source: Ipv4Addr,
        destination: Ipv4Addr,
        payload: Vec<PeerMessage>,
    },
}

impl StoreMessage {
    pub fn new_conn(source: Ipv4Addr, destination: Ipv4Addr, msg: &ConnectionMessage) -> Self {
        let c = bincode::serialize(msg).unwrap();
        let payload = bincode::deserialize(&c).unwrap();
        Self::ConnectionMessage {
            source,
            destination,
            payload,
        }
    }

    pub fn new_peer(source: Ipv4Addr, destination: Ipv4Addr, msg: &PeerMessageResponse) -> Self {
        let c = bincode::serialize(msg.messages()).unwrap();
        let payload = bincode::deserialize(&c).unwrap();
        Self::P2PMessage {
            source,
            destination,
            payload,
        }
    }
}

impl From<Packet> for StoreMessage {
    fn from(packet: Packet) -> Self {
        let packet = packet.packet();
        Self::TcpMessage {
            source: packet.get_source(),
            destination: packet.get_destination(),
            packet: packet.packet().to_vec(),
        }
    }
}

impl MessageStore {
    pub fn new(db: Arc<DB>) -> Self {
        Self {
            db,
            counter: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn store_message(&mut self, data: StoreMessage) -> Result<(), Error> {
        let content = bincode::serialize(&data)?;
        self.store(&content)
    }

    pub fn store<T: AsRef<[u8]>>(&mut self, data: T) -> Result<(), Error> {
        let id: u64 = (*self.counter).fetch_add(1, Ordering::SeqCst);
        Ok(self.db.put(id.to_ne_bytes(), data)?)
    }

    pub fn get_range(&mut self, start: u64, end: u64) -> Result<Vec<StoreMessage>, Error> {
        if start >= end || start >= self.counter.load(Ordering::SeqCst) {
            Ok(Default::default())
        } else {
            let mut ret = Vec::with_capacity((end - start) as usize);
            for i in start..end {
                let key = i.to_ne_bytes();
                if let Some(x) = self.db.get(&key)? {
                    ret.push(bincode::deserialize(&x)?);
                }
            }
            Ok(ret)
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::Arc;
    use itertools::zip;
    use std::convert::TryInto;

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
            let path = self.0.db.path();
            fs::remove_dir_all(path).expect("failed to delete testing database");
        }
    }

    fn create_test_db<P: AsRef<Path>>(path: P) -> Store {
        Store(MessageStore::new(Arc::new(DB::open_default(path).expect("failed to open database"))))
    }

    #[test]
    fn test_create_db() {
        use std::path::Path;
        let path = function!();
        {
            let db = create_test_db(path);
        }
        assert!(!Path::new(path).exists())
    }

    #[test]
    fn read_range() {
        let mut db = create_test_db(function!());
        for x in 0usize..1000 {
            db.0.store(x.to_ne_bytes());
        }
        let range = db.0.get_range(0, 1000).expect("failed to get range");
        for (db, index) in zip(range, 0usize..1000) {
            let mut bytes = [0u8; 8];
            for i in 0..8 {
                bytes[i] = db[i];
            }
            let val = usize::from_ne_bytes(bytes);
            assert_eq!(val, index)
        }
    }
}
