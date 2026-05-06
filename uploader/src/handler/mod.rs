pub mod uploader;

use http_body_util::Full;
use hyper::{
    Method, Response, StatusCode,
    body::{Bytes, Incoming},
};
use std::{convert::Infallible, pin::Pin, task::Poll};
use utils::middleware::Layer;

use crate::handler::uploader::upload;

#[derive(Debug, Clone)]
pub struct Entry;

impl Layer<Incoming> for Entry {
    type Error = Infallible;

    type Response = Full<Bytes>;

    fn call(
        &self,
        req: hyper::Request<Incoming>,
    ) -> impl Future<Output = Result<hyper::Response<Self::Response>, Self::Error>> {
        let path = req.uri().path();

        if path.starts_with("/upload") {
            if req.method() == Method::POST {
                FutureEntry::Next { f: upload(req) }
            } else {
                FutureEntry::Inmediate(Some(Ok(Response::builder()
                    .status(StatusCode::METHOD_NOT_ALLOWED)
                    .body(Full::default())
                    .unwrap_or_default())))
            }
        } else {
            FutureEntry::Inmediate(Some(Ok(Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Full::default())
                .unwrap_or_default())))
        }
    }
}

pub enum FutureEntry<F> {
    Next { f: F },
    Inmediate(Option<Result<Response<Full<Bytes>>, Infallible>>),
}

impl<F> Future for FutureEntry<F>
where
    F: Future<Output = Result<Response<Full<Bytes>>, Infallible>>,
{
    type Output = F::Output;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let mut this = unsafe { self.get_unchecked_mut() };

        match &mut this {
            FutureEntry::Next { f } => unsafe { Pin::new_unchecked(f) }.poll(cx),
            FutureEntry::Inmediate(response) => Poll::Ready(response.take().unwrap()),
        }
    }
}
