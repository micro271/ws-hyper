use super::Cors;

use std::{collections::HashSet, convert::Infallible, marker::PhantomData};

use http::{HeaderValue, Method, Request, Response};
use hyper::body::Body;

use super::{Any, Origin};

pub struct Next<F>(F);

pub struct NoNext;

pub struct CorsBuilder<T, F, Res, Req> {
    allow_origin: T,
    allow_methods: HashSet<Method>,
    allow_credentials: Option<bool>,
    allow_headers: HashSet<HeaderValue>,
    next: F,
    _ph: PhantomData<(Res, Req)>,
}

impl<Res, Req> std::default::Default for CorsBuilder<Any, NoNext, Res, Req> 
where 
    Res: Body + Default,
    Req: Body,
{
    fn default() -> Self {
        CorsBuilder {
            allow_origin: Any,
            allow_credentials: None,
            allow_headers: Default::default(),
            allow_methods: Default::default(),
            next: NoNext,
            _ph: PhantomData,
        }
    }
}

impl<T, F, Res, Req> CorsBuilder<T, F, Res, Req> 
where 
    Res: Body + Default,
    Req: Body,
{
    pub fn allow_method(mut self, methods: Method) -> Self {
        self.allow_methods.insert(methods);

        self
    }

    pub fn allow_header<V>(mut self, key: V) -> Self 
    where
        V: Into<HeaderValue>
    {
        self.allow_headers.insert(key.into());

        self
    }

    pub fn next<NextFn>(self, next_fn: NextFn) -> CorsBuilder<T, Next<NextFn>, Res, Req> 
    where 
        NextFn: AsyncFn(Request<Req>) -> Result<Response<Res>, Infallible>,
        Res: Body + Default,
    {
        let Self { allow_origin, allow_methods, allow_credentials, allow_headers, next:_, _ph } = self;
        
        CorsBuilder { allow_origin, allow_methods, allow_credentials, allow_headers, next: Next(next_fn), _ph }
    }
}

impl<F, Res, Req> CorsBuilder<Any, F, Res, Req> 
where
    Res: Body + Default,
    Req: Body,
{
    pub fn allow_origin<T: Into<String>>(self, origin: T) -> CorsBuilder<Origin, F, Res, Req> {
        let Self { allow_origin: Any, allow_methods, allow_credentials, allow_headers, next, _ph } = self;
        let mut allow_origin = Origin::new();
        allow_origin.push(origin.into()).unwrap();
        CorsBuilder { allow_origin, allow_methods, allow_credentials, allow_headers, next, _ph }
    }
}

impl<F, Res, Req> CorsBuilder<Origin, F, Res, Req> 
where
    Res: Body + Default,
    Req: Body,
{
    pub fn allow_origin<T: Into<String>>(mut self, origin: T) -> CorsBuilder<Origin, F, Res, Req> {
        self.allow_origin.push(origin.into()).unwrap();
        
        self
    }

    pub fn allow_credentials(mut self, credentials: bool) -> Self {
        self.allow_credentials = Some(credentials);

        self
    }
}

impl<F, Res, Req> CorsBuilder<Origin, Next<F>, Res, Req> 
where 
    F: AsyncFn(Request<Req>) -> Result<Response<Res>, Infallible>,
    Res: Body + Default,
    Req: Body,
{
    pub fn build(self) -> Cors<F, Res, Req> {
        let CorsBuilder { allow_origin: Origin(origin), allow_methods, allow_credentials, allow_headers, next: Next(next), _ph } = self;
        let allow_methods = allow_methods.iter().map(|x| x.as_str()).collect::<Vec<_>>().join(", ");
        let allow_methods = allow_methods.is_empty().then_some("*".to_string()).unwrap_or(allow_methods);
        let allow_headers = allow_headers.iter().map(|x| x.to_str().unwrap()).collect::<Vec<_>>().join(", ");
        let allow_headers = allow_headers.is_empty().then_some("*".to_string()).unwrap_or(allow_headers);
        Cors { origin, methods: allow_methods, headers: allow_headers, credential: allow_credentials, next, _ph }
    }
}