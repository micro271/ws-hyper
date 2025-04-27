use http::Extensions;

use super::{BodyExt, DeserializeOwned, Incoming, ResponseError, StatusCode};

pub async fn from_incoming_to<T>(body: Incoming) -> Result<T, ResponseError>
where
    T: DeserializeOwned,
{
    match body.collect().await {
        Ok(e) => match serde_json::from_slice::<'_, T>(&e.to_bytes()) {
            Ok(e) => Ok(e),
            _ => Err(ResponseError::new(
                StatusCode::BAD_REQUEST,
                Some("Parsing data entry error"),
            )),
        },
        Err(e) => {
            tracing::error!("Error to deserialize the body - {e}");
            Err(ResponseError::new(
                StatusCode::BAD_REQUEST,
                Some("Data entry error"),
            ))
        }
    }
}

pub async fn get_extention<T>(ext: &Extensions) -> Result<&T, ResponseError>
where
    T: Sync + Send + 'static,
{
    match ext.get::<T>() {
        Some(ext) => Ok(ext),
        None => {
            tracing::error!("State is not present in extensios");
            Err(ResponseError::new::<&str>(
                StatusCode::INTERNAL_SERVER_ERROR,
                None,
            ))
        }
    }
}
