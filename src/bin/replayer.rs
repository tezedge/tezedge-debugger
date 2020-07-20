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
    let msgs = storage.p2p().get_cursor(Some(6), 7, Default::default())?;
    replay(addr, msgs, true).await;

    Ok(())
}