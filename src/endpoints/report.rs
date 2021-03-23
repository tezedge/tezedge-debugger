use warp::{Filter, Rejection, reply::{with_status, WithStatus, Json, json}, http::StatusCode};

use std::sync::{Arc, Mutex};
use crate::{system::Reporter, storage_::{PerfStore, perf::Report}};

pub fn p2p_report(reporter: Arc<Mutex<Reporter>>) -> impl Filter<Extract=(WithStatus<Json>, ), Error=Rejection> + Clone + Sync + Send + 'static {
    warp::path!("v2" / "p2p_summary")
        .and(warp::query::query())
        .map(move |()| -> WithStatus<Json> {
            let report = tokio::task::block_in_place(|| futures::executor::block_on(reporter.lock().unwrap().get_p2p_report()));
            with_status(report, StatusCode::OK)
        })
}

pub fn rb_report(reporter: Arc<Mutex<Reporter>>) -> impl Filter<Extract=(WithStatus<Json>, ), Error=Rejection> + Clone + Sync + Send + 'static {
    warp::path!("v2" / "rb")
        .and(warp::query::query())
        .map(move |()| -> WithStatus<Json> {
            let report = tokio::task::block_in_place(|| futures::executor::block_on(reporter.lock().unwrap().get_counter()));
            with_status(json(&report), StatusCode::OK)
        })
}

pub fn perf_report(perf_db: PerfStore) -> impl Filter<Extract=(WithStatus<Json>, ), Error=Rejection> + Clone + Sync + Send + 'static {
    warp::path!("v2" / "perf")
        .and(warp::query::query())
        .map(move |()| -> WithStatus<Json> {
            let msgs = perf_db.get_all().unwrap();
            let report = Report::try_new(&msgs);
            with_status(json(&report), StatusCode::OK)
        })
}
