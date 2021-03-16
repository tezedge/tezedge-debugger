use std::{sync::Arc, net::SocketAddr, path::Path, iter};
use rocksdb::Cache;
use storage::persistent::{DbConfiguration, open_kv};
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId, black_box};
use tezedge_debugger::storage_::{P2pStore, p2p, indices, StoreCollector, local::LocalDb, SecondaryIndices};

fn p2p<P: AsRef<Path>>(path: P) -> P2pStore {
    //use std::fs;

    //let _ = fs::remove_dir_all(&path);
    let cache = Cache::new_lru_cache(1).unwrap();
    let schemas = P2pStore::schemas(&cache);
    let rocksdb = Arc::new(LocalDb::new(open_kv(&path, schemas, &DbConfiguration::default()).unwrap()));
    P2pStore::new(&rocksdb, p2p::Indices::new(&rocksdb), u64::MAX)
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
                db.store_message(&message).unwrap();
            }
        }
    }
}

fn filters(db: &P2pStore, node_id: u16, remote_port: u16) {
    let filters = p2p::Filters {
        node_name: Some(indices::NodeName(128 + node_id)),
        remote_addr: Some(SocketAddr::new("127.0.0.1".parse().unwrap(), 9732 + remote_port)),
        initiator: None,
        sender: None,
        types: vec![],
    };

    let messages_all_such = db.get_cursor(None, 128, &filters).unwrap();
    let messages_limit = db.get_cursor(None, 64, &filters).unwrap();

    assert_eq!(messages_all_such.len(), 128);
    assert_eq!(messages_limit.len(), 64);

    for (a, b) in messages_limit.iter().zip(messages_all_such.iter()) {
        assert_eq!(&a.decrypted_bytes[0..16], &b.decrypted_bytes[0..16]);
    }

    black_box(messages_all_such);
    black_box(messages_limit);
}

fn database(c: &mut Criterion) {
    let mut group = c.benchmark_group("database");

    let db = p2p("target/db_test_big");
    //group.bench_function(BenchmarkId::new("prepare", 0), |b| b.iter(|| prepare_p2p(&db)));
    group.bench_function(BenchmarkId::new("filters", 0), |b| b.iter(|| filters(&db, 4, 7)));

    group.finish()
}

criterion_group!(
    name = benches;
    config = Criterion::default();
    targets = database
);

criterion_main!(benches);
