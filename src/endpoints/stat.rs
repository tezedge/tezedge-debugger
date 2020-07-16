use crate::storage::MessageStore;
use warp::{Filter, Rejection};
use warp::reply::{WithStatus, Json, with_status, json};
use warp::http::StatusCode;
use crate::system::orchestrator::{CONNECTIONS};
use itertools::Itertools;

pub fn stat(storage: MessageStore) -> impl Filter<Extract=(WithStatus<Json>, ), Error=Rejection> + Clone + Sync + Send + 'static {
    warp::path!("v2" / "stat")
        .map(move || {
            with_status(json(&storage.stat().snapshot()), StatusCode::OK)
        })
}

pub fn network(_: MessageStore) -> impl Filter<Extract=(WithStatus<Json>, ), Error=Rejection> + Clone + Sync + Send + 'static {
    warp::path!("v2" / "network")
        .map(move || {
            let data = if let Ok(lock) = CONNECTIONS.read() {
                lock.values()
                    .filter_map(|x| if let Some(value) = x {
                        Some(value.clone())
                    } else {
                        None
                    }).collect_vec()
            } else { Default::default() };
            with_status(json(&data), StatusCode::OK)
        })
}