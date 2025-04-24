use super::{Arc, Incoming, Request, ResponseError, ResponseWithError, StatusCode};
use crate::repository::Repository;
use http::Method;

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

    Err(ResponseError::new(StatusCode::BAD_REQUEST, "".to_string()))
}

pub async fn insert(req: Request<Incoming>) -> ResponseWithError {
    Err(ResponseError::new(StatusCode::BAD_REQUEST, "".to_string()))
}

pub async fn update(req: Request<Incoming>) -> ResponseWithError {
    Err(ResponseError::new(StatusCode::BAD_REQUEST, "".to_string()))
}

pub async fn delete(req: Request<Incoming>) -> ResponseWithError {
    Err(ResponseError::new(StatusCode::BAD_REQUEST, "".to_string()))
}

pub async fn get(req: Request<Incoming>) -> ResponseWithError {
    Err(ResponseError::new(StatusCode::BAD_REQUEST, "".to_string()))
}
