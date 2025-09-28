use hyper::{Request, StatusCode, body::Incoming};
use utils::ParseBodyToJson;
use uuid::Uuid;

use crate::{
    Repository,
    handler::{ResponseHandlers, error::ResponseErr},
    models::user::{Claim, User},
    repository::{QueryOwn, QueryResult},
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
    let id_user = req.extensions().get::<Claim>().unwrap().sub;
    let user = repo.get(QueryOwn::<User>::builder().wh("id", id_user.into())).await?;

    if id.is_some_and(|x| x != id_user) && user.is_admin() {
        return Err(ResponseErr::status(StatusCode::UNAUTHORIZED));
    }

    let mut builder = QueryOwn::<User>::builder();

    if let Some(id) = id {
        builder = builder.wh("id", id.into());
    } 

    Ok(QueryResult::SelectOne(
        repo.get(builder)
            .await?,
    )
    .into())
}

pub async fn delete(req: Request<Incoming>, _id: Uuid) -> ResponseHandlers {
    let _repo = req
        .extensions()
        .get::<Repository>()
        .ok_or(ResponseErr::status(StatusCode::INTERNAL_SERVER_ERROR))?;

    todo!()
}

pub async fn update(_req: Request<Incoming>, _id: Uuid) -> ResponseHandlers {
    Err(ResponseErr::status(StatusCode::NOT_IMPLEMENTED))
}
