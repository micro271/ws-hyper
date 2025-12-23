use std::time::Instant;

use crate::middleware::Layer;
use http::{Request, Response};
use hyper::body::Body;

use super::{super::IntoLayer, Log};

#[derive(Debug)]
pub struct LogLayer<OnReq, OnRes> {
    pub(super) on_req: OnReq,
    pub(super) on_res: OnRes,
}

impl<OnReq: Clone, OnRes: Clone> std::clone::Clone for LogLayer<OnReq, OnRes> {
    fn clone(&self) -> Self {
        Self {
            on_req: self.on_req.clone(),
            on_res: self.on_res.clone(),
        }
    }
}

impl<S, A, B, ReqBody, ResBody> IntoLayer<S, ReqBody, ResBody> for LogLayer<B, A>
where
    ResBody: Body + Default + Send,
    ReqBody: Body + Send,
    B: for<'a> AsyncFn(&'a Request<ReqBody>) + Send + Clone + Copy,
    A: for<'a> AsyncFn(&'a Response<ResBody>, Instant) + Send + Clone + Copy,
    S: Layer<ReqBody, ResBody> + Clone,
{
    type Output = Log<S, B, A>;

    fn into_layer(self, inner: S) -> Self::Output
    where
        Self: Sized,
    {
        Log {
            inner,
            before: self.on_req,
            after: self.on_res,
        }
    }
}
