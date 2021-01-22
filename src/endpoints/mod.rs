// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

pub mod p2p;
pub mod rpc;
pub mod log;
pub mod stat;
mod version;

use warp::{
    Filter, Reply,
    reject::Rejection,
    reply::with::header,
};
use crate::storage::MessageStore;
use crate::system::BpfSniffer;
use crate::endpoints::p2p::{p2p, p2p_report};
use crate::endpoints::rpc::rpc;
use crate::endpoints::log::log;
use crate::endpoints::stat::stat;

/// Create router for consisting of all endpoint
pub fn routes(storage: MessageStore, sniffer: BpfSniffer) -> impl Filter<Extract=impl Reply, Error=Rejection> + Clone + Sync + Send + 'static {
    warp::get().and(
        p2p(storage.clone())
            .or(p2p_report(sniffer))
            .or(rpc(storage.clone()))
            .or(log(storage.clone()))
            .or(stat(storage.clone()))
            .or(self::version::api_call())
    )
        .with(header("Content-Type", "application/json"))
        .with(header("Access-Control-Allow-Origin", "*"))
}
