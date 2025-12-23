mod builder;
mod future;
pub mod layer;

pub use builder::*;
use http::{HeaderMap, HeaderName, HeaderValue, Method, Request, Response, StatusCode, header};
use hyper::body::Body;

use crate::middleware::{Layer, cors::future::Kind};

#[derive(Debug, Clone)]
pub struct Cors<L> {
    origin: Vec<String>,
    methods: (HeaderName, HeaderValue),
    headers: (HeaderName, HeaderValue),
    credential: Option<(HeaderName, HeaderValue)>,
    inner: L,
}

impl<L, Res, Req> Layer<Req, Res> for Cors<L>
where
    L: Layer<Req, Res>,
    Res: Body + Default + Send,
    Req: Body + Send,
{
    type Error = L::Error;

    fn call(&self, req: Request<Req>) -> impl Future<Output = Result<Response<Res>, Self::Error>> {
        let mut header_map = HeaderMap::new();

        let Some(origin) = req
            .headers()
            .get(header::ORIGIN)
            .filter(|x| self.origin.iter().any(|y| y == *x))
        else {
            return future::CorsFuture {
                kind: Kind::Inmediate {
                    res: Some(
                        Response::builder()
                            .status(StatusCode::OK)
                            .body(<Res as Default>::default())
                            .unwrap_or_default(),
                    ),
                },
            };
        };

        header_map.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin.clone());

        if let Some((k, v)) = self.credential.as_ref() {
            header_map.insert(k, v.clone());
        }

        if req.method() == Method::OPTIONS {
            header_map.insert(self.headers.0.clone(), self.headers.1.clone());
            header_map.insert(self.methods.0.clone(), self.methods.1.clone());

            future::CorsFuture {
                kind: Kind::Preflight {
                    headers: header_map,
                },
            }
        } else {
            future::CorsFuture {
                kind: Kind::Cors {
                    header: header_map,
                    fut: self.inner.call(req),
                },
            }
        }
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
