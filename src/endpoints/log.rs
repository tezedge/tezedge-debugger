// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::convert::TryInto;
use serde::{Serialize, Deserialize};
use failure::Error;
use warp::{
    Filter, Reply, Rejection,
    reply::{with_status, json, WithStatus, Json},
    http::StatusCode,
};
use crate::storage_::{LogStore, log::Filters, indices::{LogLevel, ParseLogLevelError, NodeName}};

/// Cursor structure mapped from the endpoint URI
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct LogCursor {
    pub cursor_id: Option<u64>,
    pub limit: Option<usize>,
    pub level: Option<String>,
    pub timestamp: Option<String>,
    pub node_name: Option<u16>,
}

impl LogCursor {
    /// Parse given timestamp as UNIX timestamp
    fn get_timestamp(&self) -> Result<Option<u128>, Error> {
        if let Some(ref ts) = self.timestamp {
            Ok(Some(ts.parse()?))
        } else {
            Ok(None)
        }
    }

    /// Parse given log level as an database-understandable value
    fn get_level(&self) -> Result<Vec<LogLevel>, ParseLogLevelError> {
        if let Some(ref level) = self.level {
            let mut ret = vec![];
            for l in level.split(',') {
                ret.push(l.parse()?);
            }
            Ok(ret)
        } else {
            Ok(vec![])
        }
    }
}

impl TryInto<Filters> for LogCursor {
    type Error = Error;

    fn try_into(self) -> Result<Filters, Self::Error> {
        Ok(Filters {
            log_level: self.get_level()?,
            date: self.get_timestamp()?,
            node_name: self.node_name.map(NodeName),
        })
    }
}

/// Basic handler for log endpoint with cursor
pub fn log(storage: LogStore) -> impl Filter<Extract=impl Reply, Error=Rejection> + Clone + Sync + Send + 'static {
    warp::path!("v2" / "log")
        .and(warp::query::query())
        .map(move |cursor: LogCursor| -> WithStatus<Json> {
            let limit = cursor.limit.unwrap_or(100);
            let cursor_id = cursor.cursor_id.clone();
            match cursor.try_into() {
                Ok(filters) => match storage.get_cursor(cursor_id, limit, &filters) {
                    Ok(msgs) => {
                        with_status(json(&msgs), StatusCode::OK)
                    },
                    Err(err) => with_status(json(&format!("database error: {}", err)), StatusCode::INTERNAL_SERVER_ERROR),
                },
                Err(level_error) => with_status(json(&format!("invalid type-name: {}", level_error)), StatusCode::BAD_REQUEST),
            }
        })
}