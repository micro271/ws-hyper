use super::layer::LogLayer;
use http::{Request, Response};
use hyper::body::Body;
use std::{marker::PhantomData, time::Instant};

pub struct NoReq;
pub struct Req<R>(R);
pub struct NoRes;
pub struct Res<R>(R);

pub struct LogLayerBuilder<OnReq, OnRes, ReqBody, ResBody> {
    on_req: OnReq,
    on_res: OnRes,
    _ph: PhantomData<(ReqBody, ResBody)>,
}

impl<ReqBody, ResBody> std::default::Default
    for LogLayerBuilder<NoReq, NoRes, ReqBody, ResBody>
{
    fn default() -> Self {
        Self {
            on_req: NoReq,
            on_res: NoRes,
            _ph: PhantomData,
        }
    }
}

impl<OnRes, ReqBody, ResBody>
    LogLayerBuilder<NoReq, OnRes, ReqBody, ResBody>
where
    ReqBody: Body + Send,
    ResBody: Body + Send + Default,
{
    pub fn on_request<R>(
        self,
        on: R,
    ) -> LogLayerBuilder<Req<R>, OnRes, ReqBody, ResBody>
    where
        R: for<'a> AsyncFn(&'a Request<ReqBody>) + Send,
    {
        LogLayerBuilder {
            on_req: Req(on),
            on_res: self.on_res,
            _ph: self._ph,
        }
    }
}

impl<OnReq, ReqBody, ResBody>
    LogLayerBuilder<OnReq, NoRes, ReqBody, ResBody>
where
    ReqBody: Body + Send + Sync,
    ResBody: Body + Send + Sync + Default,
{
    pub fn on_response<R>(
        self,
        on: R,
    ) -> LogLayerBuilder<OnReq, Res<R>, ReqBody, ResBody>
    where
        R:for<'a> AsyncFn(&'a Response<ResBody>, Instant) + Send,
    {
        LogLayerBuilder {
            on_req: self.on_req,
            on_res: Res(on),
            _ph: self._ph,
        }
    }
}

impl<OnReq, OnRes, ReqBody, ResBody>
    LogLayerBuilder<Req<OnReq>, Res<OnRes>, ReqBody, ResBody>
where
    ReqBody: Body + Send,
    ResBody: Body + Send + Default,
    OnReq: for<'a> AsyncFn(&'a Request<ReqBody>) + Send + Sync,
    OnRes: for<'a> AsyncFn(&'a Response<ResBody>, Instant) + Send + Sync,
{
    pub fn build(self) -> LogLayer<OnReq, OnRes> {
        let LogLayerBuilder { on_req: Req(on_req), on_res: Res(on_res), _ph } = self;
        LogLayer {
            on_req,
            on_res,
        }
    }
}