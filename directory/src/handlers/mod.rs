use futures::StreamExt;
use http::StatusCode;
use http_body_util::Full;
use hyper::{Request, Response, body::Bytes, body::Incoming};
use hyper_tungstenite::{HyperWebsocket, tungstenite::Message};
use std::convert::Infallible;

pub async fn entry(req: Request<Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
    let path = req.uri().path();
    todo!()
}

pub async fn serve_ws(ws: HyperWebsocket) -> Result<(), &'static str> {
    let mut ws = ws.await.unwrap();

    while let Some(Ok(msg)) = ws.next().await {
        match msg {
            Message::Text(txt) => {
                let path = txt.strip_prefix("subscribe: ");
                
            },
            Message::Ping(bytes) => {
                tracing::debug!("Received ping message: {bytes:02X?}");
            },
            Message::Pong(bytes) => {
                tracing::debug!("Received pong message: {bytes:02X?}");
            },
            Message::Close(close_frame) => {
                if let Some(msg) = close_frame {
                    tracing::debug!("Received close message with code {} and message: {}", msg.code, msg.reason);
                } else {
                    tracing::debug!("Received close message");
                }
            },
            _ => {}
        }
    }

    Ok(())
}

pub async fn server_upgrade(req: Request<Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
    if hyper_tungstenite::is_upgrade_request(&req) {
        let (res, ws) = hyper_tungstenite::upgrade(req, None).unwrap();
        tokio::spawn(async move {
            if let Err(e) = serve_ws(ws).await {
                tracing::error!("{e}");
            }
        });
        Ok(res)
    } else {
        Ok(Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Full::default())
            .unwrap_or_default())
    }
}
