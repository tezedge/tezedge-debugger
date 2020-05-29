use crate::storage::{MessageStore, RpcFilters};
use warp::{
    Filter, Reply, Rejection,
    reply::{with_status, json},
    http::StatusCode,
};
use serde::{Serialize, Deserialize};
use warp::reply::{WithStatus, Json};
use std::net::SocketAddr;

#[derive(Default, Debug, Copy, Clone, Serialize, Deserialize)]
pub struct RpcCursor {
    pub cursor_id: Option<u64>,
    pub limit: Option<usize>,
    pub remote_addr: Option<SocketAddr>,
}

impl Into<crate::storage::RpcFilters> for RpcCursor {
    fn into(self) -> RpcFilters {
        RpcFilters {
            remote_addr: self.remote_addr,
        }
    }
}

pub fn rpc(storage: MessageStore) -> impl Filter<Extract=impl Reply, Error=Rejection> + Clone + Sync + Send + 'static {
    warp::path!("v2" / "rpc")
        .and(warp::body::json())
        .map(move |cursor: RpcCursor| -> WithStatus<Json> {
            let limit = cursor.limit.unwrap_or(100);
            let cursor_id = cursor.cursor_id.clone();
            match storage.rpc().get_cursor(cursor_id, limit, cursor.into()) {
                Ok(msgs) => with_status(json(&msgs), StatusCode::OK),
                Err(err) => with_status(json(&format!("database error: {}", err)), StatusCode::INTERNAL_SERVER_ERROR),
            }
        })
}