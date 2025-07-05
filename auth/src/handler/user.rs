use std::convert::Infallible;

use http_body_util::Full;
use hyper::{
    Request, Response, StatusCode,
    body::{Bytes, Incoming},
};
use utils::ParseBodyToJson;
use uuid::Uuid;

use crate::{Repository, models::user::User};

pub async fn new(req: Request<Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
    let (_parts, body) = req.into_parts();
    let user = ParseBodyToJson::<User>::get(body).await.unwrap();

    let repo = _parts.extensions.get::<Repository>().unwrap();
    let result = match repo.insert_one_user(user).await {
        Ok(_e) => StatusCode::OK,
        Err(_err) => StatusCode::INTERNAL_SERVER_ERROR,
    };
    Ok(Response::builder()
        .status(result)
        .body(Full::default())
        .unwrap_or_default())
}

pub async fn get(
    req: Request<Incoming>,
    id: Option<Uuid>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let (_parts, body) = req.into_parts();
    let user = ParseBodyToJson::<User>::get(body).await.unwrap();

    let repo = _parts.extensions.get::<Repository>().unwrap();
    let result = match repo.get_user_with_id(id.unwrap()).await {
        Ok(_e) => StatusCode::OK,
        Err(_err) => StatusCode::INTERNAL_SERVER_ERROR,
    };
    Ok(Response::builder()
        .status(result)
        .body(Full::default())
        .unwrap_or_default())
}

pub async fn delete(req: Request<Incoming>, id: Uuid) -> Result<Response<Full<Bytes>>, Infallible> {
    unimplemented!()
}

pub async fn update(req: Request<Incoming>, id: Uuid) -> Result<Response<Full<Bytes>>, Infallible> {
    unimplemented!()
}
