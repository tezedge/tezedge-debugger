// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use sysinfo::{System, SystemExt, DiskExt};
use chrono::{DateTime, Utc, Duration};
use crate::messages::metric_message::MetricMessage;

/// Configuration of alert, conditions that trigger the alert
#[derive(Clone)]
pub struct AlertConfig {
    /// mount point of disk where database stored
    // needed for disk exhausting estimation
    pub db_mount_point: String,
}

impl AlertConfig {
    pub fn condition_checker(&self) -> SystemCapacityObserver {
        let system = {
            use sysinfo::RefreshKind;

            let r = RefreshKind::new().with_disks_list().with_disks().with_memory().with_cpu();
            let mut s = System::new_with_specifics(r);
            s.refresh_disks_list();
            s.refresh_disks();
            s.refresh_memory();
            s.refresh_cpu();
            s
        };
        let disk_index = system.get_disks().iter().enumerate()
            .find(|&d| d.1.get_mount_point().to_str() == Some(self.db_mount_point.as_str()))
            .map(|(x, _)| x);
        SystemCapacityObserver {
            system: system,
            memory: MemoryEstimator::new(),
            disk_index: disk_index,
            _disk: DiskEstimator::new(),
            cpu: CpuUsage::None,
            cpu_usage_literal: None,
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
        MemoryEstimator {
            usage: Vec::new(),
        }
    }

    pub fn observe(&mut self, timestamp: DateTime<Utc>, usage: u64) {
        // TODO:
        self.usage.push((timestamp, usage));
    }

    pub fn estimate(&self, _available: u64) -> Option<Duration> {
        // TODO:
        None
    }

    pub fn status(&self) -> u64 {
        self.usage.last().cloned().unwrap_or((Utc::now(), 0)).1
    }
}

pub enum CpuUsage {
    None,
    One((DateTime<Utc>, Duration)),
    Two {
        pre_last: (DateTime<Utc>, Duration),
        last: (DateTime<Utc>, Duration),
    },
}

impl CpuUsage {
    pub fn observe(&mut self, timestamp: DateTime<Utc>, usage: Duration) {
        *self = match *self {
            CpuUsage::None => CpuUsage::One((timestamp, usage)),
            CpuUsage::One(pre_last) => CpuUsage::Two {
                pre_last,
                last: (timestamp, usage),
            },
            CpuUsage::Two { pre_last: _, last} => CpuUsage::Two {
                pre_last: last,
                last: (timestamp, usage),
            },
        }
    }

    pub fn status(&self) -> Option<f64> {
        match self {
            &CpuUsage::None | &CpuUsage::One(..) => None,
            &CpuUsage::Two {
                pre_last: (fst_timestamp, fst_duration),
                last: (scd_timestamp,scd_duration),
            } => {
                let duration_total = scd_timestamp - fst_timestamp;
                let duration_used = scd_duration - fst_duration;
                let nanoseconds = |d: Duration| -> f64 {
                    d.num_nanoseconds().unwrap_or(1) as f64
                };
                Some(nanoseconds(duration_used) / nanoseconds(duration_total))
            },
        }
    }
}

pub struct SystemCapacityObserver {
    system: System,
    memory: MemoryEstimator,
    disk_index: Option<usize>,
    _disk: DiskEstimator,
    cpu: CpuUsage,
    cpu_usage_literal: Option<f64>,
}

impl SystemCapacityObserver {
    pub fn observe(&mut self, message: &MetricMessage) {
        let timestamp = message.timestamp();
        self.memory.observe(timestamp, message.memory_used());
        match message {
            &MetricMessage::Cadvisor(ref stats) => {
                self.cpu.observe(timestamp, Duration::nanoseconds(stats.cpu.usage.total.clone() as i64));        
            },
            &MetricMessage::Docker(ref stats) => {
                let cpu_delta = stats.cpu_stats.cpu_usage.total_usage - stats.precpu_stats.cpu_usage.total_usage;
                let system_cpu_delta = stats.cpu_stats.system_cpu_usage - stats.precpu_stats.system_cpu_usage;
                self.cpu_usage_literal = Some((cpu_delta as f64 / system_cpu_delta as f64) * stats.cpu_stats.online_cpus as f64);
            },
        }
        self.system.refresh_disks();
        self.system.refresh_memory();
        self.system.refresh_cpu();
    }

    pub fn alert(&self) -> Vec<String> {
        let mut alerts = Vec::new();
        if let Some(estimate) = self.memory.estimate(self.system.get_available_memory()) {
            if estimate < Duration::minutes(10) {
                alerts.push(format!("Warning, memory will exhaust estimated in {}", estimate));
            }
        }

        alerts
    }

    pub fn status(&self) -> Vec<String> {
        let mut v = Vec::new();
        let gb = |x| (x as f64) / (0x40000000 as f64);

        let memory = self.memory.status();
        v.push(
            format!(
                "Memory used: {:.2} GiB, free: {:.2} GiB",
                gb(memory),
                gb(self.system.get_free_memory() * 1024),
            ),
        );

        if let Some(disk_index) = self.disk_index {
            if let Some(disk) = self.system.get_disks().get(disk_index) {
                let name = disk.get_name();
                let total = gb(disk.get_total_space());
                let available = gb(disk.get_available_space());
                v.push(
                    format!(
                        "Disk {:?} total space: {:.2} GiB, available space: {:.2} GiB",
                        name,
                        total,
                        available,
                    ),
                )
            }
        }

        if let Some(cpu_usage) = self.cpu.status() {
            v.push(format!("CPU usage: {:.1}%", cpu_usage * 100.0))
        }

        if let Some(cpu_usage) = self.cpu_usage_literal {
            v.push(format!("CPU usage: {:.1}%", cpu_usage * 100.0))
        }

        v
    }
}
