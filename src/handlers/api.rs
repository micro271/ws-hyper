use std::sync::Arc;

use bytes::Bytes;
use futures::StreamExt;
use http::{header, Method, Request, Response, StatusCode};
use http_body_util::{BodyStream, Full};
use hyper::body::Incoming;
use multer::Multipart;
use time::UtcOffset;
use tokio::{fs::File, io::AsyncWriteExt};
use uuid::Uuid;
use crate::{models::file::Files, repository::Repository};

use super::error::ResponseError;

pub async fn api(req: Request<Incoming>, repository: Arc<Repository>) -> Result<Response<Full<Bytes>>, ResponseError> {
    let path = req.uri().path().split("/api/v1").nth(1).unwrap_or_default();

    if path.starts_with("/upload") && req.method() == Method::POST {
        let path = path.split("/upload/").nth(1).map(|x|x.split("/").collect::<Vec<&str>>());
        
        match path {
            Some(mut e) if e.len() == 2 => {
                let parse_error = ResponseError::new(StatusCode::BAD_GATEWAY, format!("Endpoint {} invalid", req.uri().to_string()));
                let id_user = e.remove(0).parse().map_err(|_|parse_error.clone())?;
                let id_tvshow = e.remove(0).parse().map_err(|_|parse_error)?;

                return upload(req, id_user, id_tvshow).await;
            },
            _ => {}
        }
    } 
    
    Err(ResponseError::new(StatusCode::NOT_FOUND, format!("Entpoint {} not found", req.uri().to_string())))
}

pub async fn upload(mut req: Request<Incoming>, id_user: String, id_tvshow: String) -> Result<Response<Full<Bytes>>, ResponseError> {

    if let Some(e) = req.headers().get(header::CONTENT_TYPE).cloned() {
        let boundary = multer::parse_boundary(e.to_str().unwrap())
            .map_err(|e| {
                tracing::error!("{}", e.to_string());
                ResponseError::new(StatusCode::BAD_REQUEST, "Parse Error".to_string())
            })?;

        let aux = BodyStream::new(req.body_mut()).filter_map(|x| async move { x.map(|x| x.into_data().ok()).transpose()});
        let mut multipart = Multipart::new(aux, boundary);
        let mut time;
        let mut duration;

        while let Ok(Some(mut field)) = multipart.next_field().await {

            let tmp = field.name().unwrap();
            println!("field.name: {:?}",tmp);

            let mut tmp = field.file_name().map(|x|x.split(".")
                .collect::<Vec<&str>>())
                .filter(|x| x.len() >= 2)
                .ok_or(ResponseError::new(StatusCode::BAD_REQUEST, "File name error, we have't identified the stem and extension".to_string()))?;

            let extension = tmp.pop().unwrap().to_string();

            let stem = if tmp.len() > 1 {
                tmp.join(".")
            } else {
                tmp.pop().unwrap().to_string()
            };

            let file_name = field.file_name().unwrap();

            println!("file name: {:?}",file_name);

            if let Some(e) = field.content_type() {
                println!("{:?}",e);
            }

            time = time::OffsetDateTime::now_utc().to_offset(UtcOffset::from_hms(-3, 0, 0).unwrap());
            let mut file = File::create(file_name).await.unwrap();

            let elapsed = std::time::Instant::now();

            let mut size: usize = 0;

            while let Ok(Some(e)) = field.chunk().await {
                size += e.len();
                file.write_all(&e).await.unwrap();
            }

            tracing::warn!("File Size: {}", size);

            duration = Some(usize::try_from(elapsed.elapsed().as_secs()).unwrap_or_default());

            let new = Files {
                id: Uuid::new_v4(),
                create_at: time,
                elapsed_upload: duration,
                extension,
                id_tvshow: Uuid::new_v4(),
                stem,
            };

        }


        Ok(Response::new(Full::new(Bytes::from(""))))
    } else {
        Ok(Response::new(Full::new(Bytes::from(""))))
    }
}