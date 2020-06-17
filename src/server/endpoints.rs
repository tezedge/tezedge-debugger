use warp::{
    Filter, Reply,
    reply::{Json, WithStatus, json, with_status, with::header},
    reject::Rejection, http::status::StatusCode,
};
use serde::{Serialize, Deserialize};
use crate::storage::{MessageStore, P2pFilters};
use std::net::SocketAddr;
use crate::messages::p2p_message::SourceType;
use crate::storage::secondary_indexes::{ParseTypeError, Type};
use std::convert::TryInto;

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct P2pCursor {
    cursor_id: Option<u64>,
    limit: Option<usize>,
    remote_addr: Option<SocketAddr>,
    types: Option<String>,
    request_id: Option<u64>,
    incoming: Option<bool>,
    source_type: Option<SourceType>,
}

impl P2pCursor {
    fn get_types(&self) -> Result<Option<u32>, ParseTypeError> {
        if let Some(ref values) = self.types {
            let mut ret = 0u32;
            for r#type in values.split(',') {
                let r#type: Type = r#type.parse()?;
                ret |= r#type as u32;
            }
            if ret == 0 {
                Ok(None)
            } else {
                Ok(Some(ret))
            }
        } else {
            Ok(None)
        }
    }
}

impl TryInto<P2pFilters> for P2pCursor {
    type Error = ParseTypeError;

    fn try_into(self) -> Result<P2pFilters, Self::Error> {
        Ok(P2pFilters {
            source_type: self.source_type.map(|st| st.as_bool()),
            remote_addr: self.remote_addr,
            types: self.get_types()?,
            request_id: self.request_id,
            incoming: self.incoming,
        })
    }
}

fn v2_p2p(storage: MessageStore) -> impl Filter<Extract=(WithStatus<Json>, ), Error=Rejection> + Clone + Sync + Send + 'static {
    warp::path!("v2" / "p2p")
        .and(warp::query::query())
        .map(move |cursor: P2pCursor| -> WithStatus<Json> {
            let limit = cursor.limit.unwrap_or(100);
            let cursor_id = cursor.cursor_id.clone();
            match cursor.try_into() {
                Ok(filters) => match storage.p2p().get_cursor(cursor_id, limit, filters) {
                    Ok(msgs) => with_status(json(&msgs), StatusCode::OK),
                    Err(err) => with_status(json(&format!("database error: {}", err)), StatusCode::INTERNAL_SERVER_ERROR),
                },
                Err(type_err) => with_status(json(&format!("invalid type-name: {}", type_err)), StatusCode::BAD_REQUEST),
            }
        })
}

pub fn routes(storage: MessageStore) -> impl Filter<Extract=impl Reply, Error=Rejection> + Clone + Sync + Send + 'static {
    v2_p2p(storage.clone())
        .with(header("Content-Type", "application/json"))
        .with(header("Access-Control-Allow-Origin", "*"))
}