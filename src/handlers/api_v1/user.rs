use crate::handlers::{State, from_incoming_to};
use crate::models::user::User;

use super::data_entry::NewUser;
use super::{Incoming, Request, ResponseError, ResponseWithError, StatusCode};
use bytes::Bytes;
use http::{Method, Response, header};
use http_body_util::Full;
use mongodb::bson::doc;
use serde_json::json;

pub async fn user(req: Request<Incoming>) -> ResponseWithError {
    let method = req.method();

    match *method {
        Method::POST => insert(req).await,
        Method::PATCH => update(req).await,
        Method::DELETE => delete(req).await,
        Method::GET => get(req).await,
        _ => Err(ResponseError::new::<&str>(StatusCode::BAD_REQUEST, None)),
    }
}

pub async fn insert(req: Request<Incoming>) -> ResponseWithError {
    let (parts, body) = req.into_parts();
    let mut body = from_incoming_to::<NewUser>(body).await?;
    if let Err(e) = body.encrypt() {
        tracing::error!("Encryption error - {e}");
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
    match req.extensions().get::<State>() {
        Some(state) => {
            let len = state.delete::<User>(doc! {}).await?;
            Ok(Response::builder()
                .header(header::CONTENT_TYPE, "application/json")
                .status(StatusCode::OK)
                .body(Full::new(Bytes::from(
                    json!({
                        "document_affects": len
                    })
                    .to_string(),
                )))
                .unwrap_or_default())
        }
        _ => {
            tracing::error!("State not defined");
            Err(ResponseError::new::<&str>(
                StatusCode::INTERNAL_SERVER_ERROR,
                None,
            ))
        }
    }
}

pub async fn get(req: Request<Incoming>) -> ResponseWithError {
    match req.extensions().get::<State>() {
        Some(state) => {
            let user = state.get_one::<User>(doc! {}).await?;
            Ok(Response::builder()
                .header(header::CONTENT_TYPE, "application/json")
                .status(StatusCode::OK)
                .body(Full::new(Bytes::from(
                    json!({
                        "data": user,
                        "length": 1,
                    })
                    .to_string(),
                )))
                .unwrap_or_default())
        }
        _ => {
            tracing::error!("State not defined");
            Err(ResponseError::new::<&str>(
                StatusCode::INTERNAL_SERVER_ERROR,
                None,
            ))
        }
    }
}
