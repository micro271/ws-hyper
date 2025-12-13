use std::time::Instant;
use http::{Request, Response};
use hyper::body::Body;

use super::Layer;

pub mod builder;
pub mod layer;

pub struct Log<L, B, A> {
    inner: L,
    before: B,
    after: A,
}

impl<L: Clone, A: Clone, B: Clone> std::clone::Clone for Log<L, A, B>{
    fn clone(&self) -> Self {
        Self { inner: self.inner.clone(), before: self.before.clone(), after: self.after.clone() }
    }
}

impl<L, ReqBody, ResBody, B, A> Layer<ReqBody, ResBody> for Log<L, B, A> 
where
    L: Layer<ReqBody, ResBody> + Clone,
    B:for<'a> AsyncFn(&'a Request<ReqBody>) + Send + Clone,
    A:for<'a> AsyncFn(&'a Response<ResBody>, Instant) + Send + Clone,
    ReqBody: Body + Send,
    ResBody: Body + Send + Default,

{
    type Error = L::Error;

    fn call(&self, req: http::Request<ReqBody>) -> impl Future<Output = Result<http::Response<ResBody>, Self::Error>> {
        let before = self.before.clone();
        let after = self.after.clone();
        let caller = self.inner.clone();
        async move {
            let ins = Instant::now();
            before(&req).await;
            match caller.call(req).await {
                Ok(e) => {
                    after(&e, ins).await;
                    Ok(e)
                },
                Err(er) => Err(er),
            }
        }
    }
}