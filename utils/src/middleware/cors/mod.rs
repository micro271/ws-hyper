mod builder;
pub mod layer;

pub use builder::*;
use http::{HeaderValue, Method, Request, Response, StatusCode, header};
use hyper::body::Body;

use crate::middleware::Layer;

#[derive(Debug, Clone)]
pub struct Cors<L> {
    origin: Vec<String>,
    methods: String,
    headers: String,
    credential: Option<bool>,
    inner: L,
}

impl<L, Res, Req> Layer<Req, Res> for Cors<L>
where
    L: Layer<Req, Res>,
    Res: Body + Default + Send,
    Req: Body + Send,
{
    type Error = L::Error;
    async fn call(&self, req: Request<Req>) -> Result<Response<Res>, Self::Error> {
        let Some(origin) = req
            .headers()
            .get(header::ORIGIN)
            .and_then(|x| x.to_str().ok())
        else {
            return Ok(Response::builder()
                .status(StatusCode::FORBIDDEN)
                .body(<Res as Default>::default())
                .unwrap_or_default());
        };

        if !self.origin.iter().any(|x| x.as_str() == origin) {
            return Ok(Response::builder()
                .status(StatusCode::OK)
                .body(<Res as Default>::default())
                .unwrap_or_default());
        };

        let resp = if req.method() == Method::OPTIONS {
            let mut r = Response::builder()
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin)
                .header(
                    header::ACCESS_CONTROL_ALLOW_HEADERS,
                    HeaderValue::from_str(&self.headers).unwrap(),
                )
                .header(
                    header::ACCESS_CONTROL_ALLOW_METHODS,
                    HeaderValue::from_str(&self.methods).unwrap(),
                )
                .status(StatusCode::OK)
                .body(<Res as Default>::default())
                .unwrap_or_default();
            if let Some(cred) = self.credential {
                r.headers_mut().insert(
                    header::ACCESS_CONTROL_ALLOW_CREDENTIALS,
                    HeaderValue::from_str(&cred.to_string()).unwrap(),
                );
            }

            Ok(r)
        } else {
            let origin = origin.to_string();
            self.inner.call(req).await.map(|mut x| {
                let header = x.headers_mut();
                if let Some(cred) = self.credential {
                    header.insert(
                        header::ACCESS_CONTROL_ALLOW_CREDENTIALS,
                        HeaderValue::from_str(&cred.to_string()).unwrap(),
                    );
                }
                header.insert(
                    header::ACCESS_CONTROL_ALLOW_ORIGIN,
                    HeaderValue::from_str(origin.as_str()).unwrap(),
                );
                x
            })
        };

        resp
    }
}

pub struct Any;

#[derive(Default)]
pub struct Origin(Vec<String>);

impl Origin {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push<T: Into<String> + PartialEq<&'static str>>(
        &mut self,
        origin: T,
    ) -> Result<(), OriginError> {
        if origin == "*" {
            return Err(OriginError);
        }
        self.0.push(origin.into());
        Ok(())
    }
}

#[derive(Debug)]
pub struct OriginError;

impl std::fmt::Display for OriginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Invalid origin")
    }
}

impl std::error::Error for OriginError {}
