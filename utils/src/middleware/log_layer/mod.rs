use http::{Request, Response};
use hyper::body::Body;
use std::time::Instant;
use tracing::{Instrument, Level, span};

use super::Layer;

pub mod builder;
pub mod layer;

pub struct Log<L, B, A> {
    inner: L,
    before: B,
    after: A,
}

impl<L: Clone, A: Clone, B: Clone> std::clone::Clone for Log<L, A, B> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            before: self.before.clone(),
            after: self.after.clone(),
        }
    }
}

impl<L, ReqBody, B, A> Layer<ReqBody> for Log<L, B, A>
where
    L: Layer<ReqBody> + Clone,
    B: for<'a> AsyncFn(&'a Request<ReqBody>) + Send + Clone + Copy,
    A: for<'a> AsyncFn(&'a Response<L::Response>, Instant) + Send + Clone + Copy,
    ReqBody: Body + Send,
{
    type Error = L::Error;
    type Response = L::Response;
    async fn call(
        &self,
        req: http::Request<ReqBody>,
    ) -> Result<http::Response<Self::Response>, Self::Error> {
        let span = span!(Level::INFO, "HTTP", path = %req.uri().path(), rid = nanoid::nanoid!());
        let instant = Instant::now();
        (self.before.clone())(&req).instrument(span.clone()).await;
        let resp = self.inner.call(req).instrument(span.clone()).await?;
        (self.after)(&resp, instant).instrument(span).await;
        Ok(resp)
    }
}
