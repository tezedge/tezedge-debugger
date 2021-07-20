use warp::reply::Response;
use warp::http::header::{HeaderValue, CONTENT_TYPE};
use warp::Reply;

pub struct RawJson( pub &'static [u8]);

impl Reply for RawJson {
    #[inline]
    fn into_response(self) -> Response {
        let mut res = Response::new(self.0.into());
        res.headers_mut()
            .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        res
    }
}