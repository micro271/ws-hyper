pub(super) mod data_entry;
pub(super) mod file;
use http::{Method, Request, StatusCode, header};
use hyper::body::Incoming;

use crate::{handlers::{cors, GrpcCli}, models::user::Claim};

use super::{
    ResultResponse,
    error::{ParseError, ResponseError},
};

pub async fn upload(req: Request<Incoming>) -> ResultResponse {
    let mut path = req
        .uri()
        .path()
        .split("api/v1/file/")
        .nth(1)
        .map(|x| {
            x.split('/')
                .map(ToString::to_string)
                .collect::<Vec<String>>()
        })
        .unwrap_or_default();

    if path.len() != 2 {
        return Err(ResponseError::parse_error(ParseError::Path));
    }

    let programa = path.pop();
    let ch = path.pop();

    let _user = req.extensions().get::<Claim>();

    if req.method() == Method::OPTIONS {
        Ok(cors())
    } else if req.method() == Method::POST
        && let Some(programa) = programa
        && let Some(ch) = ch
    {
        let id = req.extensions().get::<Claim>().unwrap().sub;
        let info = req.extensions().get::<GrpcCli>().unwrap();

        let user = info.user_info(id).await.unwrap();
        
        if !user.resources.split(',').collect::<Vec<&str>>().chunks(2).filter(|x| x.len() == 2).any(|x| x[0] == ch && x[1] == programa) {
            Err(ResponseError::new(StatusCode::UNAUTHORIZED, Some("You don't have authority above this resource")))
        } else {
            file::upload_video(req, ch, programa, user.username, user.role.try_into().unwrap()).await
        }

    } else {
        Err(ResponseError::new(
            StatusCode::NOT_FOUND,
            Some(format!("Entpoint {} not found", req.uri())),
        ))
    }
}
