use std::convert::Infallible;
use http_body_util::Full;
use hyper::{Method, StatusCode, body::{Bytes, Incoming}};
use utils::{JwtBoth, JwtHandle, Token, VerifyTokenEcdsa, middleware::Layer};

use crate::{handler::{PREFIX_PATH, api, error::ResponseErr, login}, models::user::Claim};

#[derive(Debug, Clone)]
pub struct EPoint;

impl Layer<Incoming, Full<Bytes>> for EPoint  {
    type Error = Infallible;

    async fn call(&self, mut req: hyper::Request<Incoming>) -> Result<hyper::Response<Full<Bytes>>, Self::Error> {
        let url = req.uri().path();
        let resp = match (url, req.method()) {
            ("/login", &Method::POST) => login::login(req).await,
            (path, _) if path.starts_with(PREFIX_PATH) => {
                let Some(token) = Token::<JwtBoth>::get_token(req.headers()) else {
                    return Ok(ResponseErr::new("Token not found", StatusCode::UNAUTHORIZED).into());
                };

                let claim = match JwtHandle::verify_token::<Claim>(&token) {
                    Ok(claim) => claim,
                    Err(err) => return Ok(ResponseErr::new(err, StatusCode::UNAUTHORIZED).into()),
                };

                req.extensions_mut().insert(claim);
                api(req).await
            }
            _ => Err(ResponseErr::new("Path not found", StatusCode::BAD_REQUEST)),
        };

        Ok(match resp {
            Ok(e) => e,
            Err(er) => er.into(),
        })
    }
}