#![allow(dead_code)]

mod configuration;
mod actors;
mod network;
mod storage;
mod endpoints;

use crate::{
    endpoints::routes,
    actors::prelude::*,
    network::prelude::*,
    configuration::AppConfig,
};
use failure::Fail;
use riker::actors::*;
use main_error::MainError;
use std::sync::{Mutex, Arc};

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

    warp::serve(routes(db))
        .run(([0, 0, 0, 0], 10000))
        .await;

    Ok(())
}