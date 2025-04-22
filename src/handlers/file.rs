use crate::{models::file::Files, repository::Repository};
use bytes::Bytes;
use futures::StreamExt;
use http::{Method, Request, Response, StatusCode, header};
use http_body_util::{BodyStream, Full};
use hyper::body::Incoming;
use mongodb::bson::oid::ObjectId;
use multer::Multipart;
use std::{
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
};
use time::UtcOffset;
use tokio::{fs::File, io::AsyncWriteExt};
use uuid::Uuid;

use super::{
    ResponseWithError,
    error::{ParseError, ResponseError},
};

static PATH_DIR: OnceLock<PathBuf> = OnceLock::new();

fn get_path_dir<'a>() -> PathBuf {
    PATH_DIR
        .get_or_init(|| {
            let tmp = std::env::var("DIRECTORY").expect("the program's directory is not defined");
            Path::new(&tmp)
                .canonicalize()
                .expect(&format!("{tmp} Directory not exists"))
        })
        .clone()
}

pub async fn file(req: Request<Incoming>, repository: Arc<Repository>) -> ResponseWithError {
    let mut path = req
        .uri()
        .path()
        .split("/file")
        .nth(1)
        .map(|x| x.split('/').collect::<Vec<_>>())
        .unwrap();
    let len = path.len();
    let method = req.method();
    let parse_error = ResponseError::parse_error(ParseError::Path);

    let id_tvshow = path
        .pop()
        .ok_or(ResponseError::new(
            StatusCode::BAD_REQUEST,
            "tv programs' id not present".to_string(),
        ))
        .and_then(|x| x.parse().map_err(|_| parse_error.clone()));

    let id_user = path
        .pop()
        .ok_or(ResponseError::new(
            StatusCode::BAD_REQUEST,
            "id's useris not present".to_string(),
        ))
        .and_then(|x| x.parse().map_err(|_| parse_error));

    if len == 2 && method == Method::POST {
        return upload(req, id_user?, id_tvshow?).await;
    } else if path.len() == 3 {
        let id_file = path
            .pop()
            .ok_or(ResponseError::new(
                StatusCode::BAD_REQUEST,
                "File is not present in path".to_string(),
            ))
            .map(|x| {
                x.parse()
                    .map_err(|_| ResponseError::parse_error(ParseError::Path))
            })?;

        match (method, id_tvshow, id_user) {
            (&Method::DELETE, Ok(id_tvshow), Ok(id_user)) => {
                return delete_file(req, repository, id_user, id_tvshow, id_file?).await;
            }
            (&Method::PATCH, Ok(id_tvshow), Ok(id_user)) => {
                return update_file(req, repository, id_tvshow, id_user, id_file?).await;
            }
            (&Method::GET, id_tvshow, id_user) => {
                let id_tvshow = id_tvshow.ok();
                let id_user = id_user.ok();
                return get_files(req, repository, id_user, id_tvshow).await;
            }
            _ => {}
        }
    }

    Err(ResponseError::unimplemented())
}

