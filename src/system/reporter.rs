use tokio::sync::mpsc;
use super::p2p;

pub struct Reporter {
    tx_p2p_command: mpsc::Sender<p2p::Command>,
    rx_p2p_report: mpsc::Receiver<serde_json::Value>,
}

impl Reporter {
    pub fn new(
        tx_p2p_command: mpsc::Sender<p2p::Command>,
        rx_p2p_report: mpsc::Receiver<serde_json::Value>,
    ) -> Self {
        Reporter {
            tx_p2p_command,
            rx_p2p_report,
        }
    }

    pub async fn get_p2p_report(&mut self) -> Option<serde_json::Value> {
        match self.tx_p2p_command.send(p2p::Command::GetReport).await {
            Ok(()) => self.rx_p2p_report.recv().await,
            Err(_) => None,
        }
    }
}
