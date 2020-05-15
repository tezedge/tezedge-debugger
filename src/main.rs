#![allow(dead_code)]

mod configuration;
mod actors;
mod network;
mod storage;

use std::{
    net::SocketAddr,
    sync::{Mutex, Arc},
};

use serde::{Serialize, Deserialize};
use failure::Fail;
use main_error::MainError;
use riker::actors::*;
use warp::{
    Filter,
    http::Response,
};

use crate::{
    actors::prelude::*,
    network::prelude::*,
    configuration::AppConfig,
};
use crate::storage::p2p_secondary_indexes::Type;
use std::net::IpAddr;
use crate::actors::logs_orchestrator::make_logs_reader;

#[derive(Debug, Fail)]
enum AppError {
    #[fail(display = "no valid network interface found")]
    NoNetworkInterface,
    #[fail(display = "only ethernet channels supported for now")]
    UnsupportedNetworkChannelType,
    #[fail(display = "encountered io error: {}", _0)]
    IOError(std::io::Error),
    #[fail(display = "received invalid packet")]
    InvalidPacket,
}

#[derive(Default, Debug, Copy, Clone, Serialize, Deserialize)]
pub struct SizeQuery {
    offset_id: Option<u64>,
    count: Option<usize>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct TagsQuery {
    tags: String,
    offset_id: Option<u64>,
    count: Option<usize>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct UrlQuery {
    offset: Option<u64>,
    count: Option<usize>,
    types: Option<String>,
    remote_host: Option<SocketAddr>,
    request_id: Option<u64>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct TsQuery {
    starts_from: Option<u128>,
    level: Option<String>,
    count: Option<usize>,
}

#[tokio::main]
async fn main() -> Result<(), MainError> {
    // -- Initialize logger
    simple_logger::init()?;

    // -- Load basic arguments
    let app_config = AppConfig::from_env();
    log::info!("Loaded arguments from CLI");
    let identity = app_config.load_identity()?;
    log::info!("Loaded identity file from '{}'", app_config.identity_file);

    // -- Initialize RocksDB
    let db = app_config.open_database()?;
    log::info!("Created RocksDB storage in: {}", app_config.storage_path);

    // -- Create logs reader
    make_logs_reader(&app_config.logs_path, db.clone())?;
    log::info!("Reading logs from: {}", app_config.logs_path);

    // loop {}

    // -- Create TUN devices
    let ((_, receiver), writer) = make_bridge(
        &app_config.tun0_address_space,
        &app_config.tun1_address_space,
        &app_config.tun0_address,
        &app_config.tun1_address,
        app_config.local_address.parse()?,
        app_config.tun1_address.parse()?,
    )?;

    log::info!("Created TUN bridge on {} <-> {} <-> {}",
        app_config.local_address,
        app_config.tun0_address,
        app_config.tun1_address,
    );

    // -- Start Actor system
    let system = ActorSystem::new()?;
    let orchestrator = system.actor_of(Props::new_args(PacketOrchestrator::new, PacketOrchestratorArgs {
        rpc_port: app_config.rpc_port,
        local_address: app_config.local_address.parse()?,
        fake_address: app_config.tun1_address.parse()?,
        local_identity: identity.clone(),
        db: db.clone(),
        writer: Arc::new(Mutex::new(writer)),
    }), "packet_orchestrator")?;

    std::thread::spawn(move || {
        loop {
            for message in receiver.recv() {
                orchestrator.tell(message, None);
            }
        }
    });

    log::info!("Started to analyze traffic through {} device", app_config.tun0_name);

    let tmp = db.clone();
    let cloner = move || {
        tmp.clone()
    };

    // -- Initialize server
    let p2p_raw = warp::path!("p2p" / u64 / u64)
        .map(move |offset: u64, count: u64| {
            match cloner().get_p2p_reverse_range(Some(offset), count as usize) {
                Ok(value) => serde_json::to_string(&value)
                    .expect("failed to serialize response"),
                Err(e) => serde_json::to_string(&format!("Database error: {}", e))
                    .expect("failed to serialize response")
            }
        }).map(|value| {
        Response::builder()
            .header("Content-Type", "application/json")
            .header("Access-Control-Allow-Origin", "*")
            .body(value)
    });

    let tmp = db.clone();
    let cloner = move || {
        tmp.clone()
    };

    let p2p_host = warp::path!("p2p" / u64 / u64 / SocketAddr)
        .map(move |offset: u64, count: u64, addr: SocketAddr| {
            match cloner().get_p2p_host_range(offset, count, addr) {
                Ok(value) => {
                    serde_json::to_string(&value)
                        .expect("failed to serialize the array")
                }
                Err(e) => serde_json::to_string(
                    &format!("Failed to read database: {}", e),
                ).unwrap()
                ,
            }
        })
        .map(|value| {
            Response::builder()
                .header("Content-Type", "application/json")
                .header("Access-Control-Allow-Origin", "*")
                .body(value)
        });

    let tmp = db.clone();
    let cloner = move || {
        tmp.clone()
    };

    let rpc_raw = warp::path!("rpc" / u64 / u64)
        .map(move |offset: u64, count: u64| {
            match cloner().get_rpc_range(offset, count) {
                Ok(value) => {
                    serde_json::to_string(&value)
                        .expect("failed to serialize the array")
                }
                Err(e) => serde_json::to_string(
                    &format!("Failed to read database: {}", e)
                ).unwrap(),
            }
        }).map(|value| {
        Response::builder()
            .header("Content-Type", "application/json")
            .header("Access-Control-Allow-Origin", "*")
            .body(value)
    });

    let tmp = db.clone();
    let cloner = move || {
        tmp.clone()
    };

    let rpc_host = warp::path!("rpc" / u64 / u64 / IpAddr)
        .map(move |offset: u64, count: u64, addr: IpAddr| {
            match cloner().get_rpc_host_range(offset, count, addr) {
                Ok(value) => {
                    serde_json::to_string(&value)
                        .expect("failed to serialize the array")
                }
                Err(e) => serde_json::to_string(
                    &format!("Failed to read database: {}", e),
                ).unwrap()
            }
        })
        .map(|value| {
            Response::builder()
                .header("Content-Type", "application/json")
                .header("Access-Control-Allow-Origin", "*")
                .body(value)
        });

    let tmp = db.clone();
    let cloner = move || {
        tmp.clone()
    };

    // If no query is provided, return last 100 p2p messages
    let v2_p2p = warp::path!("v2" / "p2p")
        .and(warp::query::query())
        .map(move |query: UrlQuery| -> String {
            let offset = query.offset.unwrap_or(0);
            let count = query.count.unwrap_or(100);
            let types = query.types.as_ref().map(|types|
                Type::parse_tags(&types)
            );

            // if query.remote_host.is_some() && query.request_id.is_some() {
            //     // TODO: Handle this properly, it does not make sense to provide both
            // }

            if let Some(remote_host) = query.remote_host {
                // Host + types
                if let Some(types) = types {
                    // Match
                    let types = match types {
                        Ok(types) => types,
                        Err(_) => return serde_json::to_string(&format!("Invalid types: {}", query.types.unwrap()))
                            .expect("failed to serialize response"),
                    };
                    match cloner().get_p2p_host_type_range(offset as usize, count, remote_host, types) {
                        Ok(value) => serde_json::to_string(&value)
                            .expect("failed to serialize response"),
                        Err(e) => serde_json::to_string(&format!("Database error: {}", e))
                            .expect("failed to serialize response")
                    }
                } else {
                    // Host only
                    match cloner().get_p2p_host_range(offset, count as u64, remote_host) {
                        Ok(value) => serde_json::to_string(&value)
                            .expect("failed to serialize response"),
                        Err(e) => serde_json::to_string(&format!("Database error: {}", e))
                            .expect("failed to serialize response")
                    }
                }
            } else if let Some(request_id) = query.request_id {
                // Ignore types for now, as it does not truly make sense to filter specific request by types
                match cloner().get_p2p_request_range(offset as usize, count, request_id) {
                    Ok(value) => serde_json::to_string(&value)
                        .expect("failed to serialize response"),
                    Err(e) => serde_json::to_string(&format!("Database error: {}", e))
                        .expect("failed to serialize response")
                }
            } else {
                // Just get last X messages
                match cloner().get_p2p_reverse_range(query.offset, count) {
                    Ok(value) => serde_json::to_string(&value)
                        .expect("failed to serialize response"),
                    Err(e) => serde_json::to_string(&format!("Database error: {}", e))
                        .expect("failed to serialize response")
                }
            }
        })
        .map(|value| {
            Response::builder()
                .header("Content-Type", "application/json")
                .header("Access-Control-Allow-Origin", "*")
                .body(value)
        });

    let tmp = db.clone();
    let cloner = move || {
        tmp.clone()
    };

    // If no query is provided, return last 100 p2p messages
    let v2_p2p_host = warp::path!("v2" / "p2p" / "host" / SocketAddr)
        .and(warp::query::query())
        .map(move |host, query: SizeQuery| {
            let count = query.count.unwrap_or(100);
            match cloner().get_p2p_host_range(query.offset_id.unwrap_or(0), count as u64, host) {
                Ok(value) => serde_json::to_string(&value)
                    .expect("failed to serialize response"),
                Err(e) => serde_json::to_string(&format!("Database error: {}", e))
                    .expect("failed to serialize response")
            }
        })
        .map(|value| {
            Response::builder()
                .header("Content-Type", "application/json")
                .header("Access-Control-Allow-Origin", "*")
                .body(value)
        });

    let tmp = db.clone();
    let cloner = move || {
        tmp.clone()
    };

    let v2_p2p_tags = warp::path!("v2"/ "p2p" / "types")
        .and(warp::query::query())
        .map(move |query: TagsQuery| {
            let tags = Type::parse_tags(&query.tags);
            let tags = match tags {
                Ok(tags) => tags,
                Err(err) => {
                    return serde_json::to_string(&format!("Database error: {}", err))
                        .expect("faield to serialize response");
                }
            };
            let count = query.count.unwrap_or(100);
            let offset = query.offset_id.unwrap_or(0) as usize;
            match cloner().get_p2p_types_range(offset, count, tags) {
                Ok(value) => serde_json::to_string(&value)
                    .expect("failed to serialize response"),
                Err(e) => serde_json::to_string(&format!("Database error: {}", e))
                    .expect("failed to serialize response")
            }
        })
        .map(|value| {
            Response::builder()
                .header("Content-Type", "application/json")
                .header("Access-Control-Allow-Origin", "*")
                .body(value)
        });

    let tmp = db.clone();
    let cloner = move || {
        tmp.clone()
    };

    let v2_p2p_requests = warp::path!("v2" / "p2p" / "request" / u64)
        .and(warp::query::query())
        .map(move |request_id: u64, query: SizeQuery| {
            let count = query.count.unwrap_or(100) as usize;
            let offset = query.offset_id.unwrap_or(0) as usize;
            match cloner().get_p2p_request_range(offset, count, request_id) {
                Ok(value) => serde_json::to_string(&value)
                    .expect("failed to serialize response"),
                Err(err) => serde_json::to_string(&format!("Database error: {}", err))
                    .expect("failed to serialize response")
            }
        })
        .map(|value| Response::builder()
            .header("Content-Type", "application/json")
            .header("Access-Control-Allow-Origin", "*")
            .body(value)
        );

    let tmp = db.clone();
    let cloner = move || {
        tmp.clone()
    };

    let v2_log = warp::path!("v2" / "log")
        .and(warp::query::query())
        .map(move |query: TsQuery| {
            let ts = query.starts_from.unwrap_or(0);
            let count = query.count.unwrap_or(100);

            if let Some(ref level) = query.level {
                match cloner().log_db().get_timestamp_level_range(level, ts, count) {
                    Ok(value) => serde_json::to_string(&value)
                        .expect("failed to serialize response"),
                    Err(err) => serde_json::to_string(&format!("Database error: {}", err))
                        .expect("failed to serialize response")
                }
            } else {
                match cloner().log_db().get_timestamp_range(ts, count) {
                    Ok(value) => serde_json::to_string(&value)
                        .expect("failed to serialize response"),
                    Err(err) => serde_json::to_string(&format!("Database error: {}", err))
                        .expect("failed to serialize response")
                }
            }
        })
        .map(|value| Response::builder()
            .header("Content-Type", "application/json")
            .header("Access-Control-Allow-Origin", "*")
            .body(value)
        );

    // let router = warp::get().and(p2p_raw.or(p2p_host).or(rpc_raw).or(rpc_host).or(v2_p2p));
    let router = warp::get().and(
        v2_p2p.or(v2_p2p_host).or(v2_p2p_tags).or(v2_p2p_requests)
            .or(v2_log)
            .or(rpc_raw).or(rpc_host)
            .or(p2p_raw).or(p2p_host)
    );

    warp::serve(router)
        // TODO: Add as config settings
        .run(([0, 0, 0, 0], app_config.rpc_port))
        .await;

    Ok(())
}