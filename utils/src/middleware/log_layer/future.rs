use std::{pin::Pin, time::Instant};
use futures::FutureExt;
use http::{Request, Response};



pub struct LogFuture<'a, L, OnReq, OnRes, ReqBody, ResBody> {
    pub(super) onreq: OnReq,
    pub(super) onres: OnRes,
    pub(super) inner: L,
    pub(super) state: LogFutureState<'a, L, ReqBody, ResBody, OnReq, OnRes>,
}

enum LogFutureState<'a, L, ReqBody, ResBody, OnReq, OnRes> {
    Pending(Request<ReqBody>),
    Before{ req: Request<ReqBody>, fut: OnReq },
    After{ res:&'a Response<ResBody>, fut: OnRes },
    Next(L)
}


impl<'a, L, OnReq, OnRes, ReqBody, ResBody> Future for LogFuture<'a, L, OnReq, OnRes, ReqBody, ResBody> 
where 
    OnReq:for<'b> AsyncFn(&'b Request<ReqBody>) + Send + Clone + Copy,
    OnRes:for<'b> AsyncFn(&'b Response<ResBody>, Instant) + Send + Clone + Copy,
{
    type Output = ();

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        let this = unsafe { self.get_unchecked_mut() };
        loop {
            match this.state {
                LogFutureState::Pending(req) => {
                    this.state = LogFutureState::Before { req: req, fut: self.onreq, }
                }
                LogFutureState::Before { req, fut } => {
                    let fut = unsafe { Pin::new_unchecked(&mut this.onreq.clone()(&req)) };
                },
                LogFutureState::After { res, a } => todo!(),
                LogFutureState::Next(_) => todo!(),
            }
        }
    }
}