// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use sysinfo::{System, SystemExt, DiskExt};
use chrono::{DateTime, Utc, Duration};
use super::StatSource;

/// Configuration of alert appearing conditions and other information
#[derive(Clone)]
pub struct AlertConfig {
    /// mount point of disk where database stored
    // needed for disk exhausting estimation
    pub db_mount_point: String,
}

impl AlertConfig {
    pub fn monitor(&self) -> CapacityMonitor {
        let system = {
            use sysinfo::RefreshKind;

            let r = RefreshKind::new()
                .with_disks_list()
                .with_disks()
                .with_memory()
                .with_cpu();
            let mut s = System::new_with_specifics(r);
            s.refresh_disks_list();
            s.refresh_disks();
            s.refresh_memory();
            s.refresh_cpu();
            s
        };
        let disk_index = system
            .get_disks()
            .iter()
            .enumerate()
            .find(|&d| d.1.get_mount_point().to_str() == Some(self.db_mount_point.as_str()))
            .map(|(x, _)| x);
        CapacityMonitor {
            system: system,
            disk_index: disk_index,
            disk: DiskEstimator::new(),
            memory: MemoryEstimator::new(),
            cpu_usage: None,
        }
    }
}

struct DiskEstimator;

impl DiskEstimator {
    pub fn new() -> Self {
        DiskEstimator
    }
}

struct MemoryEstimator {
    usage: Vec<(DateTime<Utc>, u64)>,
}

impl MemoryEstimator {
    pub fn new() -> Self {
        MemoryEstimator { usage: Vec::new() }
    }

    pub fn observe(&mut self, timestamp: DateTime<Utc>, usage: u64) {
        // TODO(capacity estimation):
        self.usage.push((timestamp, usage));
    }

    pub fn estimate(&self, _available: u64) -> Option<Duration> {
        // TODO(capacity estimation):
        None
    }

    pub fn status(&self) -> u64 {
        self.usage.last().cloned().unwrap_or((Utc::now(), 0)).1
    }
}

pub struct CapacityMonitor {
    system: System,
    disk_index: Option<usize>,
    disk: DiskEstimator,
    memory: MemoryEstimator,
    cpu_usage: Option<f64>,
}

impl CapacityMonitor {
    pub fn observe<S>(&mut self, stats: &S)
    where
        S: StatSource,
    {
        let timestamp = stats.timestamp();
        self.memory
            .observe(timestamp, stats.memory_usage() - stats.memory_cache());
        let c_cpu_delta = stats.container_cpu_usage() - stats.last_container_cpu_usage();
        let c_cpu_delta = c_cpu_delta.num_nanoseconds().unwrap_or(1) as f64;
        let t_cpu_delta = stats.total_cpu_usage() - stats.last_total_cpu_usage();
        let t_cpu_delta = t_cpu_delta.num_nanoseconds().unwrap_or(1) as f64;
        self.cpu_usage = Some((c_cpu_delta / t_cpu_delta) * (stats.num_processors() as f64));
        self.system.refresh_disks();
        self.system.refresh_memory();
        self.system.refresh_cpu();
    }

    pub fn alert(&self) -> Vec<String> {
        let mut alerts = Vec::new();
        if let Some(estimate) = self.memory.estimate(self.system.get_available_memory()) {
            if estimate < Duration::minutes(10) {
                alerts.push(format!(
                    "Warning, memory will exhaust estimated in {}",
                    estimate
                ));
            }
        }

        // TODO(capacity estimation):
        let _ = &self.disk;

        alerts
    }

    pub fn status(&self) -> Vec<String> {
        let mut v = Vec::new();
        let gb = |x| (x as f64) / (0x40000000 as f64);

        let memory = self.memory.status();
        v.push(format!(
            "Memory used: {:.2} GiB, free: {:.2} GiB",
            gb(memory),
            gb(self.system.get_free_memory() * 1024),
        ));

        if let Some(disk_index) = self.disk_index {
            if let Some(disk) = self.system.get_disks().get(disk_index) {
                let name = disk.get_name();
                let total = gb(disk.get_total_space());
                let available = gb(disk.get_available_space());
                v.push(format!(
                    "Disk {:?} total space: {:.2} GiB, available space: {:.2} GiB",
                    name, total, available,
                ))
            }
        }

        if let Some(cpu_usage) = self.cpu_usage {
            v.push(format!("CPU usage: {:.1}%", cpu_usage * 100.0))
        }

        v
    }
}
