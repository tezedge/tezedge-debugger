pub mod actors;
pub mod utility;
pub mod storage;
pub mod configuration;

use failure::Error;
use crate::configuration::AppConfig;
use riker_producer::prelude::*;
use riker::actors::*;
use crate::actors::producers::nfqueue_producer::PacketProducer;
use crate::actors::producers::packet_orchestrator::{PacketOrchestrator, PacketOrchestratorArgs};
use crate::actors::processors::processors::Processors;

#[tokio::main]
async fn main() -> Result<(), Error> {
    simple_logger::init()?;
    let cfg = AppConfig::from_env();
    let identity = cfg.load_identity()?;
    let storage = cfg.open_storage()?;
    let system = ActorSystem::new()?;

    system.actor_of_args::<Producer<PacketProducer, PacketOrchestrator>, _>("packet_producer", (
        (), PacketOrchestratorArgs {
            rpc_port: cfg.rpc_port,
            local_identity: identity.clone(),
            local_address: cfg.local_address.clone(),
        }
    ))?;

    system.actor_of_args::<Processors, _>("processors", storage.clone())?;
    let _ = tokio::signal::ctrl_c()
        .await;
    log::info!("Received ctrl-c signal. Shutting down.");

    let _ = system.shutdown().await;
    log::info!("System shut down.");

    Ok(())
}
