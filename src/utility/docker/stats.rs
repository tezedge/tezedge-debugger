use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stats {
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
pub struct BlkioStats {
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageStats {
}

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
pub struct PreCpuStats {
}

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
pub struct Network {
}
