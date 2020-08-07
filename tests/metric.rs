// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

pub mod common;

use common::{debugger_url, get_rpc_as_json};

const V2_ENDPOINT: &str = "v2/metric";

#[tokio::test]
async fn test_metrics() {
    use chrono::{DateTime, Utc, TimeZone};

    let debugger_url = debugger_url();
    let base_endpoint = format!("{}/{}", debugger_url, V2_ENDPOINT);
    let response = get_rpc_as_json(base_endpoint.as_str())
        .await.unwrap();

    let mut last_time: Option<DateTime<Utc>> = None;
    for stat in response.as_array().unwrap() {
        let stat_all = stat.as_object().unwrap();
        let stat = stat_all.get("container_stat").unwrap().as_object().unwrap();
        let time = stat.get("read").unwrap().as_str().unwrap();
        let this_time = Utc.datetime_from_str(&time, "%Y-%m-%dT%H:%M:%S.%fZ").unwrap();
        if let Some(last_time) = last_time {
            assert!(last_time >= this_time, "{} >= {}", last_time, this_time);
        }
        last_time = Some(this_time);
        let mem = stat.get("memory_stats").unwrap().as_object().unwrap();
        let usage = mem.get("usage").unwrap().as_u64().unwrap();
        let cache = mem.get("stats").unwrap().as_object().unwrap().get("cache").unwrap().as_u64().unwrap();
        
        let process_stat = stat_all.get("process_stats").unwrap().as_array().unwrap();
        let usage_sum = process_stat.iter()
            .fold(0, |usage, process| {
                usage + process.get("memory_usage").unwrap().as_u64().unwrap()
            });
        
        let _ = (usage - cache, usage_sum);
    }
}
