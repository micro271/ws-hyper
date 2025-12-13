pub mod cors;
pub mod log_layer;
pub mod entry;

use http::{Request, Response};
use hyper::body::Body;
use entry::Entry;

pub struct Empty;

#[derive(Debug, Clone)]
pub struct MiddlwareStack<S> {
    stack: S,
}

impl std::default::Default for MiddlwareStack<Empty> {
    fn default() -> Self {
        Self {
            stack: Empty,
        }
    }
}

impl MiddlwareStack<Empty> {
    pub fn entry<E, Err, ReqBody, ResBody>(&self, entry: E) -> MiddlwareStack<Entry<E>> 
    where
        E: AsyncFnOnce(Request<ReqBody>) -> Result<Response<ResBody>, Err> + Clone + Copy,
        Err: std::error::Error + Send + 'static,
        ResBody: Body + Send + Default,
        ReqBody: Body + Send,
    {
        MiddlwareStack { stack: Entry::new(entry) }
    }
}

impl<L> MiddlwareStack<L> {
    pub fn layer<I, ReqBody, ResBody>(self, layer: I) -> MiddlwareStack<I::Output> 
    where 
        L: Layer<ReqBody, ResBody> + Clone,
        I: IntoLayer<L, ReqBody, ResBody>,
        ResBody: Body + Send + Default,
        ReqBody: Body + Send,
    {
        let stack = layer.into_layer(self.stack);
        MiddlwareStack { stack }
    }
}

impl<S, ReqBody, ResBody> Layer<ReqBody, ResBody> for  MiddlwareStack<S> 
where 
    S: Layer<ReqBody, ResBody> + Clone,
    S::Error: Send,
    ReqBody: Body + Send,
    ResBody: Body + Send + Default,
{
    type Error = S::Error;

    fn call(&self, req: Request<ReqBody>) -> impl Future<Output = Result<Response<ResBody>, Self::Error> > {
        let tmp = self.stack.clone();
        async move {
            tmp.call(req).await
        }
    }
}

pub trait Middleware<ReqBody, ResBody> 
where 
    ReqBody: Send,
    ResBody: Send + Default,
{
    type Error: std::error::Error;

    fn call(
        &self,
        req: Request<ReqBody>,
    ) -> impl Future<Output = Result<Response<ResBody>, Self::Error>>;
}

pub trait Layer<ReqBody, ResBody>
where 
    ResBody: Body + Send + Default,
    ReqBody: Body + Send,
{
    type Error: std::error::Error + Send + Sync + 'static;
    fn call(&self, req: Request<ReqBody>) -> impl Future<Output = Result<Response<ResBody>, Self::Error> >;
}

pub trait IntoLayer<S, ReqBody, ResBody> 
where 
    ReqBody: Body + Send,
    ResBody: Body + Send + Default,
    S: Layer<ReqBody, ResBody> + Clone
{
    type Output: Layer<ReqBody, ResBody> + Clone;
    fn into_layer(self, inner: S) -> Self::Output where Self: Sized;
}