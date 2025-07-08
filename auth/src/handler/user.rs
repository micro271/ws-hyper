use http_body_util::Full;
use hyper::{Request, Response, StatusCode, body::Incoming};
use utils::ParseBodyToJson;
use uuid::Uuid;

use crate::{
    Repository,
    handler::{ResponseHandlers, error::ResponseErr},
    models::user::User,
};

pub async fn new(req: Request<Incoming>) -> ResponseHandlers {
    let (_parts, body) = req.into_parts();
    let user = ParseBodyToJson::<User>::get(body)
        .await
        .map_err(|x| ResponseErr::new(x, StatusCode::BAD_REQUEST))?;

    let repo = _parts.extensions.get::<Repository>().unwrap();
    let result = match repo.insert_user(user).await {
        Ok(_e) => StatusCode::OK,
        Err(_err) => StatusCode::INTERNAL_SERVER_ERROR,
    };
    Ok(Response::builder()
        .status(result)
        .body(Full::default())
        .unwrap_or_default())
}

pub async fn get(req: Request<Incoming>, id: Option<Uuid>) -> ResponseHandlers {
    let (_parts, body) = req.into_parts();

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

pub async fn delete(req: Request<Incoming>, id: Uuid) -> ResponseHandlers {
    unimplemented!()
}

pub async fn update(req: Request<Incoming>, id: Uuid) -> ResponseHandlers {
    unimplemented!()
}
