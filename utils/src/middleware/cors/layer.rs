use hyper::body::Body;

use crate::middleware::{IntoLayer, Layer, cors::Cors};

#[derive(Debug, Clone)]
pub struct CorsLayer {
    pub(super) origin: Vec<String>,
    pub(super) methods: String,
    pub(super) headers: String,
    pub(super) credential: Option<bool>,
}

impl<S, ReqBody, ResBody> IntoLayer<S, ReqBody, ResBody> for CorsLayer 
where 
    ReqBody: Body + Send,
    ResBody: Body + Send + Default,
    S: Layer<ReqBody, ResBody> + Clone,
{
    type Output = Cors<S>;
    
    fn into_layer(self, inner: S) -> Self::Output {
        super::Cors {
            origin: self.origin,
            methods: self.methods,
            headers: self.headers,
            credential: self.credential,
            inner,
        }
    }
}