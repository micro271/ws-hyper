use std::{path::PathBuf, sync::OnceLock};

use super::{
    Incoming, ParseError, Request, ResponseError, ResultResponse, StatusCode, doc, header,
};
use crate::{
    handlers::{
        State,
        utils::{get_extention, get_user_oid},
    },
    models::user::{Claims, Role, User},
    peer::Peer,
    stream_upload::{MimeAllowed, StreamUpload, Upload},
};
use futures::StreamExt;
use http::Method;
use http_body_util::BodyStream;
use mime::Mime;
use multer::Multipart;
use time::{OffsetDateTime, format_description};

static PATH_PROGRAMS: OnceLock<PathBuf> = OnceLock::new();
static PATH_ICONS: OnceLock<PathBuf> = OnceLock::new();

pub async fn file(req: Request<Incoming>) -> ResultResponse {
    let mut path = req
        .uri()
        .path()
        .split("/file/")
        .nth(1)
        .map(|x| x.split('/').collect::<Vec<_>>())
        .unwrap();

    let parse_error = ResponseError::parse_error(ParseError::Path);

    let program_tv: String = path
        .pop()
        .ok_or(ResponseError::new(
            StatusCode::BAD_REQUEST,
            "Program tv is not present".into(),
        ))
        .and_then(|x| x.parse().map_err(|_| parse_error.clone()))?;

    let channel: String = path
        .pop()
        .ok_or(ResponseError::new(
            StatusCode::BAD_REQUEST,
            "Channel is not present".into(),
        ))
        .and_then(|x| x.parse().map_err(|_| parse_error))?;

    if req.method() == Method::POST {
        let boundary = multer::parse_boundary(
            req.headers()
                .get(header::CONTENT_TYPE)
                .ok_or_else(|| {
                    tracing::error!("content-type is not present");
                    ResponseError::new(
                        StatusCode::BAD_REQUEST,
                        Some("Header content-type is not present in the request"),
                    )
                })?
                .to_str()
                .map_err(|x| {
                    tracing::error!("Prasing from HeaderValor to str fail - err {x}");
                    ResponseError::new(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Some("Get content-type value failed"),
                    )
                })?,
        )
        .map_err(|e| {
            tracing::error!("{}", e.to_string());
            ResponseError::new(StatusCode::BAD_REQUEST, "Boundary is not present".into())
        })?;

        let claims = get_extention::<Claims>(req.extensions())?;
        let repository = get_extention::<State>(req.extensions())?;
        let user = repository
            .get_one::<User>(doc! {"_id": get_user_oid(claims)?})
            .await?;

        if !matches!(user.ch.as_ref(), Some(ch) if ch.name == channel && ch.program.iter().any(|x| x.name == program_tv))
            && claims.role != Role::Admin
        {
            return Err(ResponseError::new(
                StatusCode::BAD_REQUEST,
                Some("Channel name or program name is not belong to the user"),
            ));
        }

        upload(req, boundary, channel, program_tv, user).await
    } else {
        Err(ResponseError::new(
            StatusCode::NOT_IMPLEMENTED,
            Some("Method not implemented"),
        ))
    }
}

pub async fn upload(
    req: Request<Incoming>,
    boundary: String,
    channel: String,
    program_tv: String,
    user: User,
) -> ResultResponse {
    let (parts, body) = req.into_parts();
    let repository = get_extention::<State>(&parts.extensions)?;
    let ip_src = get_extention::<Peer>(&parts.extensions)?;
    let stream = BodyStream::new(body)
        .filter_map(|x| async move { x.map(|x| x.into_data().ok()).transpose() });

    let tmp = StreamUpload::new(
        Multipart::new(stream, boundary),
        vec![MimeAllowed::MediaType(mime::VIDEO)],
    );
    let mut tmp = Upload::new(get_dir_programs(), tmp);

    while let Some(e) = tmp.next().await {
        tracing::warn!("{e:?}");
    }

    Err(ResponseError::unimplemented())
}

pub fn get_dir_programs() -> PathBuf {
    PATH_PROGRAMS
        .get_or_init(|| {
            std::env::var("DIR_PROGRAMS")
                .unwrap_or(".".to_string())
                .into()
        })
        .clone()
}

pub fn get_dir_icons() -> PathBuf {
    PATH_ICONS
        .get_or_init(|| std::env::var("DIR_ICONS").unwrap_or(".".to_string()).into())
        .clone()
}
