// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use std::io;
use super::ProcessStatSource;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessStat {
    timestamp: DateTime<Utc>,
    cmd: String,
    memory_usage: u64,
    memory_cache: u64,
}

impl ProcessStatSource for ProcessStat {
    fn timestamp(&self) -> DateTime<Utc> {
        self.timestamp.clone()
    }

    fn process_cmd(&self) -> &str {
        self.cmd.as_str()
    }

    fn memory_usage(&self) -> u64 {
        self.memory_usage
    }

    fn memory_cache(&self) -> u64 {
        self.memory_cache
    }
}

impl ProcessStat {
    pub fn read_from_system(pid: u32) -> Result<Self, io::Error> {
        let _ = pid;
        unimplemented!()
    }
}
