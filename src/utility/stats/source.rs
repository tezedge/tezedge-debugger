// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use chrono::{DateTime, Utc, Duration};

pub trait StatSource {
    fn timestamp(&self) -> DateTime<Utc>;
    fn memory_usage(&self) -> u64;
    fn memory_cache(&self) -> u64;
    fn container_cpu_usage(&self) -> Duration;
    fn total_cpu_usage(&self) -> Duration;
    fn last_container_cpu_usage(&self) -> Duration;
    fn last_total_cpu_usage(&self) -> Duration;
    fn num_processors(&self) -> usize;
}

pub trait ProcessStatSource {
    fn timestamp(&self) -> DateTime<Utc>;
    fn process_cmd(&self) -> &str;
    fn memory_usage(&self) -> u64;
    fn memory_cache(&self) -> u64;
}
