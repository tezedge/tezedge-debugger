
use std::sync::Arc;
use warp::{Filter, Rejection, Reply, reply::{WithStatus, Json, self}, http::StatusCode};
use super::database::{DatabaseFetch, ConnectionsFilter, MessagesFilter};

fn connections<Db>(db: Arc<Db>) -> impl Filter<Extract=(WithStatus<Json>, ), Error=Rejection> + Clone + Sync + Send + 'static
where
    Db: DatabaseFetch + Sync + Send + 'static,
{
    warp::path!("v3" / "connections")
        .and(warp::query::query())
        .map(move |filter: ConnectionsFilter| -> WithStatus<Json> {
            match db.fetch_connections(&filter, 100) {
                Ok(connections) => {
                    reply::with_status(reply::json(&connections), StatusCode::OK)
                },
                Err(err) => {
                    let r = &format!("database error: {}", err);
                    reply::with_status(reply::json(&r), StatusCode::INTERNAL_SERVER_ERROR)
                },
            }
        })
}

fn messages<Db>(db: Arc<Db>) -> impl Filter<Extract=(WithStatus<Json>, ), Error=Rejection> + Clone + Sync + Send + 'static
where
    Db: DatabaseFetch + Sync + Send + 'static,
{
    warp::path!("v3" / "messages")
        .and(warp::query::query())
        .map(move |filter: MessagesFilter| -> reply::WithStatus<Json> {
            match db.fetch_messages(&filter, 100) {
                Ok(messages) => {
                    reply::with_status(reply::json(&messages), StatusCode::OK)
                },
                Err(err) => {
                    let r = &format!("database error: {}", err);
                    reply::with_status(reply::json(&r), StatusCode::INTERNAL_SERVER_ERROR)
                },
            }
        })
}

pub fn routes<Db>(
    db: Arc<Db>,
) -> impl Filter<Extract=impl Reply, Error=Rejection> + Clone + Sync + Send + 'static
where
    Db: DatabaseFetch + Sync + Send + 'static,
{
    use warp::reply::with;

    warp::get()
        .and(connections(db.clone()).or(messages(db)))
        .with(with::header("Content-Type", "application/json"))
        .with(with::header("Access-Control-Allow-Origin", "*"))
}
