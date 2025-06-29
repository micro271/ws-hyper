use http::{HeaderMap, header};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use p256::{
    ecdsa::{SigningKey, VerifyingKey},
    elliptic_curve::rand_core,
    pkcs8::{EncodePrivateKey, EncodePublicKey},
};
use serde::{Serialize, de::DeserializeOwned};
use std::{fs, path::PathBuf, pin::Pin};
const JWT_IDENTIFIED: &str = "JWT";
const ECDS_PRIV_FILE: &str = "ec_priv_key.pem";
const ECDS_PUB_FILE: &str = "ec_pub_key.pem";
const PKI: &str = "pki_auth";
const ALGORITHM_JWT: Algorithm = Algorithm::ES256;

pub struct JwtHandle;

pub trait GenEcdsa {
    fn gen_ecdsa() -> Result<(), JwtHandleError>;
}

pub trait VerifyTokenEcdsa {
    fn verify_token<B>(token: &str) -> Result<B, JwtHandleError>
    where
        B: DeserializeOwned;
}

pub trait GenTokenFromEcds {
    fn gen_token<T, B>(claim: T) -> Result<String, JwtHandleError>
    where
        T: Claims,
        B: Serialize;
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
    fn gen_ecdsa() -> Result<(), JwtHandleError> {
        let key = SigningKey::random(&mut rand_core::OsRng);
        let pub_key = VerifyingKey::from(&key);

        let private_pem = key.to_pkcs8_pem(Default::default()).unwrap();
        let public_pem = pub_key.to_public_key_pem(Default::default()).unwrap();
        let path = PathBuf::from(PKI);

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

impl GenTokenFromEcds for JwtHandleError {
    fn gen_token<T, B>(claim: T) -> Result<String, JwtHandleError>
    where
        T: Claims,
        B: Serialize,
    {
        let mut path_priv_key = PathBuf::from(PKI);
        path_priv_key.push(ECDS_PRIV_FILE);

        let priv_key = fs::read(path_priv_key).unwrap();
        let priv_key = EncodingKey::from_ec_pem(&priv_key).unwrap();

        let token = encode(
            &Header::new(ALGORITHM_JWT),
            &claim.get_claim::<B>(),
            &priv_key,
        )
        .unwrap();
        Ok(token)
    }
}

impl JwtHandle {
    pub fn verify_token<B>(token: String) -> Result<B, JwtHandleError>
    where
        B: DeserializeOwned,
    {
        let mut path = PathBuf::from(PKI);
        path.push(ECDS_PUB_FILE);
        let pub_key = fs::read(path).unwrap();

        let dec = DecodingKey::from_ec_pem(&pub_key).unwrap();

        let validate = Validation::new(ALGORITHM_JWT);

        let decode = decode::<B>(&token, &dec, &validate).unwrap();
        Ok(decode.claims)
    }
}

pub trait Claims {
    fn get_claim<T>(&self) -> T
    where
        T: Serialize;
}

#[derive(Debug)]
pub enum JwtHandleError {
    EnvNotFound,
    GenEc,
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
