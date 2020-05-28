use warp::{
    Filter, Reply,
    reply::{Json, WithStatus, json, with_status, with::header},
    reject::Rejection, http::status::StatusCode,
};
use serde::{Serialize, Deserialize};
use crate::storage::Storage;

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct P2PQuery {
    offset_id: Option<u64>,
    count: Option<usize>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct RPCQuery {
    offset_id: Option<u64>,
    count: Option<usize>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct LogQuery {
    offset_id: Option<u64>,
    count: Option<usize>,
}

fn v2_p2p(storage: Storage) -> impl Filter<Extract=impl Reply, Error=Rejection> + Clone + Sync + Send + 'static {
    warp::path!("v2" / "p2p")
        .and(warp::query::query())
        .map(move |query: P2PQuery| -> WithStatus<Json> {
            match storage.p2p_store().get_range(query.offset_id, query.count.unwrap_or(100)) {
                Ok(msg) => {
                    with_status(json(&msg), StatusCode::OK)
                }
                Err(err) => {
                    with_status(json(&format!("database error: {}", err)), StatusCode::INTERNAL_SERVER_ERROR)
                }
            }
        })
}

fn v2_rpc(storage: Storage) -> impl Filter<Extract=impl Reply, Error=Rejection> + Clone + Sync + Send + 'static {
    warp::path!("v2" / "rpc")
        .and(warp::query::query())
        .map(move |query: RPCQuery| -> WithStatus<Json> {
            match storage.rpc_store().get_range(query.offset_id, query.count.unwrap_or(100)) {
                Ok(msg) => {
                    with_status(json(&msg), StatusCode::OK)
                }
                Err(err) => {
                    with_status(json(&format!("database error: {}", err)), StatusCode::INTERNAL_SERVER_ERROR)
                }
            }
        })
}

pub async fn routes(storage: Storage) -> impl Filter<Extract=impl Reply, Error=Rejection> + Clone + Sync + Send + 'static {
    warp::get().and(
        v2_p2p(storage.clone()).or(v2_rpc(storage.clone()))
    )
        .with(header("Content-Type", "application/json"))
        .with(header("Access-Control-Allow-Origin", "*"))
}
