// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use tracing::{error, Level};
use storage::persistent::{open_kv, DbConfiguration};
use std::{sync::Arc, path::Path, process::exit};
use tezedge_debugger::storage::{MessageStore, get_ts, cfs};

/// Create new message store, from well defined path
fn open_database() -> Result<MessageStore, failure::Error> {
    let storage_path = format!("/tmp/volume/{}", get_ts());
    let path = Path::new(&storage_path);
    let schemas = cfs();
    let rocksdb = Arc::new(open_kv(path, schemas, &DbConfiguration::default())?);
    Ok(MessageStore::new(rocksdb))
}

fn main() -> Result<(), failure::Error> {
    // Initialize tracing default tracing console subscriber
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    // Initialize storage for messages
    let _storage = match open_database() {
        Ok(storage) => storage,
        Err(err) => {
            error!(error = tracing::field::display(&err), "failed to open database");
            exit(1);
        }
    };

    Ok(())
}
