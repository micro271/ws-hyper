use crate::handlers::utils::get_user_oid;
use crate::models::user::{Claims, Encrypt, Role, User};
use crate::{
    handlers::{
        State,
        utils::{from_incoming_to, get_extention},
    },
    models::user::UpdateUser,
};

use super::data_entry::NewUser;
use super::{Incoming, Request, ResponseError, ResultResponse, StatusCode};
use bytes::Bytes;
use http::{Method, Response, header};
use http_body_util::Full;
use mongodb::bson::doc;
use mongodb::bson::oid::ObjectId;
use serde_json::json;

pub async fn user(req: Request<Incoming>) -> ResultResponse {
    let method = req.method();
    let user = req
        .uri()
        .path()
        .split("/user/")
        .nth(1)
        .and_then(|x| x.parse().ok());

    match (method, user) {
        (&Method::POST, None) => insert(req).await,
        (&Method::PATCH, Some(user)) => update(req, user).await,
        (&Method::DELETE, Some(user)) => delete(req, user).await,
        (&Method::GET, user) => get(req, user).await,
        _ => Err(ResponseError::new(
            StatusCode::BAD_REQUEST,
            Some(format!("{} is not a valid endpoint", req.uri())),
        )),
    }
}

pub async fn insert(req: Request<Incoming>) -> ResultResponse {
    let (parts, body) = req.into_parts();
    let claims = get_extention::<Claims>(&parts.extensions).unwrap();

    if claims.role != Role::Admin {
        return Err(ResponseError::new(
            StatusCode::UNAUTHORIZED,
            Some("The user is not an admin"),
        ));
    }

    let mut user = from_incoming_to::<NewUser>(body).await?;
    user.password = user.password.encrypt()?;

    let state = get_extention::<State>(&parts.extensions)?;

    let resp = state.insert::<User>(user.into()).await?;

    Ok(Response::builder()
        .status(StatusCode::CREATED)
        .body(Full::new(Bytes::from(
            serde_json::json!({"new_element": resp}).to_string(),
        )))
        .unwrap_or_default())
}

pub async fn update(req: Request<Incoming>, user: ObjectId) -> ResultResponse {
    let (parts, body) = req.into_parts();
    let state = get_extention::<State>(&parts.extensions)?; //todo create UpdateUser
    let new_user = from_incoming_to::<UpdateUser>(body).await?;

    let claims = get_extention::<Claims>(&parts.extensions).unwrap();
    let current_user = get_user_oid(claims)?;

    if claims.role == Role::Admin {
        if current_user == user && new_user.role.is_some_and(|x| x != Role::Admin) {
            return Err(ResponseError::new(
                StatusCode::FORBIDDEN,
                Some("You cannot change the role of the admin user"),
            ));
        }

        state
            .update::<User>(new_user.try_into()?, doc! { "_id": user })
            .await?;
        Ok(Response::builder()
            .status(StatusCode::OK)
            .body(Full::new(Bytes::new()))
            .unwrap_or_default())
    } else if new_user.username.is_some() {
        Err(ResponseError::new(
            StatusCode::UNAUTHORIZED,
            Some("You do not have permission to update the username"),
        ))
    } else {
        state
            .update::<User>(new_user.try_into()?, doc! {"_id": user})
            .await?;
        Ok(Response::builder()
            .status(StatusCode::OK)
            .body(Full::new(Bytes::new()))
            .unwrap_or_default())
    }
}

pub async fn delete(req: Request<Incoming>, user: ObjectId) -> ResultResponse {
    let claims = get_extention::<Claims>(req.extensions())?;

    if claims.role != Role::Admin {
        return Err(ResponseError::new(
            StatusCode::UNAUTHORIZED,
            Some("You do not have permission to delete any element"),
        ));
    }

    let state = get_extention::<State>(req.extensions())?;
    let len = state.delete::<User>(doc! {"_id": user}).await?;

    Ok(Response::builder()
        .header(header::CONTENT_TYPE, "application/json")
        .status(StatusCode::OK)
        .body(Full::new(Bytes::from(
            json!({
                "document_affects": len
            })
            .to_string(),
        )))
        .unwrap_or_default())
}

pub async fn get(req: Request<Incoming>, get_user: Option<ObjectId>) -> ResultResponse {
    let state = get_extention::<State>(req.extensions())?;
    let claims = get_extention::<Claims>(req.extensions()).unwrap();
    let current_user = get_user_oid(claims)?;

    let filter = if claims.role == Role::Admin {
        get_user.map(|user| doc! {"_id": user}).unwrap_or_default()
    } else if let Some(user) = get_user.filter(|x| x == &current_user) {
        doc! {"_id": user}
    } else {
        return Err(ResponseError::new(
            StatusCode::FORBIDDEN,
            Some("You cannot see other users"),
        ));
    };

    let mut user = state.get::<User>(filter).await?;

    Ok(Response::builder()
        .header(header::CONTENT_TYPE, "application/json")
        .status(StatusCode::OK)
        .body(Full::new(Bytes::from(
            if user.len() == 1 {
                json!({
                    "data": user.pop(),
                    "length": 1,
                })
            } else {
                json!({
                    "data": user,
                    "length": user.len(),
                })
            }
            .to_string(),
        )))
        .unwrap_or_default())
}
