use super::{
    Incoming, ParseError, Request, ResponseError, ResultResponse, StatusCode, doc, header,
};
use crate::{
    handlers::{
        State,
        utils::{get_extention, get_user_oid},
    },
    models::{
        logs::{Logs, Operation, Owner, ResultOperation, upload::UploadLog},
        user::{Claim, Role, User},
    },
    stream_upload::{
        Upload, UploadResult,
        stream::{MimeAllowed, StreamUpload},
    },
};
use bytes::Bytes;
use futures::StreamExt;
use http::{HeaderMap, Method, Response};
use http_body_util::{BodyStream, Full};
use multer::Multipart;
use std::{path::PathBuf, sync::OnceLock};
use time::OffsetDateTime;
use utils::Peer;

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
        let claims = get_extention::<Claim>(req.extensions())?;
        let repository = get_extention::<State>(req.extensions())?;

        if !matches!(user.ch.as_ref(), Some(ch) if ch.name == channel && ch.program.iter().any(|x| x.name == program_tv))
            && claims.role != Role::Admin
        {
            return Err(ResponseError::new(
                StatusCode::BAD_REQUEST,
                Some("Channel name or program name is not belong to the user"),
            ));
        }

        upload(req, channel, program_tv, user).await
    } else {
        Err(ResponseError::new(
            StatusCode::NOT_IMPLEMENTED,
            Some("Method not implemented"),
        ))
    }
}

pub async fn upload(
    req: Request<Incoming>,
    channel: String,
    program_tv: String,
    user: User,
) -> ResultResponse {
    let (parts, body) = req.into_parts();
    let repository = get_extention::<State>(&parts.extensions)?;
    let ip_src = get_extention::<Peer>(&parts.extensions)?;
    let stream = BodyStream::new(body)
        .filter_map(|x| async move { x.map(|x| x.into_data().ok()).transpose() });

    let boundary = get_boundary(&parts.headers)?;
    let boundary = multer::parse_boundary(boundary)
        .map_err(|e| ResponseError::new(StatusCode::BAD_REQUEST, Some(e.to_string())))?;

    let stream = StreamUpload::new(
        Multipart::new(stream, boundary),
        vec![
            MimeAllowed::MediaType(mime::VIDEO),
            MimeAllowed::MediaType(mime::IMAGE),
        ],
    );
    let mut stream = Upload::new(stream, |x| {
        if mime::VIDEO.eq(&x.type_()) {
            get_dir_programs()
        } else {
            get_dir_icons()
        }
    });

    loop {
        let (upload_result, operation_result) = match stream.next().await {
            Some(Ok(log)) => (log, ResultOperation::Success),
            Some(Err(err)) => (
                UploadResult {
                    size: 0,
                    elapsed: 0,
                    file_name: "".to_string(),
                },
                ResultOperation::Fail(err.to_string()),
            ),
            None => {
                break Ok(Response::builder()
                    .status(StatusCode::CREATED)
                    .body(Full::new(Bytes::new()))
                    .unwrap_or_default());
            }
        };

        let new_log = Logs {
            id: None,
            owner: Owner {
                username: user.username.clone(),
                src: ip_src.get_ip_or_unknown(),
                role: user.role,
            },
            at: OffsetDateTime::now_local().unwrap(),
            operation: Operation::Upload {
                detail: UploadLog {
                    file_name: upload_result.file_name,
                    channel: channel.clone(),
                    program_tv: program_tv.clone(),
                    elapsed_upload: Some(upload_result.elapsed),
                    size: upload_result.size,
                },
                result: operation_result,
            },
        };
        repository.insert(new_log).await?;
    }
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

fn get_boundary(headers: &HeaderMap) -> Result<&str, ResponseError> {
    headers
        .get(header::CONTENT_TYPE)
        .ok_or(ResponseError::new(
            StatusCode::BAD_REQUEST,
            Some("Content-Type not found"),
        ))
        .and_then(|x| {
            x.to_str().map_err(|_| {
                ResponseError::new(StatusCode::BAD_REQUEST, Some("Boundary parse error"))
            })
        })
}
