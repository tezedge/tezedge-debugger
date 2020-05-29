use crate::storage::{
    {MessageStore, P2PFilters},
    p2p_secondary_indexes::{ParseTypeError, Type},
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
    fn get_types(&self) -> Result<Option<u32>, ParseTypeError> {
        if let Some(ref values) = self.types {
            let mut ret: u32 = 0;

            for value in values {
                let parsed: Type = value.parse()?;
                ret |= parsed as u32;
            }

            Ok(Some(ret))
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
        .and(warp::body::json())
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