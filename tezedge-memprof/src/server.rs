// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    sync::{Arc, atomic::{Ordering, AtomicU32}, Mutex, RwLock},
    fs::File,
    io::{Error, BufReader, BufRead},
};
use warp::{
    Filter, Rejection, Reply,
    reply::{WithStatus, Json, self},
    http::StatusCode,
};
use serde::{Serialize, Deserialize};
use super::{StackResolver, Reporter};

pub fn routes<T>(
    history: Arc<Mutex<T>>,
    resolver: Arc<RwLock<StackResolver>>,
    p: Arc<AtomicU32>,
) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone + Sync + Send + 'static
where
    T: Reporter + Send + 'static,
{
    use warp::reply::with;

    warp::get()
        .and(tree(history, resolver, p.clone()).or(pid(p)))
        .with(with::header("Content-Type", "application/json"))
        .with(with::header("Access-Control-Allow-Origin", "*"))
}

fn pid(p: Arc<AtomicU32>) -> impl Filter<Extract = (WithStatus<Json>,), Error = Rejection> + Clone + Sync + Send + 'static {
    warp::path!("v1" / "pid")
        .and(warp::query::query())
        .map(move |()| -> WithStatus<Json> {
            reply::with_status(reply::json(&p.load(Ordering::Relaxed)), StatusCode::OK)
        })
}

fn rss_anon(p: Arc<AtomicU32>) -> Result<u64, Error> {
    let pid = p.load(Ordering::Relaxed);
    let f = File::open(format!("/proc/{}/status", pid))?;
    let reader = BufReader::new(f);
    let mut v = 0;
    for line in reader.lines() {
        let line = line?;
        let mut words = line.split_whitespace();
        if let Some("RssAnon:") = words.next() {
            v = words.next().map(|s| s.parse().unwrap_or(0)).unwrap_or(0);
        } else {
            continue;
        }
    }

    Ok(v)
}

fn tree<T>(
    history: Arc<Mutex<T>>,
    resolver: Arc<RwLock<StackResolver>>,
    p: Arc<AtomicU32>,
) -> impl Filter<Extract = (WithStatus<Json>,), Error = Rejection> + Clone + Sync + Send + 'static
where
    T: Reporter + Send + 'static,
{
    #[derive(Deserialize)]
    struct Params {
        threshold: Option<u64>,
        reverse: Option<bool>,
        short: Option<bool>,
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct ShortReport {
        usage_total: u64,
        usage: u64,
        usage_cache: u64,
        rss_anon: u64,
    }

    warp::path!("v1" / "tree")
        .and(warp::query::query())
        .map(move |params: Params| -> WithStatus<Json> {
            let resolver = resolver.read().unwrap();
            let history = history.lock().unwrap();
            if params.short.unwrap_or(false) {
                let (usage_total, usage_cache) = history.short_report();
                let rss_anon = rss_anon(p.clone()).unwrap_or(0);
                let report = ShortReport {
                    usage_total,
                    usage: usage_total - usage_cache,
                    usage_cache,
                    rss_anon,
                };
                reply::with_status(reply::json(&report), StatusCode::OK)
            } else {
                let report = history.tree_report(
                    resolver,
                    params.threshold.unwrap_or(512),
                    params.reverse.unwrap_or(false),
                );
                reply::with_status(reply::json(&report), StatusCode::OK)
            }
        })
}
