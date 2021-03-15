use std::{sync::Arc, net::SocketAddr, path::Path, iter};
use storage::persistent::{DbConfiguration, open_kv};
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId, black_box};
use tezedge_debugger::{storage::{MessageStore, cfs, P2pFilters}, messages::p2p_message::{P2pMessage, SourceType}};

fn p2p<P: AsRef<Path>>(path: P) -> MessageStore {
    //use std::fs;

    //let _ = fs::remove_dir_all(&path);
    let rocksdb = Arc::new(open_kv(&path, cfs(), &DbConfiguration::default()).unwrap());
    MessageStore::new(rocksdb)
}

#[allow(dead_code)]
fn prepare_p2p(db: &MessageStore) {

    let names = 128..(128 + 16);
    let ports = 9732..(9732 + 16);

    for node_name in names {
        for port in ports.clone() {
            let initiator_local = ((node_name % 13) + (port % 17)) % 11 != 0;
            let source_type = if initiator_local {
                SourceType::Local
            } else {
                SourceType::Remote
            };

            let remote_addr = SocketAddr::new("127.0.0.1".parse().unwrap(), port);

            for i in 0..128 {
                let bytes = iter::repeat(0).map(|_| rand::random()).take(128).collect::<Vec<u8>>();
                let mut message = P2pMessage::new(
                    node_name,
                    remote_addr.clone(),
                    i % 2 == 0,
                    source_type.clone(),
                    bytes.clone(),
                    bytes,
                    Err("bar".to_string()),
                );
                db.p2p().store_message(&mut message).unwrap();
            }
        }
    }
}

fn filters(db: &MessageStore, node_id: u16, remote_port: u16) {
    let filters = P2pFilters {
        node_name: Some(128 + node_id),
        remote_addr: Some(SocketAddr::new("127.0.0.1".parse().unwrap(), 9732 + remote_port)),
        request_id: None,
        incoming: None,
        source_type: None,
        types: None,
    };

    let messages_all_such = db.p2p().get_cursor(None, 128, filters.clone()).unwrap();
    let messages_limit = db.p2p().get_cursor(None, 64, filters).unwrap();

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

    let db = p2p("target/db_test_big_old");
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
