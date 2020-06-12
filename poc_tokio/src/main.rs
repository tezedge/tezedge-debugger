pub mod system;
pub mod utility;
pub mod messages;

use std::process::exit;

use tokio::signal;
use tracing::{info, error, Level};

use crate::system::{build_raw_socket_system, build_nfqueue_system};
use std::net::IpAddr;

#[allow(dead_code)]
fn init_tracer() -> Result<(), failure::Error> {
    use opentelemetry::api::Provider;
    use opentelemetry::sdk;
    use tracing_subscriber::prelude::*;

    let exporter = opentelemetry_jaeger::Exporter::builder()
        .with_agent_endpoint("127.0.0.1:6831".parse().unwrap())
        .with_process(opentelemetry_jaeger::Process {
            service_name: "debugger".to_string(),
            tags: Vec::new(),
        })
        .init()?;
    let provider = sdk::Provider::builder()
        .with_simple_exporter(exporter)
        .with_config(sdk::Config {
            default_sampler: Box::new(sdk::Sampler::Always),
            ..Default::default()
        })
        .build();
    let tracer = provider.get_tracer("tracing");

    let opentelemetry = tracing_opentelemetry::layer().with_tracer(tracer);
    tracing_subscriber::registry()
        .with(opentelemetry)
        .try_init()?;

    Ok(())
}

/// 1. There is a producer, which provides packet for analysis (either from a network or from some sort of storage).
/// -  Producer is in form of a asynchronous channel
/// 2. There is an orchestrator, which sorts packets by a tcp connections
/// -  All packets from a single connection are sent to a dedicated parser, for that connection
/// 3. There are packet parsers, which buffers contents of TCP packets, extract keys & nonces.
/// -  Parsers decipher & deserialize content of TCP packets.
/// -  Deserialized messages are forwarded to message processors
/// 4. There is a message processor orchestrator
/// -  Processor orchestrator holds all processing units for messages
/// -  There should always be at leas one processor.
/// -  For benchmarking, there should be a `Printing Processor` which only prints the messages to the console.
/// -  For debugging there should be a `Storage` which saves and indexes all messages into dedicated storage.
/// -  For replays, there should be a `Replayer` which forwards packets to dedicated node.
/// 5. There is an REST server, providing data processed by `Storage` processor.
/// --
/// For easiest model possible, there is only a producer (either raw socket or NFQUEUE producer).
/// Orchestrator with only P2P processors (no RPC). And single printing processor.
#[tokio::main]
async fn main() {
    // let _ = init_tracer();
    // For now hardcoded values valid in docker
    // let ip_addr: IpAddr = "10.0.0.0".parse().unwrap();

    // Initialize tracing library
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    let variable = std::env::var("SYSTEM").unwrap_or_default();
    info!(name = display("SYSTEM"), value = display(variable.clone()), "env variable");
    let system = if &variable == "nfqueue" {
        info!("building nfqueue system");
        build_nfqueue_system()
    } else {
        info!("building raw socket system");
        build_raw_socket_system()
    };

    // - Create the packet producer from step 1.
    // - Within the producer create and manage the orchestrator from step 2
    // - Orchestrator spawns all parsers from step 3
    // - Orchestrator also spawns processors from 4
    // to directly feed produced packets to orchestrator
    match system {
        Ok(_) => {
            info!("build system");
        }
        Err(err) => {
            error!("failed build system: {}", err);
            exit(-1);
        }
    };

    // Run until signal to stop is received.
    if let Err(err) = signal::ctrl_c().await {
        error!("failed while listening for signal: {}", err);
        exit(-1);
    }
    info!(bytes = system::get_loaded_data(), "traced data");
    info!("ctrl-c received")
}
