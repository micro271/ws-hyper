use std::time::Instant;
use http::{Request, Response};
use hyper::body::Body;
use tracing::{Instrument, Level, span};

use super::Layer;

pub mod builder;
pub mod layer;

pub struct Log<L, B, A> {
    inner: L,
    before: B,
    after: A
}

impl<L: Clone, A: Clone, B: Clone> std::clone::Clone for Log<L, A, B>{
    fn clone(&self) -> Self {
        Self { inner: self.inner.clone(), before: self.before.clone(), after: self.after.clone() }
    }
}

impl<L, ReqBody, ResBody, B, A> Layer<ReqBody, ResBody> for Log<L, B, A> 
where
    L: Layer<ReqBody, ResBody> + Clone,
    B:for<'a> AsyncFn(&'a Request<ReqBody>) + Send + Clone + Copy,
    A:for<'a> AsyncFn(&'a Response<ResBody>, Instant) + Send + Clone + Copy,
    ReqBody: Body + Send,
    ResBody: Body + Send + Default,

{
    type Error = L::Error;

    async fn call(&self, req: http::Request<ReqBody>) -> Result<http::Response<ResBody>, Self::Error> {
        
        let span = span!(Level::INFO, "HTTP", path = %req.uri().path(), rid = nanoid::nanoid!());
        let instant = Instant::now();
        (self.before.clone())(&req).instrument(span.clone()).await;
        let resp = self.inner.call(req).instrument(span.clone()).await?;
        (self.after)(&resp, instant).instrument(span).await;
        Ok(resp)
    }
}