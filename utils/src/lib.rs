pub mod app_info;
pub mod claim;
pub mod connection_manager;
pub mod middleware;
mod peer;

use http::{HeaderMap, Request, Response, header};
use http_body_util::BodyExt;
use hyper::{
    body::{Body, Incoming},
    service::Service,
};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
pub use peer::*;

use p256::{
    ecdsa::{SigningKey, VerifyingKey},
    elliptic_curve::rand_core,
    pkcs8::{EncodePrivateKey, EncodePublicKey},
};
use serde::{Serialize, de::DeserializeOwned};
use std::{fs, marker::PhantomData, path::PathBuf, pin::Pin, sync::Arc};

pub const JWT_IDENTIFIED: &str = "JWT";
const ECDS_PRIV_FILE: &str = "ec_priv_key.pem";
const ECDS_PUB_FILE: &str = "ec_pub_key.pem";
const PKI: &str = "pki_auth";
const ALGORITHM_JWT: Algorithm = Algorithm::ES256;

pub struct JwtHandle;

pub trait GenEcdsa {
    fn gen_ecdsa(path: Option<&str>) -> Result<(), JwtHandleError>;
}

pub trait VerifyTokenEcdsa {
    fn verify_token<B>(token: &str) -> Result<B, JwtHandleError>
    where
        B: DeserializeOwned;
}

pub trait GenTokenFromEcds {
    fn gen_token<T>(claim: claim::Claim<T>) -> Result<String, JwtHandleError>
    where
        T: Serialize;
}

impl VerifyTokenEcdsa for JwtHandle {
    fn verify_token<B>(token: &str) -> Result<B, JwtHandleError>
    where
        B: DeserializeOwned,
    {
        let mut path_pub_key = PathBuf::from(PKI);
        path_pub_key.push(ECDS_PUB_FILE);
        let pem_key = fs::read(path_pub_key).unwrap();
        let dec = DecodingKey::from_ec_pem(&pem_key).unwrap();

        Ok(decode::<B>(token, &dec, &Validation::new(ALGORITHM_JWT))
            .unwrap()
            .claims)
    }
}

impl GenEcdsa for JwtHandle {
    fn gen_ecdsa(path: Option<&str>) -> Result<(), JwtHandleError> {
        let key = SigningKey::random(&mut rand_core::OsRng);
        let pub_key = VerifyingKey::from(&key);

        let private_pem = key.to_pkcs8_pem(Default::default()).unwrap();
        let public_pem = pub_key.to_public_key_pem(Default::default()).unwrap();
        let path = PathBuf::from(path.unwrap_or(PKI));

        if !path.exists() {
            fs::create_dir(&path).unwrap();
        }

        let mut path_privkey_pem = path.clone();
        path_privkey_pem.push(ECDS_PRIV_FILE);

        fs::write(path_privkey_pem, private_pem).unwrap();

        let mut path_pubkey_pem = path;
        path_pubkey_pem.push(ECDS_PUB_FILE);

        fs::write(path_pubkey_pem, public_pem).unwrap();

        Ok(())
    }
}

impl GenTokenFromEcds for JwtHandle {
    fn gen_token<T: Serialize>(claim: claim::Claim<T>) -> Result<String, JwtHandleError> {
        let mut path_priv_key = PathBuf::from(PKI);
        path_priv_key.push(ECDS_PRIV_FILE);

        let priv_key = fs::read(path_priv_key).unwrap();
        let priv_key = EncodingKey::from_ec_pem(&priv_key).unwrap();

        let token = encode(&Header::new(ALGORITHM_JWT), &claim, &priv_key).unwrap();
        Ok(token)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum JwtHandleError {
    EnvNotFound,
    GenEc,
}

impl std::error::Error for JwtHandleError {}

impl std::fmt::Display for JwtHandleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JwtHandleError::EnvNotFound => write!(f, "Var Environment not found"),
            JwtHandleError::GenEc => write!(f, "Gen Ecds fail"),
        }
    }
}

pub trait GetToken {
    fn search(headers: &HeaderMap) -> Option<Self>
    where
        Self: Sized;
    fn get(self) -> String;
}

pub struct JwtCookie(String);

