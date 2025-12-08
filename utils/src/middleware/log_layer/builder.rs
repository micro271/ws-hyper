use crate::middleware::{Next, log_layer::LogLayer};
use http::{Request, Response};
use hyper::body::Body;
use std::{convert::Infallible, marker::PhantomData};

pub struct NoReq;
pub struct Req<R>(R);
pub struct NoRes;
pub struct Res<R>(R);
pub struct NoNext;

pub struct LogLayerBuilder<OnReq, OnRes, N, ReqBody, ResBody> {
    on_req: OnReq,
    on_res: OnRes,
    next: N,
    _ph: PhantomData<(ReqBody, ResBody)>,
}

impl<ReqBody, ResBody> std::default::Default
    for LogLayerBuilder<NoReq, NoRes, NoNext, ReqBody, ResBody>
{
    fn default() -> Self {
        Self {
            on_req: NoReq,
            on_res: NoRes,
            next: NoNext,
            _ph: PhantomData,
        }
    }
}

impl<OnRes, N, ReqBody, ResBody>
    LogLayerBuilder<NoReq, OnRes, N, ReqBody, ResBody>
where
    ReqBody: Body + Send,
    ResBody: Body + Send + Default,
{
    pub fn on_request<R>(
        self,
        on: R,
    ) -> LogLayerBuilder<Req<R>, OnRes, N, ReqBody, ResBody>
    where
        R: for<'a> AsyncFn(&'a Request<ReqBody>) -> Result<Response<ResBody>, Infallible> + Send + Clone,
    {
        LogLayerBuilder {
            on_req: Req(on),
            on_res: self.on_res,
            next: self.next,
            _ph: self._ph,
        }
    }
}

impl<OnReq, N, ReqBody, ResBody>
    LogLayerBuilder<OnReq, NoRes, N, ReqBody, ResBody>
where
    ReqBody: Body + Send + Sync,
    ResBody: Body + Send + Sync + Default,
{
    pub fn on_response<R>(
        self,
        on: R,
    ) -> LogLayerBuilder<OnReq, Res<R>, N, ReqBody, ResBody>
    where
        R:for<'a> AsyncFn(&'a Request<ReqBody>) -> Result<Response<ResBody>, Infallible> + Send + Clone,
    {
        LogLayerBuilder {
            on_req: self.on_req,
            on_res: Res(on),
            next: self.next,
            _ph: self._ph,
        }
    }
}

impl<OnReq, OnRes, ReqBody, ResBody>
    LogLayerBuilder<OnReq, OnRes, NoNext, ReqBody, ResBody>
where
    ReqBody: Body + Send,
    ResBody: Body + Send + Default,
{
    pub fn next<R>(
        self,
        next: R,
    ) -> LogLayerBuilder<OnReq, OnRes, Next<R>, ReqBody, ResBody>
    where
        R: AsyncFn(Request<ReqBody>) -> Result<Response<ResBody>, Infallible> + Send + Clone,
        ReqBody: Body + Send,
        ResBody: Body + Send + Default,
    {
        LogLayerBuilder {
            on_req: self.on_req,
            on_res: self.on_res,
            next: Next(next),
            _ph: self._ph,
        }
    }
}

impl<OnReq, OnRes, N, ReqBody, ResBody>
    LogLayerBuilder<Req<OnReq>, Res<OnRes>, Next<N>, ReqBody, ResBody>
where
    ReqBody: Body + Send,
    ResBody: Body + Send + Default,
    OnReq: for<'a> AsyncFn(&'a Request<ReqBody>) -> Result<Response<ResBody>, Infallible> + Send + Clone,
    OnRes: for<'a> AsyncFn(&'a Request<ReqBody>) -> Result<Response<ResBody>, Infallible> + Send + Clone,
    N: AsyncFn(Request<ReqBody>) -> Result<Response<ResBody>, Infallible> + Send + Clone,
{
    pub fn build(self) -> LogLayer<OnReq, OnRes, N, ReqBody, ResBody> {
        let LogLayerBuilder { on_req: Req(on_req), on_res: Res(on_res), next: Next(next), _ph } = self;
        LogLayer {
            on_req,
            on_res,
            next,
            _ph: self._ph,
        }
    }
}