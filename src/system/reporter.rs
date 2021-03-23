use tokio::{sync::mpsc, task::JoinHandle};
use warp::reply::{Json, json};
use super::{DebuggerConfig, p2p};
use crate::storage_::{StoreCollector, p2p::Message as P2pMessage};

#[cfg(target_os = "linux")]
use super::parser::Parser;

pub struct Reporter {
    tx_p2p_command: mpsc::Sender<p2p::Command>,
    rx_p2p_command: Option<mpsc::Receiver<p2p::Command>>,
    tx_p2p_report: mpsc::Sender<p2p::Report>,
    rx_p2p_report: mpsc::Receiver<p2p::Report>,
}

impl Reporter {
    pub fn new() -> Self {
        let (tx_p2p_command, rx_p2p_command) = mpsc::channel(8);
        let (tx_p2p_report, rx_p2p_report) = mpsc::channel(8);

        Reporter {
            tx_p2p_command,
            rx_p2p_command: Some(rx_p2p_command),
            tx_p2p_report,
            rx_p2p_report,
        }
    }

    pub fn spawn_parser<S>(&mut self, storage: S, config: &DebuggerConfig) -> Option<JoinHandle<()>>
    where
        S: Clone + StoreCollector<P2pMessage> + Send + 'static,
    {
        if let Some(rx_p2p_command) = self.rx_p2p_command.take() {
            #[cfg(target_os = "linux")] {
                Parser::try_spawn(storage, config, rx_p2p_command, self.tx_p2p_report.clone())
            }
            #[cfg(not(target_os = "linux"))] {
                tracing::warn!("can intercept p2p only on linux");
                None
            }
        } else {
            tracing::warn!("p2p system already running");
            None
        }
    }

    pub async fn get_p2p_report(&mut self) -> Json {
        match self.tx_p2p_command.send(p2p::Command::GetReport).await {
            Ok(()) => {
                #[cfg(target_os = "linux")] {
                    let report = self.rx_p2p_report.recv().await;
                    json(&report)
                }
                #[cfg(not(target_os = "linux"))] {
                    json::<Option<()>>(&None)
                }
            },
            Err(_) => json::<Option<()>>(&None),
        }
    }

    pub async fn get_counter(&mut self) {
        let _ = self.tx_p2p_command.send(p2p::Command::GetCounter).await;
    }

    pub async fn terminate(&self) {
        #[cfg(target_os = "linux")] {
            let _ = self.tx_p2p_command.send(p2p::Command::Terminate).await;
        }
    }
}