pub async fn upload(
    mut req: Request<Incoming>,
    id_user: String,
    id_tvshow: String,
) -> Result<Response<Full<Bytes>>, ResponseError> {
    if let Some(e) = req.headers().get(header::CONTENT_TYPE).cloned() {
        let boundary = multer::parse_boundary(e.to_str().unwrap()).map_err(|e| {
            tracing::error!("{}", e.to_string());
            ResponseError::new(StatusCode::BAD_REQUEST, "Parse Error".to_string())
        })?;

        let aux = BodyStream::new(req.body_mut())
            .filter_map(|x| async move { x.map(|x| x.into_data().ok()).transpose() });
        let mut multipart = Multipart::new(aux, boundary);

        loop {
            match multipart.next_field().await {
                Ok(Some(mut field)) => {
                    let tmp = field.name().unwrap();
                    tracing::debug!("field.name: {tmp:?}");

                    let mut tmp = field
                        .file_name()
                        .map(|x| x.split('.').collect::<Vec<&str>>())
                        .filter(|x| x.len() >= 2)
                        .ok_or(ResponseError::new(
                            StatusCode::BAD_REQUEST,
                            "File name error, we have't identified the stem and extension"
                                .to_string(),
                        ))?;

                    let extension = tmp.pop().unwrap().to_string();

                    let stem = if tmp.len() > 1 {
                        tmp.join(".")
                    } else {
                        tmp.pop().unwrap().to_string()
                    };

                    let file_name = field.file_name().unwrap();

                    tracing::debug!("file name: {file_name:?}");

                    if let Some(e) = field.content_type() {
                        tracing::debug!("{e:?}");
                    }

                    let time = time::OffsetDateTime::now_utc()
                        .to_offset(UtcOffset::from_hms(-3, 0, 0).unwrap());
                    let mut file = File::create(file_name).await.unwrap();
                    let mut size: i64 = 0;

                    let duration = std::time::Instant::now();

                    loop {
                        match field.chunk().await {
                            Ok(Some(e)) => {
                                size += match i64::try_from(e.len()) {
                                    Ok(e) => e,
                                    Err(err) => {
                                        tracing::error!(
                                            "Error parsing from bytes' length to i64 - Erro: {}",
                                            err.to_string()
                                        );
                                        Default::default()
                                    }
                                };
                                if let Err(e) = file.write_all(&e).await {
                                    tracing::error!("Error to write from bytes to file {e}");
                                    return Err(ResponseError::new(
                                        StatusCode::INTERNAL_SERVER_ERROR,
                                        "File write".to_string(),
                                    ));
                                }
                            }
                            Err(e) => {
                                tracing::error!("Read chunk error - Error: {e}");
                                return Err(ResponseError::new(
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    "Read bytes fail".to_string(),
                                ));
                            }
                            Ok(None) => break,
                        }
                    }

                    tracing::warn!("File Size: {}", size);

                    let duration =
                        Some(usize::try_from(duration.elapsed().as_secs()).unwrap_or_default());

                    let new = Files {
                        _id: ObjectId::new(),
                        create_at: time,
                        elapsed_upload: duration,
                        extension,
                        id_tvshow: Uuid::new_v4(),
                        stem,
                        size,
                    };
                }
                Err(e) => {
                    tracing::error!("Read field of the multiart error: {e}");
                    break Err(ResponseError::new(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Error to read field in multipart".to_string(),
                    ));
                }
                Ok(None) => {
                    break Err(ResponseError::new(
                        StatusCode::OK,
                        "Anithing field read".to_string(),
                    ));
                }
            }
        }
    } else {
        Err(ResponseError::new(
            StatusCode::BAD_REQUEST,
            "Content-type not present".to_string(),
        ))
    }
}

pub async fn delete_file(
    req: Request<Incoming>,
    repository: Arc<Repository>,
    id_user: String,
    id_tvshow: String,
    id_file: String,
) -> ResponseWithError {
    let mut path = get_path_dir(); /* TODO: The real path shound be in the repository */

    path.push(format!(
        "/{}/{}/{}",
        id_user, id_tvshow, id_file /* TODO: we've obtained the name from repository */
    ));

    if path.is_file() {
        let tmp = path.metadata().unwrap();
        if tmp.permissions().readonly() {
            return Err(ResponseError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Read only".to_string(),
            ));
        } else {
            match tokio::fs::remove_file(path).await {
                Ok(()) => {
                    return Ok(Response::builder()
                        .status(StatusCode::OK)
                        .body(Full::new(Bytes::new()))
                        .unwrap_or_default());
                }
                Err(e) => {
                    tracing::warn!("Error to delete the file - Error: {}", e);
                    return Err(ResponseError::new(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Fail to delete the file".to_string(),
                    ));
                }
            }
        }
    }

    Err(ResponseError {
        status: StatusCode::BAD_REQUEST,
        detail: "".to_string(),
    })
}

pub async fn update_file(
    req: Request<Incoming>,
    repository: Arc<Repository>,
    id_tvshow: String,
    id_user: String,
    id_file: String,
) -> ResponseWithError {
    Err(ResponseError::unimplemented())
}

pub async fn get_files(
    req: Request<Incoming>,
    repository: Arc<Repository>,
    id_user: Option<String>,
    id_tvshow: Option<String>,
) -> ResponseWithError {
    Err(ResponseError::unimplemented())
}
