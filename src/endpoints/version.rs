// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use warp::{
    Filter, Rejection,
    reply::{with_status, json, WithStatus, Json},
    http::StatusCode,
};

pub fn api_call() -> impl Filter<Extract=(WithStatus<Json>, ), Error=Rejection> + Clone + Sync + Send + 'static {
    warp::path!("v2" / "version")
        .and(warp::query::query())
        .map(move |()| -> WithStatus<Json> {
            with_status(json(&env!("GIT_HASH")), StatusCode::OK)
        })
}
