use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
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
    pub precpu_stats: PreCpuStats,
    pub memory_stats: MemoryStats,
    pub networks: HashMap<String, Network>,
}

#[derive(Debug, Deserialize)]
pub struct PidsStats {
    pub current: u64,
}

#[derive(Debug, Deserialize)]
pub struct BlkioStats {
}

#[derive(Debug, Deserialize)]
pub struct StorageStats {
}

#[derive(Debug, Deserialize)]
pub struct CpuStats {
}

#[derive(Debug, Deserialize)]
pub struct PreCpuStats {
}

#[derive(Debug, Deserialize)]
pub struct MemoryStats {
}

#[derive(Debug, Deserialize)]
pub struct Network {
}
