use super::Repo;
use hyper::{Request, StatusCode, body::Incoming};
use utils::ParseBodyToStruct;
use uuid::Uuid;

use crate::{
    handler::{GetRepo, ResponseHandlers, error::ResponseErr},
    models::{
        UserAllInfo,
        user::{Encrypt, User, update::UpdateSelf},
    },
    repository::{Insert, InsertOwn, QueryOwn, QueryResult, UpdateOwn},
};

pub async fn new(req: Request<Incoming>) -> ResponseHandlers {
    let (parts, body) = req.into_parts();
    let mut user = ParseBodyToStruct::<User>::get(body)
        .await
        .map_err(|x| ResponseErr::new(x, StatusCode::BAD_REQUEST))?;
    let pass = user.passwd;

    user.passwd = tokio::task::spawn_blocking(move || Encrypt::from(&pass))
        .await
        .map_err(|_| ResponseErr::status(StatusCode::INTERNAL_SERVER_ERROR))??;

    let repo = GetRepo::get(&parts.extensions)?;

    Ok(repo.insert_user(InsertOwn::insert(user)).await?.into())
}

pub async fn get(req: Request<Incoming>, id: Uuid) -> ResponseHandlers {
    let repo = GetRepo::get(req.extensions())?;
    let mut user = repo
        .get(QueryOwn::<User>::builder().wh("id", id.into()))
        .await?;

    user.passwd.clear();
    Ok(QueryResult::SelectOne(user).into())
}

pub async fn get_all(req: Request<Incoming>) -> ResponseHandlers {
    let repo = GetRepo::get(req.extensions())?;

    let mut users = repo.gets(QueryOwn::<User>::builder()).await?;
    users.iter_mut().for_each(|x| x.passwd.clear());

    Ok(QueryResult::Select(users).into())
}

pub async fn get_user_info(req: Request<Incoming>, id: Option<Uuid>) -> ResponseHandlers {
    let repo = req.extensions().get::<Repo>().unwrap();
    let query = id
        .map(|x| QueryOwn::<UserAllInfo>::builder().wh("id", x.into()))
        .unwrap_or(QueryOwn::builder());
    let resp = repo.gets(query).await?;

    Ok(QueryResult::Select(resp).into())
}

pub async fn delete(req: Request<Incoming>, id: Uuid) -> ResponseHandlers {
    let repo = GetRepo::get(req.extensions())?;

    Ok(repo.delete(id).await?.into())
}

pub async fn update(req: Request<Incoming>, id: Uuid) -> ResponseHandlers {
    let (parts, body) = req.into_parts();
    let repo = GetRepo::get(&parts.extensions)?;
    let new = ParseBodyToStruct::<UpdateSelf>::get(body)
        .await
        .map_err(|_| ResponseErr::status(StatusCode::BAD_REQUEST))?;

    Ok(repo
        .update(UpdateOwn::<User>::new(id).from(new))
        .await?
        .into())
}
