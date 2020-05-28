use crate::storage::{MessageStore, P2PFilters};
use warp::{
    Filter, Rejection,
    reply::{with_status, json},
    http::StatusCode,
};
use serde::{Serialize, Deserialize};
use warp::reply::{WithStatus, Json};
use std::net::SocketAddr;

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct P2PCursor {
    cursor_id: Option<u64>,
    limit: Option<usize>,
    remote_addr: Option<SocketAddr>,
    types: Option<Vec<String>>,
    request_id: Option<u64>,
    incoming: Option<bool>,
}

impl P2PCursor {
    fn convert_types(&self) -> Option<u32> {
        Some(0)
    }
}

impl Into<crate::storage::P2PFilters> for P2PCursor {
    fn into(self) -> P2PFilters {
        P2PFilters {
            remote_addr: self.remote_addr,
            types: self.convert_types(),
            request_id: self.request_id,
            incoming: self.incoming,
        }
    }
}

pub fn p2p(storage: MessageStore) -> impl Filter<Extract=(WithStatus<Json>, ), Error=Rejection> + Clone + Sync + Send + 'static {
    warp::path!("v2" / "p2p")
        .and(warp::body::json())
        .map(move |cursor: P2PCursor| -> WithStatus<Json> {
            let cursor_id = cursor.cursor_id;
            let limit = cursor.limit.unwrap_or(100);
            match storage.p2p().get_cursor(cursor_id, limit, cursor.into()) {
                Ok(msgs) => with_status(json(&msgs), StatusCode::OK),
                Err(err) => with_status(json(&format!("database error: {}", err)), StatusCode::INTERNAL_SERVER_ERROR),
            }
        })
}