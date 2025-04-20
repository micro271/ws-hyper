use std::sync::Arc;

use crate::{
    models::{
        file::Files,
        user::{Claims, UserEntry},
    },
    repository::Repository,
};
use bcrypt::verify;
use bytes::Bytes;
use futures::StreamExt;
use http::{HeaderMap, Method, Request, Response, StatusCode, header};
use http_body_util::{BodyExt, BodyStream, Full};
use hyper::body::Incoming;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};
use multer::Multipart;
use time::UtcOffset;
use tokio::{fs::File, io::AsyncWriteExt};
use uuid::Uuid;

use super::error::ResponseError;

type Res = Result<Response<Full<Bytes>>, ResponseError>;

const JWT_IDENTIFIED: &str = "JWT";

pub async fn api(req: Request<Incoming>, repository: Arc<Repository>) -> Res {
    let path = req.uri().path().split("/api/v1").nth(1).unwrap_or_default();

    if path.starts_with("/upload") && req.method() == Method::POST {
        let path = path
            .split("/upload/")
            .nth(1)
            .map(|x| x.split('/').collect::<Vec<&str>>());

        match path {
            Some(mut e) if e.len() == 2 => {
                let parse_error = ResponseError::new(
                    StatusCode::BAD_GATEWAY,
                    format!("Endpoint {} invalid", req.uri()),
                );
                let id_user = e.remove(0).parse().map_err(|_| parse_error.clone())?;
                let id_tvshow = e.remove(0).parse().map_err(|_| parse_error)?;

                return upload(req, id_user, id_tvshow).await;
            }
            _ => {}
        }
    }

    Err(ResponseError::new(
        StatusCode::NOT_FOUND,
        format!("Entpoint {} not found", req.uri()),
    ))
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
                        id: Uuid::new_v4(),
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

pub async fn login(req: Request<Incoming>, repository: Arc<Repository>) -> Res {
    let body = req.into_body();
    let check_user = body
        .collect()
        .await
        .map(|x| serde_json::from_slice::<'_, UserEntry>(&x.to_bytes()));

    match check_user {
        Ok(Ok(e)) => {
            if verify(e.password, "prueba").unwrap_or(false) {
                tracing::info!("Login succesful: [username: {}]", e.username);
                Ok(Response::builder()
                    .status(StatusCode::OK)
                    .header(header::SET_COOKIE, "algo")
                    .header(header::LOCATION, "/")
                    .body(Full::new(Bytes::new()))
                    .unwrap_or_default())
            } else {
                tracing::error!("Login failure: [username: {}]", e.username);
                Err(ResponseError {
                    status: StatusCode::UNAUTHORIZED,
                    detail: "Username or password error".to_string(),
                })
            }
        }
        Ok(Err(e)) => {
            tracing::error!("Bcrypt Err: {e}");
            Err(ResponseError {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                detail: e.to_string(),
            })
        }
        Err(e) => {
            tracing::info!("UserEntry is not present - Error: {}", e);
            Err(ResponseError {
                status: StatusCode::BAD_REQUEST,
                detail: "User's values is not present".to_string(),
            })
        }
    }
}

pub async fn verifi_token_from_cookie(headers: &HeaderMap) -> Result<Claims, ResponseError> {
    headers
        .get(http::header::COOKIE)
        .and_then(|x| x.to_str().ok())
        .and_then(|x| {
            x.split(';')
                .find(|x| x.starts_with(JWT_IDENTIFIED))
                .and_then(|x| x.split('=').nth(1))
        })
        .and_then(|x| {
            decode::<Claims>(
                x,
                &DecodingKey::from_secret("SECRET".as_ref()),
                &Validation::new(Algorithm::ES256),
            )
            .ok()
            .map(|x| x.claims)
        })
        .ok_or(ResponseError {
            status: StatusCode::UNAUTHORIZED,
            detail: "Token is not present".to_string(),
        })
}
