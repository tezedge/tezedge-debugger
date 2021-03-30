use std::sync::Arc;
use anyhow::Result;
use warp::{
    Filter, Rejection, Reply,
    reply::{WithStatus, Json, self},
    http::StatusCode,
};
use super::{
    database::{DatabaseFetch, ConnectionsFilter, ChunksFilter, MessagesFilter},
    tables::chunk,
};

fn connections<Db>(
    db: Arc<Db>,
) -> impl Filter<Extract = (WithStatus<Json>,), Error = Rejection> + Clone + Sync + Send + 'static
where
    Db: DatabaseFetch + Sync + Send + 'static,
{
    warp::path!("v3" / "connections")
        .and(warp::query::query())
        .map(move |filter: ConnectionsFilter| -> WithStatus<Json> {
            match db.fetch_connections(&filter) {
                Ok(connections) => reply::with_status(reply::json(&connections), StatusCode::OK),
                Err(err) => {
                    let r = &format!("database error: {}", err);
                    reply::with_status(reply::json(&r), StatusCode::INTERNAL_SERVER_ERROR)
                },
            }
        })
}

fn chunks<Db>(
    db: Arc<Db>,
) -> impl Filter<Extract = (WithStatus<Json>,), Error = Rejection> + Clone + Sync + Send + 'static
where
    Db: DatabaseFetch + Sync + Send + 'static,
{
    warp::path!("v3" / "chunks")
        .and(warp::query::query())
        .map(move |filter: ChunksFilter| -> WithStatus<Json> {
            match db.fetch_chunks_truncated(&filter) {
                Ok(chunks) => reply::with_status(reply::json(&chunks), StatusCode::OK),
                Err(err) => {
                    let r = &format!("database error: {}", err);
                    reply::with_status(reply::json(&r), StatusCode::INTERNAL_SERVER_ERROR)
                },
            }
        })
}

fn chunk<Db>(db: Arc<Db>) -> impl Filter<Extract = (WithStatus<Json>,), Error = Rejection> + Clone + Sync + Send + 'static
where
    Db: DatabaseFetch + Sync + Send + 'static,
{
    fn inner<Db>(db: &Arc<Db>, chunk_id: String) -> Result<Option<chunk::Value>>
    where
        Db: DatabaseFetch + Sync + Send + 'static,
    {
        let key = chunk_id.parse()?;
        db.fetch_chunk(&key).map_err(Into::into)
    }

    warp::path!("v3" / "chunk" / String)
        .map(move |chunk_id: String| -> WithStatus<Json> {
            
            match inner(&db, chunk_id) {
                Ok(v) => reply::with_status(reply::json(&v), StatusCode::OK),
                Err(err) => {
                    let r = format!("database error: {}", err);
                    reply::with_status(reply::json(&r), StatusCode::INTERNAL_SERVER_ERROR)
                },
            }
        })
}

fn messages<Db>(
    db: Arc<Db>,
) -> impl Filter<Extract = (WithStatus<Json>,), Error = Rejection> + Clone + Sync + Send + 'static
where
    Db: DatabaseFetch + Sync + Send + 'static,
{
    warp::path!("v3" / "messages")
        .and(warp::query::query())
        .map(move |filter: MessagesFilter| -> reply::WithStatus<Json> {
            match db.fetch_messages(&filter) {
                Ok(messages) => reply::with_status(reply::json(&messages), StatusCode::OK),
                Err(err) => {
                    let r = &format!("database error: {}", err);
                    reply::with_status(reply::json(&r), StatusCode::INTERNAL_SERVER_ERROR)
                },
            }
        })
}

pub fn routes<Db>(
    db: Arc<Db>,
) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone + Sync + Send + 'static
where
    Db: DatabaseFetch + Sync + Send + 'static,
{
    use warp::reply::with;

    warp::get()
        .and(connections(db.clone()).or(chunks(db.clone())).or(chunk(db.clone())).or(messages(db)))
        .with(with::header("Content-Type", "application/json"))
        .with(with::header("Access-Control-Allow-Origin", "*"))
}
