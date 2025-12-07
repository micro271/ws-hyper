pub mod cors;
use http::{Request, Response};
use hyper::body::Body;

pub trait Middleware: Sync {
    type ReqBody: Body + Send;
    type ResBody: Body + Default + Send;
    
    fn middleware(&self, req: Request<Self::ReqBody>) -> impl Future<Output = Response<Self::ResBody>> + Send;
}