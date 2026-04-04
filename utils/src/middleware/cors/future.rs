use std::{pin::Pin, task::Poll};

use futures::ready;
use http::{HeaderMap, Response};
use hyper::body::Body;

pub struct CorsFuture<F> {
    pub(super) kind: Kind<F>,
}

pub(super) enum Kind<F> {
    Preflight { headers: HeaderMap },
    Cors { header: HeaderMap, fut: F },
    Pass { fut: F },
}

impl<F, ResBody, E> Future for CorsFuture<F>
where
    F: Future<Output = Result<Response<ResBody>, E>>,
    ResBody: Body + Default + Send,
    E: Send,
{
    type Output = Result<Response<ResBody>, E>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let this = unsafe { self.get_unchecked_mut() };

        match &mut this.kind {
            Kind::Preflight { headers } => {
                let mut resp = Response::builder()
                    .body(<ResBody as Default>::default())
                    .unwrap_or_default();
                resp.headers_mut().extend(headers.drain());
                Poll::Ready(Ok(resp))
            }
            Kind::Cors { header, fut } => {
                let res = ready!(unsafe { Pin::new_unchecked(fut) }.poll(cx));

                Poll::Ready(res.map(|mut x| {
                    x.headers_mut().extend(header.drain());
                    x
                }))
            }
            Kind::Pass { fut } => unsafe { Pin::new_unchecked(fut) }.poll(cx),
        }
    }
}
