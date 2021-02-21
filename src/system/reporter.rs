use tokio::sync::mpsc;
use super::p2p;

pub struct Reporter {
    pub(super) tx_p2p_command: mpsc::Sender<p2p::Command>,
    pub(super) rx_p2p_report: mpsc::Receiver<p2p::Report>,
}

impl Reporter {
    pub async fn get_p2p_report(&mut self) -> Option<p2p::Report> {
        match self.tx_p2p_command.send(p2p::Command).await {
            Ok(()) => self.rx_p2p_report.recv().await,
            Err(_) => None,
        }
    }
}
