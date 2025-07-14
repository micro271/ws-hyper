use hyper::{Request, StatusCode, body::Incoming};
use utils::ParseBodyToJson;
use uuid::Uuid;

use crate::{
    Repository,
    handler::{ResponseHandlers, error::ResponseErr},
    models::user::{Claim, Role, User},
    repository::{QueryResult, Types},
};

pub async fn new(req: Request<Incoming>) -> ResponseHandlers {
    let (_parts, body) = req.into_parts();
    let mut user = ParseBodyToJson::<User>::get(body)
        .await
        .map_err(|x| ResponseErr::new(x, StatusCode::BAD_REQUEST))?;

    user.encrypt_passwd()?;

    let repo = _parts.extensions.get::<Repository>().unwrap();

    Ok(repo.insert_user(user).await?.into())
}

pub async fn get(req: Request<Incoming>, id: Option<Uuid>) -> ResponseHandlers {
    let repo = req.extensions().get::<Repository>().unwrap();
    let claim = req.extensions().get::<Claim>().unwrap();
    let Ok(QueryResult::SelectOne(user)) = repo.get_user("id", claim.sub.into()).await else {
        return Err(ResponseErr::status(StatusCode::INTERNAL_SERVER_ERROR));
    };

    match id {
        Some(e) if e == user.id.unwrap() => {
            Ok(repo.get_user_pub("id", Types::Uuid(e)).await?.into())
        }
        None if user.role == Role::Administrator => Ok(repo.get_all().await?.into()),
        _ => Err(ResponseErr::status(StatusCode::BAD_REQUEST)),
    }
}

pub async fn delete(req: Request<Incoming>, id: Uuid) -> ResponseHandlers {
    let repo = req
        .extensions()
        .get::<Repository>()
        .ok_or(ResponseErr::status(StatusCode::INTERNAL_SERVER_ERROR))?;

    Ok(repo.delete::<User>("id", Types::Uuid(id)).await?.into())
}

pub async fn update(_req: Request<Incoming>, _id: Uuid) -> ResponseHandlers {
    Err(ResponseErr::status(StatusCode::NOT_IMPLEMENTED))
}
