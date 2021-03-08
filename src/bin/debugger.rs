// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{fs, io::Read, path::Path, process::exit, sync::Arc};
use tracing::{info, error, Level};
use rocksdb::Cache;
use storage::persistent::{open_kv, DbConfiguration};
use tezedge_debugger::{
    system::{DebuggerConfig, syslog_producer::syslog_producer, Parser},
    endpoints::routes,
    storage_::{P2pStore, LogStore},
};

/// Create new message store, from well defined path
fn open_database(config: &DebuggerConfig) -> Result<(P2pStore, LogStore), failure::Error> {
    let path = Path::new(&config.db_path);
    if path.exists() && !config.keep_db {
        let _ = fs::remove_dir_all(path);
    }
    let cache = Cache::new_lru_cache(1)?;
    let schemas = P2pStore::schemas(&cache).chain(LogStore::schemas(&cache));
    let rocksdb = Arc::new(open_kv(&path, schemas, &DbConfiguration::default())?);
    Ok((P2pStore::new(&rocksdb), LogStore::new(&rocksdb)))
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
        .with_max_level(Level::INFO)
        .init();

    // Load system config to drive the rest of the system
    let config = match load_config() {
        Ok(config) => config,
        Err(err) => {
            error!(error = tracing::field::display(&err), "failed to load config");
            exit(1);
        }
    };

    // Initialize storage for messages
    let (p2p_db, log_db) = match open_database(&config) {
        Ok(storage) => storage,
        Err(err) => {
            error!(error = tracing::field::display(&err), "failed to open database");
            exit(1);
        }
    };

    // Create syslog server for each node to capture logs from docker / syslogs
    for node_config in &config.nodes {
        if let Err(err) = syslog_producer(&log_db, node_config).await {
            error!(error = tracing::field::display(&err), "failed to build syslog server");
            exit(1);
        }
    }

    // Create and spawn bpf sniffing system
    let reporter = Parser::new(&p2p_db, &config).spawn();

    // Spawn warp RPC server
    tokio::spawn(warp::serve(routes(p2p_db, log_db, reporter)).run(([0, 0, 0, 0], config.rpc_port)));

    // Wait for SIGTERM signal
    if let Err(err) = tokio::signal::ctrl_c().await {
        error!(error = tracing::field::display(&err), "failed while listening for signal");
        exit(1);
    }

    info!("ctrl-c received");

    Ok(())
}
