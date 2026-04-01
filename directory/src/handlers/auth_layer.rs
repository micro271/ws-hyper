use std::{convert::Infallible, pin::Pin, task::Poll};

use http::{Response, StatusCode};
use http_body_util::Full;
use hyper::body::{Body, Bytes};
use utils::{
    JwtBoth, JwtHandle, Token, VerifyTokenEcdsa,
    claim::Claim,
    middleware::{IntoLayer, Layer},
};

pub struct Auth;

impl<L, ReqBody> IntoLayer<L, ReqBody> for Auth
where
    L: Layer<ReqBody, Error = Infallible, Response = Full<Bytes>> + Clone,
    ReqBody: Body + Send,
{
    type Output = AuthLayer<L>;

    fn into_layer(self, inner: L) -> Self::Output
    where
        Self: Sized,
    {
        AuthLayer { inner }
    }
}

#[derive(Debug, Clone)]
pub struct AuthLayer<F> {
    inner: F,
}

impl<F, ReqBody> Layer<ReqBody> for AuthLayer<F>
where
    F: Layer<ReqBody, Error = Infallible, Response = Full<Bytes>>,
    ReqBody: Body + Send,
{
    type Error = F::Error;
    type Response = F::Response;

    fn call(
        &self,
        mut req: http::Request<ReqBody>,
    ) -> impl Future<Output = Result<http::Response<Self::Response>, Self::Error>> {
        if req
            .headers()
            .get(http::header::CONTENT_TYPE)
            .is_some_and(|x| x.to_str().is_ok_and(|x| x != "application/json"))
        {
            return AuthFuture::UnsoportedMediaType;
        }

        let Some(token) = Token::<JwtBoth>::get_token(req.headers()) else {
            return AuthFuture::Aunauthorized;
        };

        let claims = match JwtHandle::verify_token::<Claim<i32>>(&token) {
            Ok(claims) => claims,
            Err(err) => {
                tracing::error!("[Midleware jwt] {err}");
                return AuthFuture::Aunauthorized;
            }
        };
        req.extensions_mut().insert(claims);

        AuthFuture::Pass {
            f: self.inner.call(req),
        }
    }
}

pub enum AuthFuture<F> {
    Aunauthorized,
    UnsoportedMediaType,
    Pass { f: F },
}

impl<F> Future for AuthFuture<F>
where
    F: Future<Output = Result<Response<Full<Bytes>>, Infallible>>,
{
    type Output = F::Output;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        match unsafe { self.get_unchecked_mut() } {
            AuthFuture::Pass { f } => unsafe { Pin::new_unchecked(f) }.poll(cx),
            status_code => Poll::Ready(Ok(Response::builder()
                .status(match status_code {
                    AuthFuture::Aunauthorized => StatusCode::UNAUTHORIZED,
                    AuthFuture::UnsoportedMediaType => StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    _ => StatusCode::BAD_GATEWAY,
                })
                .body(Full::default())
                .unwrap_or_default())),
        }
    }
}
