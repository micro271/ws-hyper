pub mod cors;
pub mod log_layer;
pub mod entry;
pub mod handler;

use http::{Request, Response};
use hyper::body::Body;

use crate::middleware::entry::EntryFn;

#[derive(Debug, Clone)]
pub struct MiddlwareStack<S> {
    inner: S,
}

impl std::default::Default for MiddlwareStack<Empty> {
    fn default() -> Self {
        Self {
            inner: Empty::new(),
        }
    }
}

impl<S> std::ops::Deref for MiddlwareStack<S> {

    type Target = S;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl MiddlwareStack<Empty> {
    pub fn entry_fn<E, Err, ReqBody, ResBody>(&self, entry: E) -> MiddlwareStack<EntryFn<E>> 
    where
        E: AsyncFnOnce(Request<ReqBody>) -> Result<Response<ResBody>, Err> + Clone,
        Err: std::error::Error + Send + Sync + 'static,
        ResBody: Body + Send + Default,
        ReqBody: Body + Send,
    {
        MiddlwareStack { inner: EntryFn::new(entry) }
    }

    pub fn entry<L, ReqBody, ResBody>(self, inner: L) -> MiddlwareStack<L> 
    where
        L: Layer<ReqBody, ResBody>,
        ReqBody: Body + Send,
        ResBody: Body + Send + Default,
    {
        MiddlwareStack { inner }
    }
}

impl<L> MiddlwareStack<L> {
    pub fn layer<I, ReqBody, ResBody>(self, layer: I) -> MiddlwareStack<I::Output> 
    where
        L: Layer<ReqBody, ResBody> + Clone,
        I: IntoLayer<L, ReqBody, ResBody>,
        I::Output: Clone,
        ResBody: Body + Send + Default,
        ReqBody: Body + Send,
    {
        let inner = layer.into_layer(self.inner);
        MiddlwareStack { inner }
    }

    
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

#[derive(Debug, Clone, Copy)]
pub struct Empty {
    _p: (),
}

impl Empty {
    pub(self) fn new() -> Self {
        Self { _p: () }
    }
}