impl GetToken for JwtCookie {
    fn search(headers: &HeaderMap) -> Option<Self>
    where
        Self: Sized,
    {
        headers
            .get(header::COOKIE)
            .and_then(|x| {
                x.to_str().ok().and_then(|x| {
                    x.split(";")
                        .map(str::trim)
                        .find(|x| x.starts_with(JWT_IDENTIFIED))
                        .and_then(|x| x.split("=").nth(1).map(ToString::to_string))
                })
            })
            .map(Self)
    }
    fn get(self) -> String {
        self.0
    }
}

pub struct JwtHeader(String);

impl GetToken for JwtHeader {
    fn search(headers: &HeaderMap) -> Option<Self>
    where
        Self: Sized,
    {
        headers
            .get(header::AUTHORIZATION)
            .and_then(|x| {
                x.to_str()
                    .ok()
                    .and_then(|x| x.strip_prefix("Bearer "))
                    .map(ToString::to_string)
            })
            .map(Self)
    }
    fn get(self) -> String {
        self.0
    }
}

pub struct JwtBoth(String);

impl GetToken for JwtBoth {
    fn search(headers: &HeaderMap) -> Option<Self>
    where
        Self: Sized,
    {
        Token::<JwtCookie>::get_token(headers)
            .or_else(|| Token::<JwtHeader>::get_token(headers))
            .map(Self)
    }

    fn get(self) -> String {
        self.0
    }
}

pub struct Token<T: GetToken>(T);

impl<T: GetToken> Token<T> {
    pub fn get_token(headers: &HeaderMap) -> Option<String> {
        T::search(headers).map(|x| x.get())
    }
}

pub struct Io<T> {
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

pub struct ServiceWithState<C, R, Repo> {
    f: C,
    state: Arc<Repo>,
    _req: PhantomData<fn(R)>,
}

pub fn service_with_state<F, R, S, Repo>(state: Arc<Repo>, f: F) -> ServiceWithState<F, R, Repo>
where
    F: Fn(Request<R>) -> S,
    S: Future,
    Repo: Sync + Send + 'static,
{
    ServiceWithState {
        f,
        state,
        _req: PhantomData,
    }
}

impl<C, ReqBody, ResBody, F, E, Repo> Service<Request<ReqBody>>
    for ServiceWithState<C, ReqBody, Repo>
where
    C: Fn(Request<ReqBody>) -> F,
    ReqBody: Body,
    F: Future<Output = Result<Response<ResBody>, E>>,
    ResBody: Body,
    E: Into<Box<dyn std::error::Error + Send + Sync>>,
    Repo: Sync + Send + 'static,
{
    type Response = Response<ResBody>;

    type Error = E;

    type Future = F;

    fn call(&self, mut req: Request<ReqBody>) -> Self::Future {
        req.extensions_mut().insert(self.state.clone());
        (self.f)(req)
    }
}

impl<F: Clone, R, Repo: Sync + Send + 'static> std::clone::Clone for ServiceWithState<F, R, Repo> {
    fn clone(&self) -> Self {
        ServiceWithState {
            f: self.f.clone(),
            state: self.state.clone(),
            _req: PhantomData,
        }
    }
}

#[derive(Debug)]
pub struct ParseBodyToStruct<T> {
    _phantom: PhantomData<T>,
}

impl<T> ParseBodyToStruct<T>
where
    T: DeserializeOwned,
{
    pub async fn get(body: Incoming) -> Result<T, ParseErrorFromBody> {
        match body.collect().await {
            Ok(e) => match serde_json::from_slice::<'_, T>(&e.to_bytes()) {
                Ok(e) => Ok(e),
                _ => Err(ParseErrorFromBody::new("Parsing data entry error")),
            },
            Err(_) => Err(ParseErrorFromBody::new("Data entry error")),
        }
    }
}

#[derive(Debug)]
pub struct ParseErrorFromBody {
    detail: &'static str,
}
impl ParseErrorFromBody {
    pub fn new(detail: &'static str) -> Self {
        Self { detail }
    }
}

impl std::fmt::Display for ParseErrorFromBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.detail)
    }
}

#[derive(Debug, Clone)]
pub struct TokioExecutor;

impl<F> hyper::rt::Executor<F> for TokioExecutor
where
    F: std::future::Future + Send + 'static,
    F::Output: Send + 'static,
{
    fn execute(&self, fut: F) {
        tokio::task::spawn(fut);
    }
}

pub trait FromPath {
    fn get(path: &str) -> Self;
}
