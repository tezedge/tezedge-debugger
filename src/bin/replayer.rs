// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use tracing::{info, error, Level, field::{display, debug}};
use tezedge_debugger::{
    system::build_raw_socket_system,
    system::replayer::replay,
    utility::{
        identity::Identity,
        ip_settings::get_local_ip,
    },
};
use std::process::exit;
use tezedge_debugger::system::SystemSettings;
use std::time::Instant;
use tezedge_debugger::storage::{MessageStore, get_ts, cfs, P2pFilters};
use std::path::Path;
use std::sync::Arc;
use storage::persistent::open_kv;
use tezedge_debugger::system::syslog_producer::syslog_producer;
use itertools::Itertools;
use storage::IteratorMode;
use std::net::SocketAddr;

async fn load_identity() -> Identity {
    // Wait until identity appears
    let mut last_try = Instant::now();

    let identity_paths = [
        "/tmp/volume/identity.json",
        "/tmp/volume/data/identity.json",
    ];

    loop {
        for (path, file) in identity_paths
            .iter().map(|path| (path, tokio::fs::read_to_string(path)))
        {
            match file.await {
                Ok(content) => {
                    match serde_json::from_str::<Identity>(&content) {
                        Ok(identity) => {
                            info!(file_path = display(&path), "loaded identity");
                            return identity;
                        }
                        Err(err) => {
                            error!(error = display(&err), "identity file does not contains valid identity");
                            exit(1);
                        }
                    }
                }
                Err(err) => {
                    if last_try.elapsed().as_secs() >= 5 {
                        last_try = Instant::now();
                        info!(error = display(&err), "waiting for identity");
                    }
                }
            }
        }
    }
}

fn open_snapshot<P: AsRef<Path>>(path: P) -> Result<MessageStore, failure::Error> {
    let schemas = cfs();
    let db = Arc::new(open_kv(path, schemas)?);
    Ok(MessageStore::new(db))
}

#[tokio::main]
async fn main() -> Result<(), failure::Error> {
    tracing_subscriber::fmt()
        .with_max_level(Level::TRACE)
        .init();
    let path = std::env::var("SNAPSHOT_PATH")?;
    let addr: SocketAddr = std::env::var("NODE_IP")?.parse()?;
    let storage = open_snapshot(path)?;
    let msgs = storage.p2p().get_cursor(None, 100, Default::default())?;
    println!("Socket addr: {}", addr);
    let msgs = msgs.into_iter().map(|msg| msg.message).flatten().collect_vec();
    println!("{:?}", msgs);

    replay(addr, msgs, true).await;

    Ok(())
}