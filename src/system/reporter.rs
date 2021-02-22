use tokio::sync::mpsc;
use super::p2p;

pub struct Reporter {
    tx_p2p_command: mpsc::Sender<p2p::Command>,
    rx_p2p_report: mpsc::Receiver<p2p::Report>,
}

impl Reporter {
    pub fn new(
        tx_p2p_command: mpsc::Sender<p2p::Command>,
        rx_p2p_report: mpsc::Receiver<p2p::Report>,
    ) -> Self {
        Reporter {
            tx_p2p_command,
            rx_p2p_report,
        }
    }

    pub async fn get_p2p_report(&mut self) -> Option<p2p::Report> {
        match self.tx_p2p_command.send(p2p::Command::GetReport).await {
            Ok(()) => self.rx_p2p_report.recv().await,
            Err(_) => None,
        }
    }
}
