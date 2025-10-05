use crate::{
    handler::{GetRepo, ResponseHandlers, error::ResponseErr},
    models::programas::{Programa, update::ProgramaUpdate},
    repository::{QueryOwn, QueryResult, UpdateOwn},
};
use hyper::{Request, StatusCode, body::Incoming};
use utils::ParseBodyToStruct;
use uuid::Uuid;

pub async fn new(_req: Request<Incoming>) -> ResponseHandlers {
    unimplemented!()
}

pub async fn update(req: Request<Incoming>, id: Uuid) -> ResponseHandlers {
    let (parts, body) = req.into_parts();
    let program: ProgramaUpdate = ParseBodyToStruct::get(body)
        .await
        .map_err(|_| ResponseErr::status(StatusCode::BAD_REQUEST))?;
    let repo = GetRepo::get(&parts.extensions)?;

    Ok(repo
        .update(UpdateOwn::<Programa>::new(id).from(program))
        .await?
        .into())
}

pub async fn get(req: Request<Incoming>, id: Uuid) -> ResponseHandlers {
    let repo = GetRepo::get(req.extensions())?;

    Ok(QueryResult::SelectOne(
        repo.get(QueryOwn::<Programa>::builder().wh("id", id.into()))
            .await?,
    )
    .into())
}

pub async fn get_all(req: Request<Incoming>) -> ResponseHandlers {
    let repo = GetRepo::get(req.extensions())?;

    Ok(QueryResult::Select(repo.gets(QueryOwn::<Programa>::builder()).await?).into())
}

pub async fn delete(_req: Request<Incoming>, _id: Uuid) -> ResponseHandlers {
    unimplemented!()
}
