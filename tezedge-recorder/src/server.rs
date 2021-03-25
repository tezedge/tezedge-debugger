
use std::sync::Arc;
use warp::{Filter, Rejection, Reply};
use super::database::DatabaseFetch;

pub fn routes<Db>(
    db: Arc<Db>,
) -> impl Filter<Extract=impl Reply, Error=Rejection> + Clone + Sync + Send + 'static
where
    Db: DatabaseFetch + Sync + Send + 'static,
{
    use warp::{reply::with, reply::{self, Json}, http::StatusCode};

    warp::get()
        .and({
            warp::path!("v3" / "connections")
                .and(warp::query::query())
                .map(move |()| -> reply::WithStatus<Json> {
                    match db.fetch_connections(None, u64::MAX) {
                        Ok(connections) => {
                            reply::with_status(reply::json(&connections), StatusCode::OK)
                        },
                        Err(err) => {
                            let r = &format!("database error: {}", err);
                            reply::with_status(reply::json(&r), StatusCode::INTERNAL_SERVER_ERROR)
                        },
                    }
                })
        })
        .with(with::header("Content-Type", "application/json"))
        .with(with::header("Access-Control-Allow-Origin", "*"))
}
