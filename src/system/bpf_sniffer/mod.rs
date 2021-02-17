// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

mod connection;
mod event_loop;

use super::{p2p, SystemSettings, processor};

use std::path::PathBuf;
use serde::{Serialize, Deserialize};
use tokio::sync::mpsc;
use sniffer::Module;

#[derive(Clone)]
pub struct BpfSniffer {
    command_tx: mpsc::UnboundedSender<BpfSnifferCommand>,
}

#[derive(Debug)]
/// The command for the sniffer
pub enum BpfSnifferCommand {
    /// Stop sniffing, the async task will terminate
    Terminate,
    /// if `filename` has a value, will dump the content of ring buffer to the file
    /// if report is true, will send a `BpfSnifferReport` as a `BpfSnifferResponse`
    GetDebugData {
        filename: Option<PathBuf>,
        report: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BpfSnifferReport {
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BpfSnifferResponse {
    Report(BpfSnifferReport),
}

impl BpfSniffer {
    pub fn spawn(settings: &SystemSettings) -> Self {
        let (command_tx, _command_rx) = mpsc::unbounded_channel();
        let module = Module::load(&settings.namespace);
        let bpf_sniffer = self::event_loop::EventProcessor::new(module, settings);
        tokio::spawn(bpf_sniffer.run());
        BpfSniffer { command_tx }
    }

    pub fn send(&self, command: BpfSnifferCommand) {
        self.command_tx.send(command)
            .expect("failed to send command")
    }

    pub fn recv() -> Option<BpfSnifferResponse> {
        //SNIFFER_RESPONSE.lock().unwrap().take()
        None
    }
}
