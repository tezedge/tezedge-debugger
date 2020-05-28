pub mod p2p;
pub mod rpc;

use warp::{
    Filter, Reply,
    reject::Rejection,
    reply::with::header,
};
use crate::storage::MessageStore;
use crate::endpoints::p2p::p2p;

pub fn routes(storage: MessageStore) -> impl Filter<Extract=impl Reply, Error=Rejection> + Clone + Sync + Send + 'static {
    warp::get().and(
        p2p(storage.clone())
    )
        .with(header("Content-Type", "application/json"))
        .with(header("Access-Control-Allow-Origin", "*"))
}