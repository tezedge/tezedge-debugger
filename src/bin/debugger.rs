// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use tracing::{info, error, Level};
use tezedge_debugger::{
    utility::{
        ip_settings::get_local_ip,
    },
};
use std::process::exit;
use tezedge_debugger::system::SystemSettings;
use tezedge_debugger::storage::{MessageStore, get_ts, cfs};
use std::path::Path;
use std::sync::Arc;
use storage::persistent::{open_kv, DbConfiguration};
use tezedge_debugger::system::{syslog_producer::syslog_producer, BpfSniffer};

/// Create new message store, from well defined path
fn open_database() -> Result<MessageStore, failure::Error> {
    let storage_path = format!("/tmp/volume/{}", get_ts());
    let path = Path::new(&storage_path);
    let schemas = cfs();
    let rocksdb = Arc::new(open_kv(path, schemas, &DbConfiguration::default())?);
    Ok(MessageStore::new(rocksdb))
}

#[tokio::main]
async fn main() -> Result<(), failure::Error> {
    // Initialize tracing default tracing console subscriber
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    // Identify the local address
    let local_address = if let Some(ip_addr) = get_local_ip() {
        ip_addr
    } else {
        error!("failed to detect local ip address");
        exit(1);
    };

    info!(ip_address = tracing::field::display(&local_address), "detected local IP address");

    // Initialize storage for messages
    let storage = match open_database() {
        Ok(storage) => storage,
        Err(err) => {
            error!(error = tracing::field::display(&err), "failed to open database");
            exit(1);
        }
    };

    // Create system setting to drive the rest of the system
    let settings = SystemSettings {
        local_address,
        storage: storage.clone(),
        syslog_port: 13131,
        rpc_port: 17732,
        node_p2p_port: 9732,
        node_rpc_port: 18732,
    };

    // Create syslog server to capture logs from docker / syslogs
    if let Err(err) = syslog_producer(settings.clone()).await {
        error!(error = tracing::field::display(&err), "failed to build syslog server");
        exit(1);
    }

    // Create actual system
    let sniffer = BpfSniffer::spawn(&settings);

    // Spawn warp RPC server
    tokio::spawn(async move {
        use tezedge_debugger::endpoints::routes;
        warp::serve(routes(storage, sniffer))
            .run(([0, 0, 0, 0], settings.rpc_port))
            .await;
    });

    // Wait for SIGTERM signal
    if let Err(err) = tokio::signal::ctrl_c().await {
        error!(error = tracing::field::display(&err), "failed while listening for signal");
        exit(1);
    }

    info!("ctrl-c received");

    Ok(())
}
