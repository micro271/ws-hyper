use super::Repo;
use hyper::{Request, StatusCode, body::Incoming};
use utils::ParseBodyToJson;
use uuid::Uuid;

use crate::{
    handler::{ResponseHandlers, error::ResponseErr},
    models::user::User,
    repository::{Insert, InsertOwn, QueryOwn, QueryResult},
};

pub async fn new(req: Request<Incoming>) -> ResponseHandlers {
    let (_parts, body) = req.into_parts();
    let mut user = ParseBodyToJson::<User>::get(body)
        .await
        .map_err(|x| ResponseErr::new(x, StatusCode::BAD_REQUEST))?;

    user.encrypt_passwd()?;

    let repo = _parts.extensions.get::<Repo>().unwrap();

    Ok(repo.insert_user(InsertOwn::insert(user)).await?.into())
}

pub async fn get(req: Request<Incoming>, id: Uuid) -> ResponseHandlers {
    let repo = req.extensions().get::<Repo>().unwrap();

    Ok(QueryResult::SelectOne(
        repo.get(QueryOwn::<User>::builder().wh("id", id.into()))
            .await?,
    )
    .into())
}

pub async fn get_many(req: Request<Incoming>, id: Option<Uuid>) -> ResponseHandlers {
    let repo = req.extensions().get::<Repo>().unwrap();

    let mut builder = QueryOwn::<User>::builder();

    if let Some(id) = id {
        builder = builder.wh("id", id.into());
    }

    Ok(QueryResult::Select(repo.gets(builder).await?).into())
}

pub async fn delete(req: Request<Incoming>, _id: Uuid) -> ResponseHandlers {
    let _repo = req
        .extensions()
        .get::<Repo>()
        .ok_or(ResponseErr::status(StatusCode::INTERNAL_SERVER_ERROR))?;

    todo!()
}

pub async fn update(_req: Request<Incoming>, _id: Uuid) -> ResponseHandlers {
    Err(ResponseErr::status(StatusCode::NOT_IMPLEMENTED))
}
