use http::{Request, Response};
use hyper::body::Body;

use crate::middleware::Layer;

pub struct Entry<E>(E);

impl<E> Entry<E> {
    pub fn new<Err, ResBody, ReqBody>(entry: E) -> Self 
    where 
        E: AsyncFnOnce(Request<ReqBody>) -> Result<Response<ResBody>, Err> + Clone + Copy,
        Err: std::error::Error + Send,
        ResBody: Body + Send + Default,
        ReqBody: Body + Send,
    {
        Self(entry)
    }
}

impl<E: Clone> std::clone::Clone for Entry<E> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<E, Err, ReqBody, ResBody> Layer<ReqBody, ResBody> for Entry<E> 
where
    E: AsyncFnOnce(Request<ReqBody>) -> Result<Response<ResBody>, Err> + Clone + Copy,
    Err: std::error::Error + Send + Sync + 'static,
    ReqBody: Body + Send,
    ResBody: Body + Send + Default,
{
    type Error = Err;

    fn call(&self, req: http::Request<ReqBody>) -> impl Future<Output = Result<http::Response<ResBody>, Self::Error> > {
        (self.0)(req)
    }
}