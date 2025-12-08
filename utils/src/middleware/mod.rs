pub mod cors;
pub mod log_layer;

use http::{Request, Response};
use hyper::body::Body;

pub trait Middleware: Sync {
    type ReqBody: Body + Send;
    type ResBody: Body + Default + Send;
    
    fn middleware(&self, req: Request<Self::ReqBody>) -> impl Future<Output = Response<Self::ResBody>> + Send;
}

pub struct Next<R>(R);

impl<R> Next<R> {
    pub fn new<Fut, ReqBody, ResBody>(next: R) -> Self 
    where 
        R: Fn(Request<ReqBody>) -> Fut,
        Fut: Future<Output = Response<ResBody>> + Send,
        ReqBody: Body + Send,
        ResBody: Body + Send,
    {
        Self(next)
    }
}
