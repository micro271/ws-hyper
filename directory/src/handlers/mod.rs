use crate::state::State;
use futures::{StreamExt, stream::SplitStream};
use http::{StatusCode, header};
use http_body_util::Full;
use hyper::{
    Request, Response,
    body::{Bytes, Incoming},
    upgrade::Upgraded,
};
use hyper_tungstenite::{WebSocketStream, tungstenite::Message};
use hyper_util::rt::TokioIo;
use std::{convert::Infallible, sync::Arc};

type TypeState = Arc<State>;

pub async fn entry(req: Request<Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
    let path = req.uri().path();
    let repo = req.extensions().get::<TypeState>().unwrap();
    Ok(Response::builder()
        .header(header::CONTENT_TYPE, "application/json")
        .body(Full::from(repo.tree_as_json().await.to_string()))
        .unwrap_or_default())
}

pub async fn serve_ws(
    mut ws: SplitStream<WebSocketStream<TokioIo<Upgraded>>>,
) -> Result<(), &'static str> {
    while let Some(Ok(msg)) = ws.next().await {
        match msg {
            Message::Text(txt) => {
                let path = txt.strip_prefix("subscribe: ");
            }
            Message::Ping(bytes) => {
                tracing::debug!("Received ping message: {bytes:02X?}");
            }
            Message::Pong(bytes) => {
                tracing::debug!("Received pong message: {bytes:02X?}");
            }
            Message::Close(close_frame) => {
                if let Some(msg) = close_frame {
                    tracing::debug!(
                        "Received close message with code {} and message: {}",
                        msg.code,
                        msg.reason
                    );
                } else {
                    tracing::debug!("Received close message");
                }
            }
            _ => {}
        }
    }

    Ok(())
}

pub async fn server_upgrade(req: Request<Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
    let state = req.extensions().get::<TypeState>().unwrap().clone();
    if hyper_tungstenite::is_upgrade_request(&req) {
        let (res, ws) = hyper_tungstenite::upgrade(req, None).unwrap();
        let (tx, rx) = ws.await.unwrap().split();
        //state.add_cliente("".to_string(), tx);
        tokio::spawn(async move {
            if let Err(e) = serve_ws(rx).await {
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
