use thiserror::Error;
use serde_json::{value, error};

#[derive(Debug, Error)]
enum E {
    #[error("serde: {}", _0)]
    Serde(serde_json::error::Error),
    #[error("bad json")]
    BadJson,
}

async fn get(path: &str, params: &str) -> Result<value::Value, error::Error> {
    use std::env;

    let url = env::var("URL").unwrap();
    let res = reqwest::get(&format!("{}{}/?{}", url, path, params)).await.unwrap().text().await.unwrap();
    serde_json::from_str(&res)
}

async fn get_usage_kib() -> Result<i64, E> {
    let tree = get("/v1/tree", "").await.map_err(E::Serde)?;
    let tree = tree.as_object().ok_or(E::BadJson)?;
    let value = tree.get("value").ok_or(E::BadJson)?.as_i64().ok_or(E::BadJson)?;
    let cache_value = tree.get("cacheValue").ok_or(E::BadJson)?.as_i64().ok_or(E::BadJson)?;
    Ok(value - cache_value)
}

async fn get_pid() -> Result<u32, E> {
    let pid = get("/v1/pid", "").await.map_err(E::Serde)?;
    let pid = pid.as_i64().ok_or(E::BadJson)?;
    Ok(pid as _)
}

#[tokio::test]
async fn compare() {
    use std::{fs::File, io::{BufRead, BufReader}};

    let pid = get_pid().await.unwrap();
    let usage_kib = get_usage_kib().await.unwrap() as i32;

    let f = File::open(format!("/proc/{}/status", pid)).expect("no such process");
    let usage_system_kib = BufReader::new(f).lines()
        .find_map(|line| {
            let line = line.ok()?;
            let mut words = line.split_whitespace();
            if let Some("RssAnon:") = words.next() {
                words.next().map(|s| s.parse().unwrap_or(0))
            } else {
                None
            }
        })
        .unwrap_or(0);

    let diff = ((usage_kib - usage_system_kib) as f64) / (usage_system_kib as f64);
    if diff.abs() < 0.1 {
        println!("system report: {}, memprof report: {}, difference: {:.2}%", usage_system_kib, usage_kib, diff * 100.0);
    } else {
        panic!("system report: {}, memprof report: {}, difference: {:.2}%", usage_system_kib, usage_kib, diff * 100.0);
    }
}
