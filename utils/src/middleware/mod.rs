pub mod cors;
pub mod log_layer;

use std::convert::Infallible;

use http::{Request, Response};
use hyper::body::Body;

pub trait Middleware: Send {
    type ReqBody: Body + Send;
    type ResBody: Body + Default + Send;
    type Error: std::error::Error;

    fn middleware(
        &self,
        req: Request<Self::ReqBody>,
    ) -> impl Future<Output = Result<Response<Self::ResBody>, Self::Error>> + Send;
}

pub struct Next<R>(R);

impl<R> Next<R> {
    pub fn new<Fut, ReqBody, ResBody>(next: R) -> Self
    where
        R: Fn(Request<ReqBody>) -> Fut,
        Fut: Future<Output = Result<Response<ResBody>, Infallible>> + Send,
        ReqBody: Body + Send,
        ResBody: Body + Send,
    {
        Self(next)
    }
}
