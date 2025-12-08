use super::Cors;
use crate::middleware::Next;
use http::{HeaderValue, Method, Request, Response};
use hyper::body::Body;
use std::{collections::HashSet, convert::Infallible, marker::PhantomData};

use super::{Any, Origin};

pub struct NoNext;

pub struct CorsBuilder<T, F> {
    allow_origin: T,
    allow_methods: HashSet<Method>,
    allow_credentials: Option<bool>,
    allow_headers: HashSet<HeaderValue>,
    next: F,
}

impl std::default::Default for CorsBuilder<Any, NoNext> {
    fn default() -> Self {
        CorsBuilder {
            allow_origin: Any,
            allow_credentials: None,
            allow_headers: Default::default(),
            allow_methods: Default::default(),
            next: NoNext,
        }
    }
}

impl<T, F> CorsBuilder<T, F> {
    pub fn allow_method(mut self, methods: Method) -> Self {
        self.allow_methods.insert(methods);

        self
    }

    pub fn allow_header<V>(mut self, key: V) -> Self
    where
        V: Into<HeaderValue>,
    {
        self.allow_headers.insert(key.into());

        self
    }

    pub fn next<NextFn, Fut, Req, Res>(self, next_fn: NextFn) -> CorsBuilder<T, Next<NextFn>>
    where
        NextFn: Fn(Request<Req>) -> Fut,
        Fut: Future<Output = Result<Response<Res>, Infallible>> + Send,
        Res: Body + Default + Sync + Send,
        Req: Body + Sync + Send,
    {
        let Self {
            allow_origin,
            allow_methods,
            allow_credentials,
            allow_headers,
            next: _,
        } = self;

        CorsBuilder {
            allow_origin,
            allow_methods,
            allow_credentials,
            allow_headers,
            next: Next(next_fn),
        }
    }
}

impl<F> CorsBuilder<Any, F> {
    pub fn allow_origin<T: Into<String>>(self, origin: T) -> CorsBuilder<Origin, F> {
        let Self {
            allow_origin: Any,
            allow_methods,
            allow_credentials,
            allow_headers,
            next,
        } = self;
        let mut allow_origin = Origin::new();
        allow_origin.push(origin.into()).unwrap();
        CorsBuilder {
            allow_origin,
            allow_methods,
            allow_credentials,
            allow_headers,
            next,
        }
    }
}

impl<F> CorsBuilder<Origin, F> {
    pub fn allow_origin<T: Into<String>>(mut self, origin: T) -> CorsBuilder<Origin, F> {
        self.allow_origin.push(origin.into()).unwrap();

        self
    }

    pub fn allow_credentials(mut self, credentials: bool) -> Self {
        self.allow_credentials = Some(credentials);

        self
    }
}

impl<F> CorsBuilder<Origin, Next<F>> {
    pub fn build<Fut, Res, Req>(self) -> Cors<F, Res, Req>
    where
        F: Fn(Request<Req>) -> Fut,
        Fut: Future<Output = Result<Response<Res>, Infallible>> + Send,
        Res: Body + Default + Send + Sync,
        Req: Body + Send + Sync,
    {
        let CorsBuilder {
            allow_origin: Origin(origin),
            allow_methods,
            allow_credentials,
            allow_headers,
            next: Next(next),
        } = self;
        let allow_methods = allow_methods
            .iter()
            .map(|x| x.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let allow_methods = allow_methods
            .is_empty()
            .then_some("*".to_string())
            .unwrap_or(allow_methods);
        let allow_headers = allow_headers
            .iter()
            .map(|x| x.to_str().unwrap())
            .collect::<Vec<_>>()
            .join(", ");
        let allow_headers = allow_headers
            .is_empty()
            .then_some("*".to_string())
            .unwrap_or(allow_headers);
        Cors {
            origin,
            methods: allow_methods,
            headers: allow_headers,
            credential: allow_credentials,
            next,
            _ph: PhantomData,
        }
    }
}
