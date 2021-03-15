use warp::{Filter, Rejection, reply::{with_status, WithStatus, Json}, http::StatusCode};

use std::sync::{Arc, Mutex};
use crate::system::Reporter;

/// Basic handler for p2p message endpoint with cursor
pub fn p2p_report(reporter: Arc<Mutex<Reporter>>) -> impl Filter<Extract=(WithStatus<Json>, ), Error=Rejection> + Clone + Sync + Send + 'static {
    warp::path!("v2" / "p2p_summary")
        .and(warp::query::query())
        .map(move |()| -> WithStatus<Json> {
            let report = tokio::task::block_in_place(|| futures::executor::block_on(reporter.lock().unwrap().get_p2p_report()));
            with_status(report, StatusCode::OK)
        })
}
