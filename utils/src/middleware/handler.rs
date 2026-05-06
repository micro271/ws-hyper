use std::marker::PhantomData;

use crate::middleware::{IntoLayer, Layer};
use http::{Request, Response};
use hyper::body::Body;

pub struct HandlerFnMutLayer<F, ReqBody> {
    r#fn: F,
    _ph: PhantomData<ReqBody>,
}

impl<F, ReqBody> HandlerFnMutLayer<F, ReqBody>
where
    F: for<'a> AsyncFnOnce(&'a mut Request<ReqBody>) + Clone,
    ReqBody: Body + Send,
{
    pub fn new(r#fn: F) -> Self {
        Self {
            r#fn,
            _ph: PhantomData,
        }
    }
}

impl<F, ReqBody> From<F> for HandlerFnMutLayer<F, ReqBody>
where
    F: for<'a> AsyncFnOnce(&'a mut Request<ReqBody>) + Clone,
    ReqBody: Body + Send,
{
    fn from(value: F) -> Self {
        Self {
            r#fn: value,
            _ph: PhantomData,
        }
    }
}

#[derive(Debug, Clone)]
pub struct HandlerFn<L, F> {
    pub(super) inner: L,
    pub(super) fn_: F,
}

impl<L, F, ReqBody> Layer<ReqBody> for HandlerFn<L, F>
where
    ReqBody: Body + Send,
    L: Layer<ReqBody> + Clone,
    F: for<'a> AsyncFnOnce(&'a mut Request<ReqBody>) + Clone,
{
    type Error = L::Error;
    type Response = L::Response;
    fn call(
        &self,
        mut req: Request<ReqBody>,
    ) -> impl Future<Output = Result<Response<Self::Response>, Self::Error>> {
        let tmp = self.fn_.clone();
        async move {
            tmp(&mut req).await;
            self.inner.call(req).await
        }
    }
}

impl<L, F, ReqBody> IntoLayer<L, ReqBody> for HandlerFnMutLayer<F, ReqBody>
where
    ReqBody: Body + Send,
    L: Layer<ReqBody> + Clone,
    F: for<'a> AsyncFnOnce(&'a mut Request<ReqBody>) + Clone,
{
    type Output = HandlerFn<L, F>;
    fn into_layer(self, inner: L) -> Self::Output
    where
        Self: Sized,
    {
        HandlerFn {
            inner,
            fn_: self.r#fn,
        }
    }
}
