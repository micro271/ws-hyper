use http::{Request, Response};
use hyper::body::Body;
use crate::middleware::{IntoLayer, Layer};

pub struct HandlerFnLayer<F>(F);

impl<F> HandlerFnLayer<F> {
    pub fn new<ReqBody>(r#fn: F) -> Self 
    where 
        F: for<'a> AsyncFn(&'a Request<ReqBody>) + Clone + Copy,
        ReqBody: Body + Send,
    {
        Self(r#fn)
    }
}

#[derive(Debug, Clone)]
pub struct HandlerFn<L, F> {
    inner: L,
    fn_: F,
}

impl<L, F, ReqBody, ResBody> Layer<ReqBody, ResBody> for HandlerFn<L, F> 
where 
    ResBody: Body + Send + Default,
    ReqBody: Body + Send,
    L: Layer<ReqBody, ResBody> + Clone,
    F: for<'a> AsyncFn(&'a Request<ReqBody>) + Clone + Copy,
{
    type Error = L::Error;
    fn call(&self, req: Request<ReqBody>) -> impl Future<Output = Result<Response<ResBody>, Self::Error>> {
        let tmp = self.fn_.clone();
        let inner = self.inner.clone();
        async move {
            tmp(&req).await;
            inner.call(req).await
        }
    }
}

impl<L, F, ReqBody, ResBody> IntoLayer<L, ReqBody, ResBody> for HandlerFnLayer<F> 
where 
    ResBody: Body + Send + Default,
    ReqBody: Body + Send,
    L: Layer<ReqBody, ResBody> + Clone,
    F: for<'a> AsyncFn(&'a Request<ReqBody>) + Clone + Copy,
{
    type Output = HandlerFn<L, F>;
    fn into_layer(self, inner: L) -> Self::Output where Self: Sized {
        HandlerFn {
            inner,
            fn_: self.0,
        }
    }
}