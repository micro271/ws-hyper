use std::convert::Infallible;

use http_body_util::Full;
use hyper::{
    Request, Response,
    body::{Bytes, Incoming},
};

pub async fn upload(req: Request<Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
    todo!()
}
