use crate::middleware::cors::layer::CorsLayer;
use http::{HeaderValue, Method};
use std::collections::HashSet;

use super::{Any, Origin};

pub struct NoNext;

pub struct CorsBuilder<T> {
    allow_origin: T,
    allow_methods: HashSet<Method>,
    allow_credentials: Option<bool>,
    allow_headers: HashSet<HeaderValue>,
}

impl std::default::Default for CorsBuilder<Any> {
    fn default() -> Self {
        CorsBuilder {
            allow_origin: Any,
            allow_credentials: None,
            allow_headers: Default::default(),
            allow_methods: Default::default(),
        }
    }
}

impl<T> CorsBuilder<T> {
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
}

impl CorsBuilder<Any> {
    pub fn allow_origin<T: Into<String>>(self, origin: T) -> CorsBuilder<Origin> {
        let Self {
            allow_origin: Any,
            allow_methods,
            allow_credentials,
            allow_headers,
        } = self;
        let mut allow_origin = Origin::new();
        allow_origin.push(origin.into()).unwrap();
        CorsBuilder {
            allow_origin,
            allow_methods,
            allow_credentials,
            allow_headers,
        }
    }
}

impl CorsBuilder<Origin> {
    pub fn allow_origin<T: Into<String>>(mut self, origin: T) -> CorsBuilder<Origin> {
        self.allow_origin.push(origin.into()).unwrap();

        self
    }

    pub fn allow_credentials(mut self, credentials: bool) -> Self {
        self.allow_credentials = Some(credentials);

        self
    }
}

impl CorsBuilder<Origin> {
    pub fn build(self) -> CorsLayer {
        let CorsBuilder {
            allow_origin: Origin(origin),
            allow_methods,
            allow_credentials,
            allow_headers,
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
        CorsLayer {
            origin,
            methods: allow_methods,
            headers: allow_headers,
            credential: allow_credentials,
        }
    }
}
