use super::{Arc, Incoming, Request, ResponseError, ResponseWithError, StatusCode};
use crate::repository::Repository;
use http::Method;

pub async fn user(req: Request<Incoming>, repository: Arc<Repository>) -> ResponseWithError {
    let path = req.uri().path().split("/user").collect::<Vec<_>>();
    let method = req.method();

    if path.is_empty() {
        match *method {
            Method::POST => return insert(repository).await,
            Method::PATCH => return update(repository).await,
            Method::DELETE => return delete(repository).await,
            Method::GET => return get(repository).await,
            _ => {}
        }
    }

    Err(ResponseError::new(StatusCode::BAD_REQUEST, "".to_string()))
}

pub async fn insert(repository: Arc<Repository>) -> ResponseWithError {
    Err(ResponseError::new(StatusCode::BAD_REQUEST, "".to_string()))
}

pub async fn update(repository: Arc<Repository>) -> ResponseWithError {
    Err(ResponseError::new(StatusCode::BAD_REQUEST, "".to_string()))
}

pub async fn delete(repository: Arc<Repository>) -> ResponseWithError {
    Err(ResponseError::new(StatusCode::BAD_REQUEST, "".to_string()))
}

pub async fn get(repository: Arc<Repository>) -> ResponseWithError {
    Err(ResponseError::new(StatusCode::BAD_REQUEST, "".to_string()))
}
