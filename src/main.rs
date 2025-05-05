mod handlers;
mod models;
mod peer;
mod redirect;
mod repository;
mod stream_upload;

use http::{Request, Response};
use hyper::{body::Body, service::Service};
use models::{logs::Logs, user::User};
use peer::Peer;
use repository::Repository;
use std::{marker::PhantomData, net::SocketAddr, pin::Pin, sync::Arc};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().unwrap();
    tracing_subscriber::fmt().init();

    let socket = SocketAddr::new("0.0.0.0".parse().unwrap(), 4000);
    let listen = TcpListener::bind(socket).await?;

    let db_host = std::env::var("DB_HOST").expect("Database url is not defined");
    let db_port = std::env::var("DB_PORT").expect("Port is not defined");
    let db_user = std::env::var("DB_USERNAME").expect("Database's user is not defined");
    let db_passwd = std::env::var("DB_PASSWD").expect("Database's password is not defined");
    let db_name = std::env::var("DB_NAME").expect("Database's name is not defined");

    let repository = Arc::new(
        Repository::new(
            format!("mongodb://{db_host}:{db_port}"),
            db_user,
            db_passwd,
            db_name,
        )
        .await?,
    );

    repository.create_index::<User>().await?;
    repository.create_index::<Logs>().await?;

    tracing::info!("Listening: {:?}", &socket);
    loop {
        let (stream, _) = listen.accept().await?;
        let peer = stream.peer_addr().ok();
        let repo = Arc::clone(&repository);
        let io = Io::new(stream);
        tokio::task::spawn(async move {
            if let Err(e) = hyper::server::conn::http1::Builder::new()
                .serve_connection(
                    io,
                    service_with_state(repo, |mut req, repo| {
                        let ext = req.extensions_mut();
                        ext.insert(repo);
                        ext.insert(Peer::new(peer));
                        handlers::entry(req)
                    }),
                )
                .await
            {
                tracing::error!("{e:?}");
            }
        });
    }
}

pub struct ServiceWithState<C, R> {
    f: C,
    state: Arc<Repository>,
    _req: PhantomData<fn(R)>,
}

pub fn service_with_state<F, R, S>(state: Arc<Repository>, f: F) -> ServiceWithState<F, R>
where
    F: Fn(Request<R>, Arc<Repository>) -> S,
    S: Future,
{
    ServiceWithState {
        f,
        state,
        _req: PhantomData,
    }
}

impl<C, ReqBody, ResBody, F, E> Service<Request<ReqBody>> for ServiceWithState<C, ReqBody>
where
    C: Fn(Request<ReqBody>, Arc<Repository>) -> F + Copy,
    ReqBody: Body,
    F: Future<Output = Result<Response<ResBody>, E>>,
    ResBody: Body,
    E: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    type Response = Response<ResBody>;

    type Error = E;

    type Future = F;

    fn call(&self, req: Request<ReqBody>) -> Self::Future {
        (self.f)(req, self.state.clone())
    }
}

impl<F: Clone, R> std::clone::Clone for ServiceWithState<F, R> {
    fn clone(&self) -> Self {
        ServiceWithState {
            f: self.f.clone(),
            state: self.state.clone(),
            _req: PhantomData,
        }
    }
}

struct Io<T> {
    inner: T,
}

impl<T> Io<T> {
    pub fn new(inner: T) -> Self {
        Self { inner }
    }
}

impl<T: tokio::io::AsyncRead> hyper::rt::Read for Io<T> {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        mut buf: hyper::rt::ReadBufCursor<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        let tmp = unsafe {
            let mut tbuf = tokio::io::ReadBuf::uninit(buf.as_mut());
            match tokio::io::AsyncRead::poll_read(
                Pin::new_unchecked(&mut self.get_unchecked_mut().inner),
                cx,
                &mut tbuf,
            ) {
                std::task::Poll::Ready(Ok(())) => tbuf.filled().len(),
                e => return e,
            }
        };

        unsafe {
            buf.advance(tmp);
        }

        std::task::Poll::Ready(Ok(()))
    }
}

impl<T: tokio::io::AsyncWrite> hyper::rt::Write for Io<T> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        tokio::io::AsyncWrite::poll_write(
            unsafe { Pin::new_unchecked(&mut self.get_unchecked_mut().inner) },
            cx,
            buf,
        )
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        tokio::io::AsyncWrite::poll_flush(
            unsafe { Pin::new_unchecked(&mut self.get_unchecked_mut().inner) },
            cx,
        )
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        tokio::io::AsyncWrite::poll_shutdown(
            unsafe { Pin::new_unchecked(&mut self.get_unchecked_mut().inner) },
            cx,
        )
    }
}
