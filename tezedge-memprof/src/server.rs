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

pub fn run<T>(
    reporter: Arc<Mutex<T>>,
    resolver: Arc<RwLock<StackResolver>>,
    pid: Arc<AtomicU32>,
) -> (tokio::task::JoinHandle<()>, tokio::runtime::Runtime)
where
    T: Reporter + Send + 'static,
{
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let server = routes(reporter, resolver, pid.clone());
    let handler = runtime.spawn(warp::serve(server).run(([0, 0, 0, 0], 17832)));
    (handler, runtime)
}

fn routes<T>(
    reporter: Arc<Mutex<T>>,
    resolver: Arc<RwLock<StackResolver>>,
    pid: Arc<AtomicU32>,
) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone + Sync + Send + 'static
where
    T: Reporter + Send + 'static,
{
    use warp::reply::with;

    warp::get()
        .and(tree(reporter, resolver, pid.clone()).or(get_pid(pid)).or(openapi()))
        .with(with::header("Content-Type", "application/json"))
        .with(with::header("Access-Control-Allow-Origin", "*"))
}

fn get_pid(p: Arc<AtomicU32>) -> impl Filter<Extract = (WithStatus<Json>,), Error = Rejection> + Clone + Sync + Send + 'static {
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
    pid: Arc<AtomicU32>,
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
        total: u64,
        cache: u64,
        anon: u64,
        system_report_anon: u64,
    }

    warp::path!("v1" / "tree")
        .and(warp::query::query())
        .map(move |params: Params| -> WithStatus<Json> {
            let resolver = resolver.read().unwrap();
            let history = history.lock().unwrap();
            if params.short.unwrap_or(false) {
                let (total, cache) = history.short_report();
                let system_report_anon = rss_anon(pid.clone()).unwrap_or(0);
                let report = ShortReport {
                    total,
                    cache,
                    anon: total - cache,
                    system_report_anon,
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

pub fn openapi() -> impl Filter<Extract=(WithStatus<Json>, ), Error=Rejection> + Clone + Sync + Send + 'static {
    warp::path!("openapi" / "memory-profiler-openapi.json")
        .and(warp::query::query())
        .map(move |()| -> reply::WithStatus<Json> {
            let s = include_str!("../openapi.json");
            let d = serde_json::from_str::<serde_json::Value>(s).unwrap();
            reply::with_status(
                reply::json(&d),
                StatusCode::OK,
            )
        })
}
