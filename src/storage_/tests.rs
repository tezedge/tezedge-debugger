use std::{sync::Arc, net::SocketAddr, iter, path::Path, fs};
use rocksdb::Cache;
use storage::persistent::{open_kv, DbConfiguration};
use super::{P2pStore, p2p, indices, StoreCollector, local::LocalDb, SecondaryIndices};

fn p2p<P: AsRef<Path>>(path: P) -> P2pStore {
    let _ = fs::remove_dir_all(&path);
    let cache = Cache::new_lru_cache(1).unwrap();
    let schemas = P2pStore::schemas(&cache);
    let rocksdb = Arc::new(LocalDb::new(open_kv(&path, schemas, &DbConfiguration::default()).unwrap()));
    P2pStore::new(&rocksdb, p2p::Indices::new(&rocksdb), u64::MAX)
}

#[test]
fn basic_store_fetch() {
    let db = p2p("target/db_test_simple");
    let messages_original = vec![
        p2p::Message::new(
            indices::NodeName(3123),
            "127.0.0.1:12345".parse().unwrap(),
            indices::Initiator::Local,
            indices::Sender::Local,
            vec![1, 2, 3],
            vec![1, 2, 3],
            None,
        ),
    ];
    for message in &messages_original {
        db.store_message(message.clone()).unwrap();
    }
    let messages = db.get_cursor(None, 1024, &p2p::Filters::default()).unwrap();
    println!("{}", serde_json::to_string(&messages_original).unwrap());
    println!("{}", serde_json::to_string(&messages).unwrap());
}

#[allow(dead_code)]
fn prepare_p2p(db: &P2pStore) {
    let names = 128..(128 + 16);
    let ports = 9732..(9732 + 16);

    for node_name in names {
        for port in ports.clone() {
            let initiator_local = ((node_name % 13) + (port % 17)) % 11 != 0;
            let node_name = indices::NodeName(node_name);
            let source_type = if initiator_local {
                indices::Initiator::Local
            } else {
                indices::Initiator::Remote
            };

            let remote_addr = SocketAddr::new("127.0.0.1".parse().unwrap(), port);

            for i in 0..128 {
                let bytes = iter::repeat(0).map(|_| rand::random()).take(128).collect::<Vec<u8>>();
                let message = p2p::Message::new(
                    node_name.clone(),
                    remote_addr.clone(),
                    source_type.clone(),
                    if i % 2 == 0 { indices::Sender::Remote } else { indices::Sender::Local },
                    bytes.clone(),
                    bytes,
                    None,
                );
                db.store_message(message).unwrap();
            }
        }
    }
}

#[test]
fn filters() {
    let db = p2p("target/db_test_big");
    prepare_p2p(&db);

    let mut filters = p2p::Filters {
        node_name: None,
        remote_addr: None,
        initiator: None,
        sender: None,
        types: vec![],
    };
    let messages = db.get_cursor(None, 1024, &filters).unwrap();
    assert_eq!(messages.len(), 1024);

    filters.node_name = Some(indices::NodeName(128 + 4));
    let messages = db.get_cursor(None, 1024, &filters).unwrap();
    assert_eq!(messages.len(), 1024);

    filters.remote_addr = Some(SocketAddr::new("127.0.0.1".parse().unwrap(), 9732 + 7));
    let messages_all_such = db.get_cursor(None, 128, &filters).unwrap();
    let messages_limit = db.get_cursor(None, 64, &filters).unwrap();

    assert_eq!(messages_all_such.len(), 128);
    assert_eq!(messages_limit.len(), 64);

    for (a, b) in messages_limit.iter().zip(messages_all_such.iter()) {
        assert_eq!(&a.decrypted_bytes[0..16], &b.decrypted_bytes[0..16]);
    }
}
