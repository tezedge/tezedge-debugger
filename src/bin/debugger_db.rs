// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

#[cfg(all(not(target_env = "msvc"), feature = "jemallocator"))]
#[global_allocator]
static GLOBAL: jemallocator::Jemalloc = jemallocator::Jemalloc;

use std::{fs, io::Read, path::Path, sync::{Arc, Mutex}};
use futures::FutureExt;
use rocksdb::Cache;
use storage::persistent::{open_kv, DbConfiguration};
use tokio::sync::oneshot;
use tezedge_debugger::{
    system::{DebuggerConfig, Reporter},
    endpoints::routes,
    storage_::{P2pStore, p2p, LogStore, log, PerfStore, SecondaryIndices, remote::{DbServer, ColumnFamilyDescriptorExt}, local::LocalDb},
};

/// Create new message store, from well defined path
fn open_database(config: &DebuggerConfig) -> Result<(DbServer, P2pStore, LogStore, PerfStore), failure::Error> {
    let path = Path::new(&config.db_path);
    if path.exists() && !config.keep_db {
        let _ = fs::remove_dir_all(path);
    }
    let cache = Cache::new_lru_cache(1)?;
    let mut cf_dictionary = Vec::new();
    for ColumnFamilyDescriptorExt { short_id, name } in P2pStore::schemas_ext().chain(LogStore::schemas_ext()).chain(PerfStore::schemas_ext()) {
        let short_id = short_id as usize;
        if cf_dictionary.len() < short_id + 1 {
            cf_dictionary.resize(short_id + 1, "");
        }
        cf_dictionary[short_id] = name;
    }
    let schemas = P2pStore::schemas(&cache).chain(LogStore::schemas(&cache)).chain(PerfStore::schemas(&cache));
    let rocksdb = Arc::new(LocalDb::new(open_kv(&path, schemas, &DbConfiguration::default())?));
    Ok((
        DbServer::bind("/tmp/debugger_db.sock", &rocksdb, cf_dictionary)?,
        P2pStore::new(&rocksdb, p2p::Indices::new(&rocksdb), config.p2p_message_limit),
        LogStore::new(&rocksdb, log::Indices::new(&rocksdb), config.log_message_limit),
        PerfStore::new(&rocksdb, SecondaryIndices::new(&rocksdb), u64::MAX),
    ))
}

fn load_config() -> Result<DebuggerConfig, failure::Error> {
    let mut settings_file = fs::File::open("config.toml")?;
    let mut settings_toml = String::new();
    settings_file.read_to_string(&mut settings_toml)?;
    toml::from_str(&settings_toml).map_err(|e| failure::Error::from_boxed_compat(Box::new(e)))
}

#[tokio::main]
async fn main() -> Result<(), failure::Error> {
    // Initialize tracing default tracing console subscriber
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // Load system config to drive the rest of the system
    let config = load_config()?;

    // Initialize storage for messages
    let (db_server, p2p_db, log_db, perf_db) = open_database(&config)?;

    let (t_tx, t_rx) = oneshot::channel();
    let db_server_handle = db_server.spawn(t_rx.map(|r| r.unwrap()));
    let reporter = Arc::new(Mutex::new(Reporter::new()));
    tokio::spawn(warp::serve(routes(p2p_db, log_db, perf_db, reporter)).run(([0, 0, 0, 0], config.rpc_port)));

    // Wait for SIGTERM signal
    tokio::signal::ctrl_c().await?;
    t_tx.send(()).unwrap();
    let () = db_server_handle.await?;

    Ok(())
}
