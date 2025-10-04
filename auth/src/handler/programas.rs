use hyper::{body::Incoming, Request, StatusCode};
use utils::ParseBodyToStruct;
use uuid::Uuid;
use crate::{handler::{error::ResponseErr, GetRepo, ResponseHandlers}, models::programas::{update::ProgramaUpdate, Programa}, repository::UpdateOwn};



pub async fn update(req: Request<Incoming>, id: Uuid) -> ResponseHandlers {
    let (parts, body) = req.into_parts();
    let program: ProgramaUpdate = ParseBodyToStruct::get(body).await.map_err(|_| ResponseErr::status(StatusCode::BAD_REQUEST))?;
    let repo = GetRepo::get(&parts.extensions)?;

    Ok(repo.update(UpdateOwn::<Programa>::new(id).from(program)).await?.into())
}