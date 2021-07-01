// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::{Arc, atomic::{Ordering, AtomicU32}, Mutex, RwLock};
use warp::{
    Filter, Rejection, Reply,
    reply::{WithStatus, Json, self},
    http::StatusCode,
};
use serde::Deserialize;
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
        .and(tree(history, resolver).or(pid(p)))
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

fn tree<T>(
    history: Arc<Mutex<T>>,
    resolver: Arc<RwLock<StackResolver>>,
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

    warp::path!("v1" / "tree")
        .and(warp::query::query())
        .map(move |params: Params| -> WithStatus<Json> {
            let resolver = resolver.read().unwrap();
            let history = history.lock().unwrap();
            if params.short.unwrap_or(false) {
                let (value, cache_value) = history.short_report();
                let report = value - cache_value;
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
