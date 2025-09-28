pub(super) mod data_entry;
pub(super) mod file;
use http::{Method, Request, StatusCode, header};
use hyper::body::Incoming;

use crate::{handlers::cors, models::user::Claim};

use super::{
    ResultResponse,
    error::{ParseError, ResponseError},
};

pub async fn upload(req: Request<Incoming>) -> ResultResponse {
    let mut path = req.uri().path().split("api/v1/file/").nth(1).map(|x| x.split('/').map(ToString::to_string).collect::<Vec<String>>()).unwrap_or_default();
    
    if path.len() != 2 {
        return Err(ResponseError::new::<&str>(StatusCode::BAD_REQUEST, None).into());
    }

    let programa = path.pop();
    let ch = path.pop();

    let user = req.extensions().get::<Claim>();

    if req.method() == Method::OPTIONS {
        Ok(cors())
    } else if req.method() == Method::POST && let Some(programa) = programa && let Some(ch) = ch {
        file::upload_video(req, ch, programa).await
    } else {
        Err(ResponseError::new(
            StatusCode::NOT_FOUND,
            Some(format!("Entpoint {} not found", req.uri())),
        ))
    }
}
