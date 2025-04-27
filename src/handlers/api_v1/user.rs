use std::str::FromStr;

use crate::models::user::{Claims, Encrypt, Role, User};
use crate::{
    handlers::{
        State,
        utils::{from_incoming_to, get_extention},
    },
    models::user::UpdateUser,
};

use super::data_entry::NewUser;
use super::{Incoming, Request, ResponseError, ResponseWithError, StatusCode};
use bytes::Bytes;
use http::{Method, Response, header};
use http_body_util::Full;
use mongodb::bson::doc;
use mongodb::bson::oid::ObjectId;
use serde_json::json;

pub async fn user(req: Request<Incoming>) -> ResponseWithError {
    let method = req.method();

    match *method {
        Method::POST => insert(req).await,
        Method::PATCH => update(req).await,
        Method::DELETE => delete(req).await,
        Method::GET => get(req).await,
        _ => Err(ResponseError::new::<&str>(StatusCode::BAD_REQUEST, None)),
    }
}

pub async fn insert(req: Request<Incoming>) -> ResponseWithError {
    let (parts, body) = req.into_parts();
    let claims = get_extention::<Claims>(&parts.extensions).await.unwrap();

    if claims.role != Role::Admin {
        return Err(ResponseError::new(
            StatusCode::UNAUTHORIZED,
            Some("The user is not an admin"),
        ));
    }

    let mut user = from_incoming_to::<NewUser>(body).await?;
    user.password = user.password.encrypt()?;

    let state = get_extention::<State>(&parts.extensions).await?;

    let resp = state.insert::<User>(user.into()).await?;

    Ok(Response::builder()
        .status(StatusCode::CREATED)
        .body(Full::new(Bytes::from(
            serde_json::json!({"new_element": resp}).to_string(),
        )))
        .unwrap_or_default())
}

pub async fn update(req: Request<Incoming>) -> ResponseWithError {
    let (parts, body) = req.into_parts();
    let state = get_extention::<State>(&parts.extensions).await?; //todo create UpdateUser
    let new_user = from_incoming_to::<UpdateUser>(body).await?;

    let claims = get_extention::<Claims>(&parts.extensions).await.unwrap();

    if claims.role == Role::Admin {
        state.update::<User>(new_user.try_into()?, doc! {}).await?;
        Ok(Response::builder()
            .status(StatusCode::OK)
            .body(Full::new(Bytes::new()))
            .unwrap_or_default())
    } else {
        let _id = ObjectId::parse_str(claims.sub.as_str()).unwrap();

        if new_user.username.is_some() {
            Err(ResponseError::new(
                StatusCode::UNAUTHORIZED,
                Some("You do not have permission to update the username"),
            ))
        } else {
            state.update::<User>(new_user.try_into()?, doc! {}).await?;
            Ok(Response::builder()
                .status(StatusCode::OK)
                .body(Full::new(Bytes::new()))
                .unwrap_or_default())
        }
    }
}

pub async fn delete(req: Request<Incoming>) -> ResponseWithError {
    let claims = get_extention::<Claims>(req.extensions()).await?;

    if claims.role != Role::Admin {
        return Err(ResponseError::new(
            StatusCode::UNAUTHORIZED,
            Some("You do not have permission to delete any element"),
        ));
    }

    let state = get_extention::<State>(req.extensions()).await?;
    let len = state.delete::<User>(doc! {}).await?;

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

pub async fn get(req: Request<Incoming>) -> ResponseWithError {
    let state = get_extention::<State>(req.extensions()).await?;
    let claims = get_extention::<Claims>(req.extensions()).await.unwrap();

    let filter = if claims.role == Role::Admin {
        doc! {}
    } else {
        doc! {"_id": ObjectId::from_str(claims.sub.as_str()).unwrap()}
    };

    let user = state.get_one::<User>(filter).await?;
    Ok(Response::builder()
        .header(header::CONTENT_TYPE, "application/json")
        .status(StatusCode::OK)
        .body(Full::new(Bytes::from(
            json!({
                "data": user,
                "length": 1,
            })
            .to_string(),
        )))
        .unwrap_or_default())
}
