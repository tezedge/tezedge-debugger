// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

#[cfg(all(not(target_env = "msvc"), feature = "jemallocator"))]
#[global_allocator]
static GLOBAL: jemallocator::Jemalloc = jemallocator::Jemalloc;

use std::{fs, io::Read, sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}}};
use tezedge_debugger::{
    system::{DebuggerConfig, syslog_producer},
    storage_::{P2pStoreClient, p2p, LogStoreClient, log, SecondaryIndices, remote::DbClient},
};
use tezedge_debugger::system::Reporter;

/// Create new message store, from well defined path
fn open_database(config: &DebuggerConfig) -> Result<(P2pStoreClient, LogStoreClient), failure::Error> {
    let client = Arc::new(DbClient::connect("/tmp/debugger_db.sock")?);
    Ok((
        P2pStoreClient::new(&client, p2p::Indices::new(&client), config.p2p_message_limit),
        LogStoreClient::new(&client, log::Indices::new(&client), config.log_message_limit),
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
    let (p2p_db, log_db) = open_database(&config)?;

    //let running = Arc::new(AtomicBool::new(true));

    // Create syslog server for each node to capture logs from docker / syslogs
    //for node_config in &config.nodes {
        // TODO: spawn a single server for all nodes
    //    syslog_producer::spawn(&log_db, node_config, running.clone());
    //}

    // Create and spawn bpf sniffing system
    let reporter = Arc::new(Mutex::new(Reporter::new()));
    reporter.lock().unwrap().spawn_parser(p2p_db, &config);

    // Wait for SIGTERM signal
    tokio::signal::ctrl_c().await?;

    tracing::info!("ctrl-c received");
    //running.store(false, Ordering::Relaxed);
    reporter.lock().unwrap().terminate().await;

    Ok(())
}
