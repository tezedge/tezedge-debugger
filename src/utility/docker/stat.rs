// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use chrono::{DateTime, Utc, Duration};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use crate::utility::stats::StatSource;

impl StatSource for Stat {
    fn timestamp(&self) -> DateTime<Utc> {
        self.read.clone()
    }

    fn memory_usage(&self) -> u64 {
        self.memory_stats.usage
    }

    fn memory_cache(&self) -> u64 {
        self.memory_stats.stats.cache
    }

    fn container_cpu_usage(&self) -> Duration {
        Duration::nanoseconds(self.cpu_stats.cpu_usage.total_usage as i64)
    }

    fn total_cpu_usage(&self) -> Duration {
        Duration::nanoseconds(self.cpu_stats.system_cpu_usage as i64)
    }

    fn last_container_cpu_usage(&self) -> Duration {
        Duration::nanoseconds(self.precpu_stats.cpu_usage.total_usage as i64)
    }

    fn last_total_cpu_usage(&self) -> Duration {
        Duration::nanoseconds(self.precpu_stats.system_cpu_usage as i64)
    }

    fn num_processors(&self) -> usize {
        self.cpu_stats.online_cpus as usize
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stat {
    pub read: DateTime<Utc>,
    pub preread: DateTime<Utc>,
    pub name: String,
    pub id: String,
    pub num_procs: u32,

    pub pids_stats: PidsStats,
    pub blkio_stats: BlkioStats,
    pub storage_stats: StorageStats,
    pub cpu_stats: CpuStats,
    pub precpu_stats: CpuStats,
    pub memory_stats: MemoryStats,
    #[serde(default)]
    pub networks: HashMap<String, Network>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PidsStats {
    #[serde(default)]
    pub current: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlkioStats {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageStats {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuStats {
    #[serde(default)]
    pub cpu_usage: CpuUsage,
    #[serde(default)]
    pub system_cpu_usage: u64,
    #[serde(default)]
    pub online_cpus: u64,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct CpuUsage {
    #[serde(default)]
    pub total_usage: u64,
    #[serde(default)]
    pub percpu_usage: Vec<u64>,
    #[serde(default)]
    pub usage_in_kernelmode: u64,
    #[serde(default)]
    pub usage_in_usermode: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreCpuStats {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStats {
    #[serde(default)]
    pub limit: u64,
    #[serde(default)]
    pub usage: u64,
    #[serde(default)]
    pub max_usage: u64,
    #[serde(default)]
    pub stats: MemoryStatsExt,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct MemoryStatsExt {
    #[serde(default)]
    pub cache: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Network {}
