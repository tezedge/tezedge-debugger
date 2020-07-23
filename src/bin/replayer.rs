// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use tracing::{Level};
use tezedge_debugger::system::replayer::replay;
use tezedge_debugger::storage::{MessageStore, cfs};
use structopt::StructOpt;
use std::path::Path;
use std::sync::Arc;
use storage::persistent::open_kv;
use std::net::SocketAddr;

#[derive(StructOpt, Debug)]
#[structopt(name = "tezos message replayer")]
/// Commandline arguments
struct Opt {
    #[structopt(short, long, default_value = "/tmp/snapshot/snapshot")]
    /// Path to the snapshot, to be replayed
    pub path: String,
    #[structopt(short, long, default_value = "0.0.0.0:13030")]
    /// Address of the node to replay messages
    pub node_ip: SocketAddr,
    #[structopt(short, long)]
    /// ID of the last message to be replayed
    pub last_message_id: u64,
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
    let opts: Opt = Opt::from_args();
    let path = &opts.path;
    let addr = opts.node_ip;
    let storage = open_snapshot(path)?;
    let msgs = storage.p2p().get_cursor(Some(opts.last_message_id), (opts.last_message_id + 1) as usize, Default::default())?;
    replay(addr, msgs).await
}