use http::Extensions;
use mongodb::bson::oid::ObjectId;

use crate::models::user::Claim;

use super::{ResponseError, StatusCode};

pub fn get_extention<T>(ext: &Extensions) -> Result<&T, ResponseError>
where
    T: Sync + Send + 'static,
{
    if let Some(ext) = ext.get::<T>() {
        Ok(ext)
    } else {
        tracing::error!("State is not present in extensios");
        Err(ResponseError::new::<&str>(
            StatusCode::INTERNAL_SERVER_ERROR,
            None,
        ))
    }
}

pub fn get_user_oid(claims: &Claim) -> Result<ObjectId, ResponseError> {
    match claims.sub.parse::<ObjectId>() {
        Ok(oid) => Ok(oid),
        Err(e) => {
            tracing::error!("Error to parsing from string to objectid - Err: {e}");
            Err(ResponseError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                Some("we cannot obtain your username"),
            ))
        }
    }
}
