use http::{HeaderMap, header};
use std::pin::Pin;

const JWT_IDENTIFIED: &str = "JWT";

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
