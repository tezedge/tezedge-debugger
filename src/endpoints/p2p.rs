// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::{
    storage::{
        {MessageStore, P2pFilters},
        p2p_indexes::{ParseTypeError, Type},
    },
    system::Reporter,
};
use warp::{
    Filter, Rejection,
    reply::{with_status, json},
    http::StatusCode,
};
use serde::{Serialize, Deserialize};
use warp::reply::{WithStatus, Json};
use std::{
    net::SocketAddr,
    convert::TryInto,
    sync::{Arc, Mutex},
};
use itertools::Itertools;
use crate::messages::p2p_message::SourceType;

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
/// Cursor structure mapped from the endpoint URI
pub struct P2pCursor {
    cursor_id: Option<u64>,
    limit: Option<usize>,
    remote_addr: Option<SocketAddr>,
    types: Option<String>,
    request_id: Option<u64>,
    incoming: Option<bool>,
    source_type: Option<SourceType>,
    node_name: Option<u16>,
}

impl P2pCursor {
    /// Parse given list of types as bit-flag
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
            source_type: self.source_type.map(|st| match st {
                SourceType::Local => true,
                SourceType::Remote => false,
            }),
            remote_addr: self.remote_addr,
            types: self.get_types()?,
            request_id: self.request_id,
            incoming: self.incoming,
            node_name: self.node_name,
        })
    }
}

/// Basic handler for p2p message endpoint with cursor
pub fn p2p(storage: MessageStore) -> impl Filter<Extract=(WithStatus<Json>, ), Error=Rejection> + Clone + Sync + Send + 'static {
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

/// Basic handler for p2p message endpoint with cursor
pub fn p2p_report(reporter: Arc<Mutex<Reporter>>) -> impl Filter<Extract=(WithStatus<Json>, ), Error=Rejection> + Clone + Sync + Send + 'static {
    warp::path!("v2" / "p2p_summary")
        .and(warp::query::query())
        .map(move |()| -> WithStatus<Json> {
            let report = tokio::task::block_in_place(|| futures::executor::block_on(reporter.lock().unwrap().get_p2p_report()));

            with_status(json(&report), StatusCode::OK)
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