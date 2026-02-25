use http::Response;
use hyper::body::Body;

use crate::middleware::Layer;

pub struct State<K, L> {
    state: K,
    inner: L,
}

impl<K: Clone, L: Clone> std::clone::Clone for State<K, L> {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            inner: self.inner.clone(),
        }
    }
}

impl<K, L> State<K, L> {
    pub(super) fn new<ReqBody>(state: K, layer: L) -> State<K, L>
    where
        K: Clone + Send + Sync + 'static,
        L: Layer<ReqBody>,
        ReqBody: Body + Send,
    {
        State {
            state,
            inner: layer,
        }
    }
}

impl<L, K, ReqBody> Layer<ReqBody> for State<K, L>
where
    ReqBody: Body + Send,
    L: Layer<ReqBody>,
    K: Clone + Send + Sync + 'static,
{
    type Error = L::Error;
    type Response = L::Response;
    fn call(
        &self,
        mut req: http::Request<ReqBody>,
    ) -> impl Future<Output = Result<Response<Self::Response>, Self::Error>> {
        req.extensions_mut().insert(self.state.clone());

        ResponseFutureState {
            f: self.inner.call(req),
        }
    }
}

pub struct ResponseFutureState<K> {
    f: K,
}

impl<K, ResBody, E> Future for ResponseFutureState<K>
where
    K: Future<Output = Result<Response<ResBody>, E>>,
    ResBody: Body + Send + Default,
{
    type Output = Result<Response<ResBody>, E>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        unsafe { self.map_unchecked_mut(|x| &mut x.f) }.poll(cx)
    }
}
