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
    stream_upload::{StreamUpload, Upload},
};
use futures::StreamExt;
use http::Method;
use http_body_util::BodyStream;
use mime::Mime;
use multer::Multipart;
use time::{OffsetDateTime, format_description};
use tokio::fs::File;

static PATH_PROGRAMS: OnceLock<PathBuf> = OnceLock::new();
static PATH_ICONS: OnceLock<PathBuf> = OnceLock::new();
const BUFFER_WRITER: usize = 1024 * 64;

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

    let mut multipart = Multipart::new(stream, boundary);
    // let mut oids = Vec::new();
    let mut path = get_dir_programs();

    let tmp = StreamUpload::new(multipart, Vec::new());
    let mut tmp = Upload::new(path, tmp);

    tmp.next().await;

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

async fn process_mime(
    mime: Option<&Mime>,
    channel: &str,
    program_tv: &str,
    file_name: &str,
) -> Result<(PathBuf, String), ResponseError> {
    match mime.map(|x| x.type_()) {
        Some(mime::VIDEO) => {
            let mut path = get_dir_programs();
            path.push(channel);
            path.push(program_tv);

            if !path.exists() {
                if let Err(e) = tokio::fs::create_dir_all(&path).await {
                    tracing::error!("Error to create dirs - Err: {}", e);
                    return Err(ResponseError::new(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Some("there is no location to store the file"),
                    ));
                }
            }

            let time = OffsetDateTime::now_local().map_err(|x| {
                ResponseError::new(StatusCode::INTERNAL_SERVER_ERROR, Some(x.to_string()))
            })?;

            let time = time
                .format(
                    &format_description::parse("[year]-[month]-[day]_[hour]-[minute]").map_err(
                        |_| {
                            ResponseError::new(
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Some("failt to create the format description"),
                            )
                        },
                    )?,
                )
                .map_err(|_| {
                    ResponseError::new(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Some("Failed to formatting the time"),
                    )
                })?;

            let file_name = format!("{time}__{file_name}");
            path.push(&file_name);

            Ok((path, file_name))
        }
        Some(mime::IMAGE) => {
            let mut path = get_dir_icons();
            let file_name = format!(
                "{}_{}.{}",
                channel,
                program_tv,
                file_name.split('.').last().ok_or(ResponseError::new(
                    StatusCode::BAD_REQUEST,
                    Some("File have not extension")
                ))?
            );
            path.push(&file_name);

            Ok((path, file_name))
        }
        mime => {
            tracing::error!("The mime {{ {:?} }} is not permit ", mime);
            Err(ResponseError::new(
                StatusCode::BAD_REQUEST,
                Some("Thie file's type is not permit"),
            ))
        }
    }
}
