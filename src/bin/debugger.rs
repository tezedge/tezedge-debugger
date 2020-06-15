use tracing::{info, error, Level};
use tezedge_debugger::system::build_raw_socket_system;
use std::process::exit;

#[tokio::main]
async fn main() -> Result<(), failure::Error> {
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    match build_raw_socket_system() {
        Ok(_) => {
            info!("system built");
        }
        Err(err) => {
            error!(error = display(err), "failed to build system");
            exit(1);
        }
    }

    if let Err(err) = tokio::signal::ctrl_c().await {
        error!(error = display(err), "failed while listening for signal");
        exit(1)
    }

    info!("ctrl-c received");

    Ok(())
}