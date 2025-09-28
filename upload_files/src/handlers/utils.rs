use http::Extensions;

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
