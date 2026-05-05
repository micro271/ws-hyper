pub mod cors;
pub mod entry;
pub mod handler;
pub mod log_layer;
pub mod proxy_info;
pub mod state;

use http::{Request, Response};
use hyper::body::Body;

use crate::middleware::{entry::EntryFn, handler::HandlerFnMutLayer, state::State};

#[derive(Debug, Clone)]
pub struct MiddlewareStack<S> {
    inner: S,
}

#[derive(Debug, Clone)]
pub struct MiddlewareBuilder;

impl MiddlewareBuilder {
    pub fn entry_fn<E, Err, ReqBody, ResBody>(entry: E) -> MiddlewareStack<EntryFn<E>>
    where
        E: AsyncFnOnce(Request<ReqBody>) -> Result<Response<ResBody>, Err> + Clone,
        Err: std::error::Error + Send + Sync + 'static,
        ResBody: Body + Send + Default,
        ReqBody: Body + Send,
    {
        MiddlewareStack {
            inner: EntryFn::new(entry),
        }
    }

    pub fn entry<L, ReqBody>(inner: L) -> MiddlewareStack<L>
    where
        L: Layer<ReqBody>,
        ReqBody: Body + Send,
    {
        MiddlewareStack { inner }
    }
}

impl<L> MiddlewareStack<L> {
    pub fn layer<I, ReqBody>(self, layer: I) -> MiddlewareStack<I::Output>
    where
        L: Layer<ReqBody> + Clone,
        I: IntoLayer<L, ReqBody>,
        I::Output: Layer<ReqBody, Response = L::Response>,
        ReqBody: Body + Send,
    {
        let inner = layer.into_layer(self.inner);
        MiddlewareStack { inner }
    }

    pub fn layer_mut_fn<H, ReqBody, ResBody>(
        self,
        layer: H,
    ) -> MiddlewareStack<<HandlerFnMutLayer<H, ReqBody> as IntoLayer<L, ReqBody>>::Output>
    where
        L: Layer<ReqBody> + Clone,
        H: for<'a> AsyncFnOnce(&'a mut Request<ReqBody>)
            + Clone
            + Into<HandlerFnMutLayer<H, ReqBody>>,
        ReqBody: Body + Send,
    {
        MiddlewareStack {
            inner: layer.into().into_layer(self.inner),
        }
    }

    pub fn state<K, ReqBody>(self, state: K) -> MiddlewareStack<State<K, L>>
    where
        K: Send + Sync + Clone + 'static,
        ReqBody: Body + Send,
        L: Layer<ReqBody>,
    {
        MiddlewareStack {
            inner: State::new(state, self.inner),
        }
    }
}

pub trait Layer<ReqBody>
where
    ReqBody: Body + Send,
{
    type Error: std::error::Error + Send + Sync + 'static;
    type Response: Body + Default + Send;
    fn call(
        &self,
        req: Request<ReqBody>,
    ) -> impl Future<Output = Result<Response<Self::Response>, Self::Error>>;
}

pub trait IntoLayer<S, ReqBody>
where
    ReqBody: Body + Send,
    S: Layer<ReqBody> + Clone,
{
    type Output: Layer<ReqBody, Response = S::Response> + Clone;
    fn into_layer(self, inner: S) -> Self::Output
    where
        Self: Sized;
}

impl<E, ReqBody> Layer<ReqBody> for MiddlewareStack<E>
where
    E: Layer<ReqBody>,
    ReqBody: Body + Send,
{
    type Error = E::Error;
    type Response = E::Response;

    fn call(
        &self,
        req: Request<ReqBody>,
    ) -> impl Future<Output = Result<Response<Self::Response>, Self::Error>> {
        SimpleFuture {
            f: self.inner.call(req),
        }
    }
}

pub(crate) struct SimpleFuture<F> {
    pub(crate) f: F,
}

impl<F, R, E> Future for SimpleFuture<F>
where
    F: Future<Output = Result<R, E>>,
{
    type Output = Result<R, E>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        unsafe { self.map_unchecked_mut(|x| &mut x.f) }.poll(cx)
    }
}
