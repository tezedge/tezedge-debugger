#![allow(dead_code)]

mod configuration;
mod actors;
mod network;
mod storage;

use std::{
    sync::{Mutex, Arc},
};

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
        .map(move |offset, count| {
            match cloner().get_p2p_range(offset, count) {
                Ok(value) => serde_json::to_string(&value)
                    .expect("failed to serialize the array"),
                Err(e) => serde_json::to_string(
                    &format!("Failed to read database: {}", e)
                ).unwrap(),
            }
        })
        .map(|value| {
            Response::builder()
                .header("Content-Type", "application/json")
                .body(value)
        });

    let tmp = db.clone();
    let cloner = move || {
        tmp.clone()
    };

    let p2p_host = warp::path!("p2p" / u64 / u64 / String)
        .map(move |offset, count, host: String| {
            match host.parse() {
                Ok(addr) => match cloner().get_p2p_host_range(offset, count, addr) {
                    Ok(value) => serde_json::to_string(&value)
                        .expect("failed to serialize the array"),
                    Err(e) => serde_json::to_string(
                        &format!("Failed to read database: {}", e),
                    ).unwrap()
                },
                Err(e) => serde_json::to_string(
                    &format!("Invalid socket address: {}", e),
                ).unwrap(),
            }
        })
        .map(|value| {
            Response::builder()
                .header("Content-Type", "application/json")
                .body(value)
        });

    let tmp = db.clone();
    let cloner = move || {
        tmp.clone()
    };

    let rpc_raw = warp::path!("rpc" / u64 / u64)
        .map(move |offset, count| {
            match cloner().get_rpc_range(offset, count) {
                Ok(value) => serde_json::to_string(&value)
                    .expect("failed to serialize the array"),
                Err(e) => serde_json::to_string(
                    &format!("Failed to read database: {}", e)
                ).unwrap(),
            }
        })
        .map(|value| {
            Response::builder()
                .header("Content-Type", "application/json")
                .body(value)
        });

    let tmp = db.clone();
    let cloner = move || {
        tmp.clone()
    };

    let rpc_host = warp::path!("rpc" / u64 / u64 / String)
        .map(move |offset, count, host: String| {
            match host.parse() {
                Ok(addr) => match cloner().get_rpc_host_range(offset, count, addr) {
                    Ok(value) => serde_json::to_string(&value)
                        .expect("failed to serialize the array"),
                    Err(e) => serde_json::to_string(
                        &format!("Failed to read database: {}", e),
                    ).unwrap()
                },
                Err(e) => serde_json::to_string(
                    &format!("Invalid socket address: {}", e),
                ).unwrap(),
            }
        })
        .map(|value| {
            Response::builder()
                .header("Content-Type", "application/json")
                .body(value)
        });

    let router = warp::get().and(p2p_raw.or(p2p_host).or(rpc_raw).or(rpc_host));

    warp::serve(router)
        // TODO: Add as config settings
        .run(([0, 0, 0, 0], app_config.rpc_port))
        .await;

    Ok(())
}