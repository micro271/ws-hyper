use std::collections::HashMap;

use super::Repo;
use hyper::{Request, StatusCode, body::Incoming};
use serde::de::DeserializeOwned;
use utils::ParseBodyToStruct;
use uuid::Uuid;

use crate::{
    handler::{GetRepo, ResponseHandlers, error::ResponseErr},
    models::{
        UserAllInfo,
        user::{Encrypt, User},
    },
    repository::{Insert, InsertOwn, QueryOwn, QueryResult, TABLA_USER, Types, UpdateOwn},
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
    let mut user = repo.get(QueryOwn::<User>::builder().wh("id", id)).await?;

    user.passwd.clear();
    Ok(QueryResult::SelectOne(user).into())
}

pub async fn get_all(req: Request<Incoming>) -> ResponseHandlers {
    let repo = GetRepo::get(req.extensions())?;

    let mut users = repo.gets(QueryOwn::<User>::builder()).await?;
    for x in &mut users {
        x.passwd.clear();
    }

    Ok(QueryResult::Select(users).into())
}

pub async fn get_user_info(req: Request<Incoming>, id: Option<Uuid>) -> ResponseHandlers {
    let repo = req.extensions().get::<Repo>().unwrap();
    let key = format!("{TABLA_USER}.id");
    let query = id.map_or(QueryOwn::builder(), |x| {
        QueryOwn::<UserAllInfo>::builder().wh(&key, x)
    });
    let resp = repo.gets(query).await?;

    Ok(QueryResult::Select(resp).into())
}

pub async fn delete(req: Request<Incoming>, id: Uuid) -> ResponseHandlers {
    let repo = GetRepo::get(req.extensions())?;

    Ok(repo.delete(id).await?.into())
}

pub async fn update<T>(req: Request<Incoming>, id: Uuid) -> ResponseHandlers
where
    T: Into<HashMap<&'static str, Types>> + DeserializeOwned,
{
    let (parts, body) = req.into_parts();
    let repo = GetRepo::get(&parts.extensions)?;
    let new = ParseBodyToStruct::<T>::get(body)
        .await
        .map_err(|x| ResponseErr::new(x, StatusCode::BAD_REQUEST))?;

    Ok(repo
        .update(UpdateOwn::<'_, User>::new().from(new).wh("id", id))
        .await?
        .into())
}
