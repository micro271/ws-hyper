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
pub struct Stack<S> {
    inner: S,
}

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

impl MiddlwareStack<Empty> {
    pub fn entry_fn<E, Err, ReqBody, ResBody>(&self, entry: E) -> Stack<EntryFn<E>>
    where
        E: AsyncFnOnce(Request<ReqBody>) -> Result<Response<ResBody>, Err> + Clone,
        Err: std::error::Error + Send + Sync + 'static,
        ResBody: Body + Send + Default,
        ReqBody: Body + Send,
    {
        Stack {
            inner: EntryFn::new(entry),
        }
    }

    pub fn entry<L, ReqBody>(self, inner: L) -> Stack<L>
    where
        L: Layer<ReqBody>,
        ReqBody: Body + Send,
    {
        Stack { inner }
    }
}

impl<L> Stack<L> {
    pub fn layer<I, ReqBody>(self, layer: I) -> Stack<I::Output>
    where
        L: Layer<ReqBody> + Clone,
        I: IntoLayer<L, ReqBody>,
        I::Output: Layer<ReqBody, Response = L::Response>,
        ReqBody: Body + Send,
    {
        let inner = layer.into_layer(self.inner);
        Stack { inner }
    }

    pub fn layer_mut_fn<H, ReqBody, ResBody>(
        self,
        layer: H,
    ) -> Stack<<HandlerFnMutLayer<H, ReqBody> as IntoLayer<L, ReqBody>>::Output>
    where
        L: Layer<ReqBody> + Clone,
        H: for<'a> AsyncFnOnce(&'a mut Request<ReqBody>)
            + Clone
            + Into<HandlerFnMutLayer<H, ReqBody>>,
        ReqBody: Body + Send,
    {
        Stack {
            inner: layer.into().into_layer(self.inner),
        }
    }

    pub fn state<K, ReqBody>(self, state: K) -> Stack<State<K, L>>
    where
        K: Send + Sync + Clone + 'static,
        ReqBody: Body + Send,
        L: Layer<ReqBody>,
    {
        Stack {
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

#[derive(Debug, Clone, Copy)]
pub struct Empty {
    _p: (),
}

impl Empty {
    pub(self) fn new() -> Self {
        Self { _p: () }
    }
}

impl<E, ReqBody> Layer<ReqBody> for Stack<E>
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
        self.inner.call(req)
    }
}
