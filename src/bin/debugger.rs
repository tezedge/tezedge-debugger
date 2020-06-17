use tracing::{info, error, Level};
use tezedge_debugger::{
    system::build_raw_socket_system,
    utility::{
        identity::Identity,
        ip_settings::get_local_ip,
    },
};
use std::process::exit;
use tezedge_debugger::system::SystemSettings;
use std::time::Instant;
use tezedge_debugger::storage::{MessageStore, get_ts, cfs};
use std::path::Path;
use std::sync::Arc;
use storage::persistent::open_kv;

fn open_database() -> Result<MessageStore, failure::Error> {
    let storage_path = format!("/tmp/volume/{}", get_ts());
    let path = Path::new(&storage_path);
    let schemas = cfs();
    let rocksdb = Arc::new(open_kv(path, schemas)?);
    Ok(MessageStore::new(rocksdb))
}

#[tokio::main]
async fn main() -> Result<(), failure::Error> {
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    let local_address = if let Some(ip_addr) = get_local_ip() {
        ip_addr
    } else {
        error!("failed to detect local ip address");
        exit(1);
    };

    info!(ip_address = display(&local_address), "detected local IP address");

    // Wait until identity appears
    let mut last_try = Instant::now();
    let identity = loop {
        let file = tokio::fs::read_to_string("/tmp/volume/identity.json").await;
        match file {
            Ok(content) => {
                match serde_json::from_str::<Identity>(&content) {
                    Ok(identity) => break identity,
                    Err(err) => {
                        error!(error = display(err), "identity file does not contains valid identity");
                        exit(1);
                    }
                }
            }
            Err(err) => {
                if last_try.elapsed().as_secs() >= 5 {
                    last_try = Instant::now();
                    info!(error = display(err), "waiting for identity");
                }
            }
        }
    };

    info!(peer_id = display(&identity.peer_id), "loaded identity");

    let storage = match open_database() {
        Ok(storage) => storage,
        Err(err) => {
            error!(error = display(err), "failed to open database");
            exit(1);
        }
    };

    match build_raw_socket_system(SystemSettings { identity, local_address, storage: storage.clone() }) {
        Ok(_) => {
            info!("system built");
        }
        Err(err) => {
            error!(error = display(err), "failed to build system");
            exit(1);
        }
    }

    tokio::spawn(async move {
        use tezedge_debugger::server::endpoints::routes;
        info!("started server on port 13030");
        warp::serve(routes(storage))
            .run(([127, 0, 0, 1], 13030))
            .await;
    });

    if let Err(err) = tokio::signal::ctrl_c().await {
        error!(error = display(err), "failed while listening for signal");
        exit(1)
    }

    info!("ctrl-c received");

    Ok(())
}