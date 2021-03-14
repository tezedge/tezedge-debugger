// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

#[cfg(all(not(target_env = "msvc"), feature = "jemallocator"))]
#[global_allocator]
static GLOBAL: jemallocator::Jemalloc = jemallocator::Jemalloc;

use std::{fs, io::Read, path::Path, sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}}};
use rocksdb::Cache;
use storage::persistent::{open_kv, DbConfiguration};
use tezedge_debugger::{
    system::{DebuggerConfig, syslog_producer},
    endpoints::routes,
    storage_::{P2pStore, LogStore},
};
use tezedge_debugger::system::Reporter;

/// Create new message store, from well defined path
fn open_database(config: &DebuggerConfig) -> Result<(P2pStore, LogStore), failure::Error> {
    let path = Path::new(&config.db_path);
    if path.exists() && !config.keep_db {
        let _ = fs::remove_dir_all(path);
    }
    let cache = Cache::new_lru_cache(1)?;
    let schemas = P2pStore::schemas(&cache).chain(LogStore::schemas(&cache));
    let rocksdb = Arc::new(open_kv(&path, schemas, &DbConfiguration::default())?);
    Ok((P2pStore::new(&rocksdb, config.p2p_message_limit), LogStore::new(&rocksdb, config.log_message_limit)))
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
    let (p2p_db, log_db) = open_database(&config)?;

    let running = Arc::new(AtomicBool::new(true));

    // Create syslog server for each node to capture logs from docker / syslogs
    for node_config in &config.nodes {
        syslog_producer::spawn(&log_db, node_config, running.clone());
    }

    // Create and spawn bpf sniffing system
    let reporter = {
        let mut reporter = Reporter::new();
        reporter.spawn_parser(&p2p_db, &config);
        Arc::new(Mutex::new(reporter))
    };
    tokio::spawn(warp::serve(routes(p2p_db, log_db, reporter)).run(([0, 0, 0, 0], config.rpc_port)));

    // Wait for SIGTERM signal
    tokio::signal::ctrl_c().await?;

    tracing::info!("ctrl-c received");
    running.store(false, Ordering::Relaxed);
    // TODO: stop bpf sniffer and p2p parsers

    Ok(())
}
