use crate::storage::{
    {MessageStore, P2PFilters},
    p2p_indexes::{ParseTypeError, Type},
};
use warp::{
    Filter, Rejection,
    reply::{with_status, json},
    http::StatusCode,
};
use serde::{Serialize, Deserialize};
use warp::reply::{WithStatus, Json};
use std::net::SocketAddr;
use std::convert::TryInto;
use itertools::Itertools;
// use storage::StorageError;

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct P2PCursor {
    cursor_id: Option<u64>,
    limit: Option<usize>,
    remote_addr: Option<SocketAddr>,
    types: Option<String>,
    request_id: Option<u64>,
    incoming: Option<bool>,
}

impl P2PCursor {
    fn get_types(&self) -> Result<Option<u32>, ParseTypeError> {
        if let Some(ref values) = self.types {
            let mut ret = 0u32;
            for r#type in values.split(',').next() {
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

impl TryInto<crate::storage::P2PFilters> for P2PCursor {
    type Error = ParseTypeError;

    fn try_into(self) -> Result<P2PFilters, Self::Error> {
        Ok(P2PFilters {
            remote_addr: self.remote_addr,
            types: self.get_types()?,
            request_id: self.request_id,
            incoming: self.incoming,
        })
    }
}

pub fn p2p(storage: MessageStore) -> impl Filter<Extract=(WithStatus<Json>, ), Error=Rejection> + Clone + Sync + Send + 'static {
    warp::path!("v2" / "p2p")
        .and(warp::query::query())
        .map(move |cursor: P2PCursor| -> WithStatus<Json> {
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

pub fn types(storage: MessageStore) -> impl Filter<Extract=(WithStatus<Json>, ), Error=Rejection> + Clone + Sync + Send + 'static {
    warp::path!("types"/ u64 / u32)
        .map(move |index: u64, types: u32| -> WithStatus<Json> {
            match storage.p2p().type_iterator(Some(index), types) {
                Ok(values) => {
                    with_status(json(&values.collect_vec()), StatusCode::OK)
                }
                Err(err) => with_status(json(&format!("database error: {}", err)), StatusCode::INTERNAL_SERVER_ERROR),
            }
        })
}