use super::{
    Bytes, Full, Incoming, ParseError, Request, Response, ResponseError, ResultResponse,
    StatusCode, doc, header,
};
use crate::{
    handlers::{
        State,
        utils::{get_extention, get_user_oid},
    },
    models::{
        file::{FileLog, Owner},
        user::{Ch, Claims, User},
    },
    peer::Peer,
};
use futures::StreamExt;
use http::Method;
use http_body_util::BodyStream;
use multer::Multipart;
use serde_json::json;
use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};
use time::UtcOffset;
use tokio::{fs::File, io::AsyncWriteExt};

static PATH_DIR: OnceLock<PathBuf> = OnceLock::new();

fn get_path_dir() -> PathBuf {
    PATH_DIR
        .get_or_init(|| {
            let tmp = std::env::var("DIRECTORY").expect("the program's directory is not defined");
            Path::new(&tmp)
                .canonicalize()
                .unwrap_or_else(|_| panic!("{tmp} Directory not exists"))
        })
        .clone()
}

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
            "tv programs' id not present".into(),
        ))
        .and_then(|x| x.parse().map_err(|_| parse_error.clone()))?;

    let channel: String = path
        .pop()
        .ok_or(ResponseError::new(
            StatusCode::BAD_REQUEST,
            "id's useris not present".into(),
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
        let oid = get_user_oid(claims)?;
        let user = repository.get_one::<User>(doc! {"_id": oid}).await?;
        if !matches!(user.ch, Some(ch) if ch.name == channel && ch.program.iter().any(|x| x.name == program_tv))
        {
            return Err(ResponseError::new(
                StatusCode::BAD_REQUEST,
                Some("Channel name or program name is not belong to the user"),
            ));
        }
        upload(req, boundary, channel, program_tv, user.username).await
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
    username: String,
) -> ResultResponse {
    let (parts, body) = req.into_parts();
    let repository = get_extention::<State>(&parts.extensions)?;
    let ip_src = get_extention::<Peer>(&parts.extensions)?;
    let stream = BodyStream::new(body)
        .filter_map(|x| async move { x.map(|x| x.into_data().ok()).transpose() });

    let mut multipart = Multipart::new(stream, boundary);
    let mut oids = Vec::new();
    loop {
        match multipart.next_field().await {
            Ok(Some(mut field)) => {
                tracing::debug!("field.name: {:?}", field.name().unwrap_or_default());

                if field
                    .content_type()
                    .is_none_or(|x| x.type_() != mime::VIDEO)
                {
                    return Err(ResponseError::new(
                        StatusCode::BAD_REQUEST,
                        Some("The file is not a video"),
                    ));
                }
                let file_name = field
                    .file_name()
                    .ok_or(ResponseError::new(
                        StatusCode::BAD_REQUEST,
                        Some("The file have not an name"),
                    ))?
                    .to_string();

                tracing::debug!("file name: {file_name:?}");

                let time = time::OffsetDateTime::now_utc()
                    .to_offset(UtcOffset::from_hms(-3, 0, 0).unwrap());

                // we've appended the file name to the directory that contains all the all programs and channels

                let mut file = File::create(&file_name).await.unwrap();
                let mut size: usize = 0;

                let duration = std::time::Instant::now();

                loop {
                    match field.chunk().await {
                        Ok(Some(e)) => {
                            size += e.len();
                            if let Err(e) = file.write_all(&e).await {
                                tracing::error!("Error to write from bytes to file {e}");
                                return Err(ResponseError::new(
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    "File write".into(),
                                ));
                            }
                        }
                        Err(e) => {
                            tracing::error!("Read chunk error - Error: {e}");
                            return Err(ResponseError::new(
                                StatusCode::INTERNAL_SERVER_ERROR,
                                "Read bytes fail".into(),
                            ));
                        }
                        Ok(None) => break,
                    }
                }

                let duration =
                    Some(usize::try_from(duration.elapsed().as_secs()).unwrap_or_default());

                let new = FileLog {
                    _id: None,
                    create_at: time,
                    elapsed_upload: duration,
                    file_name: file_name.clone(),
                    channel: channel.clone(),
                    program_tv: program_tv.clone(),
                    owner: Owner {
                        username: username.clone(),
                        ip_src: ip_src.get_ip_or_unknown(),
                    },
                    size,
                };

                tracing::debug!("FileLog: {:#?}", new);

                let oid = repository.insert(new).await.unwrap();
                oids.push(json!({"_oid": oid}));
            }
            Err(e) => {
                tracing::error!("Read field of the multiart error: {e}");
                break Err(ResponseError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Error to read field in multipart".into(),
                ));
            }
            Ok(None) => {
                break Ok(Response::builder()
                    .status(StatusCode::CREATED)
                    .body(Full::new(Bytes::from(
                        json!({"added_in": program_tv}).to_string(),
                    )))
                    .unwrap_or_default());
            }
        }
    }
}
