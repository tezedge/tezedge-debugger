// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use chrono::{DateTime, Utc, Duration};

pub trait StatsSource {
    fn timestamp(&self) -> DateTime<Utc>;
    fn memory_usage(&self) -> u64;
    fn memory_cache(&self) -> u64;
    fn container_cpu_usage(&self) -> Duration;
    fn total_cpu_usage(&self) -> Duration;
    fn last_container_cpu_usage(&self) -> Duration;
    fn last_total_cpu_usage(&self) -> Duration;
    fn num_processors(&self) -> usize;
}
