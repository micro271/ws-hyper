use std::{convert::Infallible, marker::PhantomData, time::Instant};

use http::{Request, Response};
use hyper::body::Body;
use tracing::{Instrument, Level, span};

pub mod builder;

#[derive(Debug)]
pub struct LogLayer<OnReq, OnRes, N, ReqBody, ResBody> {
    pub on_req: OnReq,
    pub on_res: OnRes,
    pub next: N,
    pub _ph: PhantomData<(ReqBody, ResBody)>
}

impl<OnReq: Clone, OnRes: Clone, N: Clone, ReqBody, ResBody> std::clone::Clone for LogLayer<OnReq, OnRes, N, ReqBody, ResBody> {
    fn clone(&self) -> Self {
        Self {
            on_req: self.on_req.clone(),
            on_res: self.on_res.clone(),
            next: self.next.clone(),
            _ph: self._ph.clone()
        }
    }
}

impl<OnReq, OnRes, N, ReqBody, ResBody> 
    LogLayer<OnReq, OnRes, N, ReqBody, ResBody>
where
    OnReq:for<'a> AsyncFn(&'a Request<ReqBody>) + Send + Clone + Sync,
    OnRes:for<'a> AsyncFn(&'a Response<ResBody>, Instant) + Send + Clone + Sync,
    N: AsyncFn(Request<ReqBody>) -> Result<Response<ResBody>, Infallible> + Send + Clone + Sync,
    ReqBody: Body + Send,
    ResBody: Body + Send + Default,
{
    pub async fn middleware(
        &self,
        req: http::Request<ReqBody>,
    ) -> Result<Response<ResBody>, Infallible> {
        let elapsed = Instant::now();
        let span = span!(Level::INFO, "HTTP");
        
        (self.on_req)(&req).instrument(span.clone()).await;
        let resp = (self.next)(req).instrument(span.clone()).await?;        
        (self.on_res)(&resp, elapsed).instrument(span.clone()).await;
        
        Ok(resp)
    }
}