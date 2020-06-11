pub mod system;
pub mod utility;
pub mod messages;

use std::process::exit;

use tokio::signal;
use tracing::{info, error, Level};

use crate::system::build_raw_socket_system;

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
    // Initialize tracing library
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    // - Create the packet producer from step 1.
    // - Within the producer create and manage the orchestrator from step 2
    // - Orchestrator spawns all parsers from step 3
    // - Orchestrator also spawns processors from 4
    // to directly feed produced packets to orchestrator
    match build_raw_socket_system() {
        Ok(_) => {
            info!("opened raw socket packet producer");
        }
        Err(err) => {
            error!("failed to open raw socket producer: {}", err);
            exit(-1);
        }
    };

    // Run until signal to stop is received.
    if let Err(err) = signal::ctrl_c().await {
        error!("failed while listening for signal: {}", err);
        exit(-1);
    }
    info!("ctrl-c received")
}
