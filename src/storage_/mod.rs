pub mod p2p;
pub mod indices;

mod sorted_intersect;

mod store;
pub use self::store::Store;

mod secondary_index;
pub use self::secondary_index::SecondaryIndices;

pub type P2pStore = Store<p2p::Message, p2p::Schema, p2p::Indices>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_fetch() {
        use std::sync::Arc;
        use rocksdb::Cache;
        use storage::persistent::{open_kv, DbConfiguration};

        let cache = Cache::new_lru_cache(1).unwrap();
        let schemas = P2pStore::schemas(&cache);
        let rocksdb = Arc::new(open_kv("target/db_new", schemas, &DbConfiguration::default()).unwrap());
        let storage_new = P2pStore::new(&rocksdb);
        let mut message = p2p::Message::new(
            "tezedge".to_string(),
            "127.0.0.1:12345".parse().unwrap(),
            false,
            indices::SourceType::Local,
            vec![1, 2, 3],
            vec![1, 2, 3],
            Err("foo".to_string()),
        );
        storage_new.store_message(&mut message).unwrap();
        let messages = storage_new.get_cursor(None, 1, &p2p::Filters::default()).unwrap();
        println!("{}", serde_json::to_string(&messages[0]).unwrap());
        println!("{}", serde_json::to_string(&message).unwrap());
    }
}
