use tokio::sync::mpsc;
use super::{DebuggerConfig, p2p};
use crate::storage_::P2pStore;

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

    pub fn spawn_parser(&mut self, storage: &P2pStore, config: &DebuggerConfig) {
        if let Some(rx_p2p_command) = self.rx_p2p_command.take() {
            #[cfg(target_os = "linux")] {
                Parser::try_spawn(storage, config, rx_p2p_command, self.tx_p2p_report.clone())
            }
            #[cfg(not(target_os = "linux"))] {
                tracing::warn!("can intercept p2p only on linux");
            }
        } else {
            tracing::warn!("p2p system already running");
        }
    }

    pub async fn get_p2p_report(&mut self) -> serde_json::Value {
        match self.tx_p2p_command.send(p2p::Command::GetReport).await {
            Ok(()) => {
                #[cfg(target_os = "linux")] {
                    let report = self.rx_p2p_report.recv().await;
                    serde_json::to_value(report).unwrap()
                }
                #[cfg(not(target_os = "linux"))] {
                    serde_json::Value::Null
                }
            },
            Err(_) => serde_json::Value::Null,
        }
    }

    pub async fn terminate(&self) -> Result<(), ()> {
        #[cfg(target_os = "linux")] {
            self.tx_p2p_command.send(p2p::Command::Terminate).await.map_err(|_| ())
        }
        #[cfg(not(target_os = "linux"))] {
            Ok(())
        }
    }
}
