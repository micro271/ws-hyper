use crate::{
    handler::{error::ResponseErr, GetRepo, ResponseHandlers},
    models::programas::{update::ProgramaUpdate, Programa},
    repository::{Insert, InsertOwn, QueryOwn, QueryResult, UpdateOwn},
};
use hyper::{Request, StatusCode, body::Incoming};
use utils::ParseBodyToStruct;
use uuid::Uuid;

pub async fn new(req: Request<Incoming>) -> ResponseHandlers {
    let (parts, body) = req.into_parts();
    let programa: Programa = ParseBodyToStruct::get(body).await.map_err(|x| ResponseErr::new(x,StatusCode::BAD_GATEWAY))?;
    // This action requires creating a new directory to store the videos
    Ok(
        GetRepo::get(&parts.extensions)?.insert(InsertOwn::insert(programa)).await?.into()
    )
}

pub async fn update(req: Request<Incoming>, id: Uuid) -> ResponseHandlers {
    let (parts, body) = req.into_parts();
    let program: ProgramaUpdate = ParseBodyToStruct::get(body)
        .await
        .map_err(|_| ResponseErr::status(StatusCode::BAD_REQUEST))?;
    // This action requires modifying the directory name
    Ok(GetRepo::get(&parts.extensions)?
        .update(UpdateOwn::<'_, Programa>::new().wh("id", id).from(program))
        .await?
        .into())
}

pub async fn get(req: Request<Incoming>, id: Uuid) -> ResponseHandlers {
    Ok(QueryResult::SelectOne(
        GetRepo::get(req.extensions())?
            .get(QueryOwn::<Programa>::builder().wh("id", id))
            .await?,
    )
    .into())
}

pub async fn get_all(req: Request<Incoming>) -> ResponseHandlers {
    Ok(QueryResult::Select(
        GetRepo::get(req.extensions())?
            .gets(QueryOwn::<Programa>::builder())
            .await?,
    )
    .into())
}

pub async fn delete(req: Request<Incoming>, id: Uuid) -> ResponseHandlers {
    // If the directory of the program have elements, we'll need to force to delete all elements even main directory.
    let _force = url::form_urlencoded::parse(req.uri().query().unwrap_or_default().as_bytes())
        .find(|(k, _)| k == "force")
        .and_then(|(_, v)| v.parse::<bool>().ok())
        .unwrap_or(false);

    Ok(GetRepo::get(req.extensions())?.delete(id).await?.into())
}
