use http::{Request, Response};
use hyper::body::Body;

use crate::middleware::Layer;

#[derive(Clone)]
pub struct EntryFn<E>(E);

impl<E> EntryFn<E> {
    pub fn new<Err, ResBody, ReqBody>(entry: E) -> Self
    where
        E: AsyncFnOnce(Request<ReqBody>) -> Result<Response<ResBody>, Err> + Clone,
        Err: std::error::Error + Send,
        ResBody: Body + Send + Default,
        ReqBody: Body + Send,
    {
        Self(entry)
    }
}

impl<E, Err, ReqBody, ResBody> Layer<ReqBody> for EntryFn<E>
where
    E: AsyncFnOnce(Request<ReqBody>) -> Result<Response<ResBody>, Err> + Clone,
    Err: std::error::Error + Send + Sync + 'static,
    ReqBody: Body + Send,
    ResBody: Body + Send + Default,
{
    type Error = Err;
    type Response = ResBody;
    fn call(
        &self,
        req: http::Request<ReqBody>,
    ) -> impl Future<Output = Result<http::Response<ResBody>, Self::Error>> {
        let tmp = self.0.clone();
        async move { tmp(req).await }
    }
}
