use crate::storage::MessageStore;
use warp::{
    Filter, Reply, Rejection,
    reply::{with_status, json},
    http::StatusCode,
};
use serde::{Serialize, Deserialize};
use warp::reply::{WithStatus, Json};

#[derive(Default, Debug, Copy, Clone, Serialize, Deserialize)]
pub struct RpcQuery {
    offset_id: Option<u64>,
    count: Option<usize>,
}

pub fn rpc(storage: MessageStore) -> impl Filter<Extract=impl Reply, Error=Rejection> + Clone + Sync + Send + 'static {
    warp::path!("v2" / "rpc")
        .and(warp::query::query())
        .map(move |query: RpcQuery| -> WithStatus<Json> {
            match storage.rpc().get_range(query.offset_id.unwrap_or(0), query.count.unwrap_or(100) as u64) {
                Ok(msgs) => with_status(json(&msgs), StatusCode::OK),
                Err(err) => with_status(json(&format!("database error: {}", err)), StatusCode::INTERNAL_SERVER_ERROR),
            }
        })
}