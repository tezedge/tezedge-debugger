#![allow(dead_code)]

mod configuration;
mod actors;
mod network;
mod storage;

use failure::{Error, Fail};
use riker::actors::*;
use warp::Filter;

use pnet::{packet::{
    Packet as _,
    tcp::TcpPacket,
    ipv4::Ipv4Packet,
    ipv6::Ipv6Packet,
    ethernet::{EthernetPacket, EtherTypes},
    ip::IpNextHeaderProtocols,
}, datalink};

use crate::{
    actors::prelude::*,
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
async fn main() -> Result<(), Error> {
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

    // -- Start Actor system
    let system = ActorSystem::new()?;
    let orchestrator = system.actor_of(Props::new_args(PacketOrchestrator::new, PacketOrchestratorArgs {
        local_identity: identity.clone(),
        db: db.clone(),
    }), "packet_orchestrator")?;

    // -- Acquire raw network interface
    let interface = datalink::interfaces().into_iter()
        .filter(|x| x.name == app_config.interface)
        .next()
        .ok_or(AppError::NoNetworkInterface)?;
    log::info!("Captured interface {}", interface.name);
    let (_, mut rx) = datalink::channel(&interface, Default::default())
        .map_err(|err| AppError::IOError(err))
        .and_then(|chan| match chan {
            datalink::Channel::Ethernet(tx, rx) => Ok((tx, rx)),
            _ => Err(AppError::UnsupportedNetworkChannelType)
        })?;

    log::info!("Starting to analyze traffic on port {}", app_config.port);

    std::thread::spawn(move || {
        loop {
            let packet = EthernetPacket::new(rx.next().expect("Failed to read packet")).unwrap();
            let (payload, protocol) = match packet.get_ethertype() {
                EtherTypes::Ipv4 => {
                    let header = Ipv4Packet::new(packet.payload()).unwrap();
                    (header.payload().to_vec(), header.get_next_level_protocol())
                }
                EtherTypes::Ipv6 => {
                    let header = Ipv6Packet::new(packet.payload()).unwrap();
                    ((header.payload()).to_vec(), header.get_next_header())
                }
                _ => continue,
            };

            if protocol == IpNextHeaderProtocols::Tcp {
                let tcp = TcpPacket::new(&payload).unwrap();
                let (source, dest) = (tcp.get_source(), tcp.get_destination());
                if app_config.port == dest {
                    orchestrator.send_msg(Packet::outgoing(source, tcp.payload().to_vec()), None);
                } else if app_config.port == source {
                    orchestrator.send_msg(Packet::incoming(dest, tcp.payload().to_vec()), None);
                } else {
                    continue;
                }
            }
        }
    });

    let cloner = move || {
        db.clone()
    };

    let endpoint = move |start, end| {
        match cloner().get_range(start, end) {
            Ok(value) => serde_json::to_string(&value).expect("failed to serialize the array"),
            _ => format!("failed")
        }
    };

    let port: usize = 5050;
    log::info!("Starting to serving data at {}", port);

    // -- Initialize server
    let endpoint = warp::path!("data" / u64 / u64)
        .map(endpoint);

    warp::serve(endpoint)
        .run(([127, 0, 0, 1], 5050))
        .await;

    Ok(())
}