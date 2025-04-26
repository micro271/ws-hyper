use crate::handlers::{State, from_incoming_to};
use crate::models::user::User;

use super::data_entry::NewUser;
use super::{Incoming, Request, ResponseError, ResponseWithError, StatusCode};
use bytes::Bytes;
use http::{Method, Response};
use http_body_util::Full;

pub async fn user(req: Request<Incoming>) -> ResponseWithError {
    let path = req.uri().path().split("/user").collect::<Vec<_>>();
    let method = req.method();

    if path.is_empty() {
        match *method {
            Method::POST => return insert(req).await,
            Method::PATCH => return update(req).await,
            Method::DELETE => return delete(req).await,
            Method::GET => return get(req).await,
            _ => {}
        }
    }

    Err(ResponseError::new::<&str>(StatusCode::BAD_REQUEST, None))
}

pub async fn insert(req: Request<Incoming>) -> ResponseWithError {
    let (parts, body) = req.into_parts();
    let mut body = from_incoming_to::<NewUser>(body).await?;
    if body.encrypt().is_err() {
        return Err(ResponseError::new::<&str>(
            StatusCode::INTERNAL_SERVER_ERROR,
            None,
        ));
    }
    match parts.extensions.get::<State>() {
        Some(state) => {
            let resp = state.insert::<User>(body.into()).await?;
            Ok(Response::builder()
                .status(StatusCode::CREATED)
                .body(Full::new(Bytes::from(
                    serde_json::json!({"new_element": resp}).to_string(),
                )))
                .unwrap_or_default())
        }
        None => {
            tracing::error!("State is not present in extensios");
            Err(ResponseError::new::<&str>(
                StatusCode::INTERNAL_SERVER_ERROR,
                None,
            ))
        }
    }
}

pub async fn update(req: Request<Incoming>) -> ResponseWithError {
    Err(ResponseError::new::<&str>(StatusCode::BAD_REQUEST, None))
}

pub async fn delete(req: Request<Incoming>) -> ResponseWithError {
    Err(ResponseError::new::<&str>(StatusCode::BAD_REQUEST, None))
}

pub async fn get(req: Request<Incoming>) -> ResponseWithError {
    Err(ResponseError::new::<&str>(StatusCode::BAD_REQUEST, None))
}
