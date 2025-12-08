use std::marker::PhantomData;

use http::{Request, Response};
use hyper::body::Body;

use crate::middleware::Next;



pub struct NoReq;
pub struct Req<R>(R);
pub struct NoRes;
pub struct Res<R>(R);
pub struct NoNext;

pub struct LogLayerBuilder<OnReq, OnRes, N, ReqBody, ResBody, OnReqFut, OnResFut, NextFut> {
    on_req: OnReq,
    on_res: OnRes,
    next: N,
    _ph: PhantomData<(ReqBody, ResBody, OnReqFut, OnResFut, NextFut)>
}

impl<ReqBody, ResBody, OnReqFut, OnResFut, NextFut> std::default::Default for LogLayerBuilder<NoReq, NoRes, NoNext, ReqBody, ResBody, OnReqFut, OnResFut, NextFut> {
    fn default() -> Self {
        Self {
            on_req: NoReq,
            on_res: NoRes,
            next: NoNext,
            _ph: PhantomData,
        }
    }
}

impl<OnRes, N, ReqBody, ResBody, OnReqFut, OnResFut, NextFut> LogLayerBuilder<NoReq, OnRes, N, ReqBody, ResBody, OnReqFut, OnResFut, NextFut>  
where
    ReqBody: Body + Send,
    ResBody: Body + Send,
    OnReqFut: Future<Output = Response<ResBody>> + Send,
{
    pub fn on_request<R>(self, on: R) -> LogLayerBuilder<Req<R>, OnRes, N, ReqBody, ResBody, OnReqFut, OnResFut, NextFut> 
    where 
        R: Fn(&Request<ReqBody>) -> OnReqFut,
    {
        LogLayerBuilder { on_req: Req(on), on_res: self.on_res, next: self.next, _ph: self._ph }
    }
}

impl<OnReq, N, ReqBody, ResBody, OnReqFut, OnResFut, NextFut> LogLayerBuilder<OnReq, NoRes, N, ReqBody, ResBody, OnReqFut, OnResFut, NextFut>  
where
    ReqBody: Body + Send,
    ResBody: Body + Send,
    OnResFut: Future<Output = Response<ResBody>> + Send,
{
    pub fn on_response<R>(self, on: R) -> LogLayerBuilder<OnReq, Res<R>, N, ReqBody, ResBody, OnReqFut, OnResFut, NextFut> 
    where 
        R: Fn(&Request<ReqBody>) -> OnReqFut,
    {
        LogLayerBuilder { on_req: self.on_req, on_res: Res(on), next: self.next, _ph: self._ph }
    }
}

impl<OnReq, OnRes, ReqBody, ResBody, OnReqFut, OnResFut, NextFut> LogLayerBuilder<OnReq, OnRes, NoNext, ReqBody, ResBody, OnReqFut, OnResFut, NextFut>  
where
    ReqBody: Body + Send,
    ResBody: Body + Send,
    OnResFut: Future<Output = Response<ResBody>> + Send,
{
    pub fn next<R>(self, next: R) -> LogLayerBuilder<OnReq, OnRes, Next<R>, ReqBody, ResBody, OnReqFut, OnResFut, NextFut> 
    where 
        R: Fn(Request<ReqBody>) -> NextFut,
        ReqBody: Body + Send,
        ResBody: Body + Send,
        NextFut: Future<Output = Response<ResBody>> + Send,
    {
        LogLayerBuilder { on_req: self.on_req, on_res: self.on_res, next: Next::new(next), _ph: self._ph }
    }
}