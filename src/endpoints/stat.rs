use crate::storage::MessageStore;
use warp::{Filter, Rejection};
use warp::reply::{WithStatus, Json, with_status, json};
use warp::http::StatusCode;

pub fn stat(storage: MessageStore) -> impl Filter<Extract=(WithStatus<Json>, ), Error=Rejection> + Clone + Sync + Send + 'static {
    warp::path!("v2" / "stat")
        .map(move || {
            with_status(json(&storage.stat().snapshot()), StatusCode::OK)
        })
}