pub mod error;
pub mod stream;

use bytes::{Buf, Bytes, BytesMut};
use error::UploadError;
use futures::{FutureExt, Stream, ready};
use mime::Mime;
use std::{fmt::Debug, path::PathBuf, pin::Pin, task::Poll, time::Instant};
use stream::{ResultStream, StreamUpload};
use tokio::{fs::File, io::AsyncWrite};

const DEFAULT_BUFFER: usize = 8 * 1024;

pub struct Upload<'a, F> {
    stream: StreamUpload<'a>,
    buffer: Option<Buffer>,
    state: StateUpload,
    written: usize,
    elapsed: Option<Instant>,
    meta_file: Option<MetaFile>,
    path: F,
}

pub struct MetaFile {
    file_name: String,
    mime: Mime,
}

impl MetaFile {
    fn file_name(&self) -> &str {
        &self.file_name
    }
    fn mime(&self) -> &Mime {
        &self.mime
    }
}

pub enum StateUpload {
    Writting(Bytes),
    Reading,
    Flush,
    Create(Pin<Box<dyn Future<Output = tokio::io::Result<File>> + Send>>),
    Done,
}

#[derive(Debug, Default)]
pub struct UploadResult {
    pub size: usize,
    pub elapsed: u64,
    pub file_name: String,
}

impl UploadResult {
    fn new(size: usize, elapsed: u64, file_name: String) -> Self {
        Self {
            size,
            elapsed,
            file_name,
        }
    }
}

#[derive(Debug)]
pub struct Buffer {
    inner: tokio::fs::File,
    bytes: BytesMut,
    capacity: usize,
}

impl Buffer {
    pub fn new(writer: tokio::fs::File) -> Self {
        Buffer {
            inner: writer,
            bytes: BytesMut::new(),
            capacity: DEFAULT_BUFFER,
        }
    }
}

impl Buffer {
    pub fn write_flush(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        let this = unsafe { self.get_unchecked_mut() };
        let mut writer = unsafe { Pin::new_unchecked(&mut this.inner) };

        loop {
            if this.bytes.is_empty() {
                return Poll::Ready(Ok(()));
            }
            match writer.as_mut().poll_write(cx, &this.bytes) {
                Poll::Ready(Ok(n)) => {
                    if n == 0 {
                        return Poll::Ready(Err(std::io::Error::new(
                            std::io::ErrorKind::UnexpectedEof,
                            "0 bytes written",
                        )));
                    }
                    let len_buf = this.bytes.len();

                    if len_buf < n {
                        panic!("You wrote {n} bytes when the buffer have {len_buf} bytes");
                    }
                    this.bytes.advance(n);
                }
                Poll::Ready(Err(e)) => break Poll::Ready(Err(e)),
                Poll::Pending => break Poll::Pending,
            }
        }
    }
}

impl AsyncWrite for Buffer {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        if buf.len() + self.bytes.len() >= self.capacity {
            ready!(self.as_mut().write_flush(cx))?;
        }

        let this = unsafe { self.get_unchecked_mut() };
        if buf.len() >= this.capacity {
            unsafe { Pin::new_unchecked(&mut this.inner) }.poll_write(cx, buf)
        } else {
            this.bytes.extend_from_slice(buf);
            Poll::Ready(Ok(buf.len()))
        }
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        let this = unsafe { self.get_unchecked_mut() };
        let mut writer = unsafe { Pin::new_unchecked(&mut this.inner) };

        while !this.bytes.is_empty() {
            match ready!(writer.as_mut().poll_write(cx, &this.bytes)) {
                Ok(n) => this.bytes.advance(n),
                Err(e) => return Poll::Ready(Err(e)),
            }
        }

        writer.poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        ready!(self.as_mut().poll_flush(cx))?;

        let this = unsafe { self.get_unchecked_mut() };

        unsafe { Pin::new_unchecked(&mut this.inner) }.poll_shutdown(cx)
    }
}

impl<'a, F> Upload<'a, F>
where
    F: Fn(&Mime) -> PathBuf + Send + Sync,
{
    pub fn new(stream: StreamUpload<'a>, conditional_path: F) -> Self {
        Self {
            stream,
            path: conditional_path,
            buffer: None,
            state: StateUpload::Reading,
            written: 0,
            elapsed: None,
            meta_file: None,
        }
    }
}

impl<F> Stream for Upload<'_, F>
where
    F: Fn(&Mime) -> PathBuf + Send + Sync,
{
    type Item = Result<UploadResult, UploadError>;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        let this = unsafe { self.get_unchecked_mut() };

        loop {
            match &mut this.state {
                StateUpload::Writting(buf) => {
                    let writer = if let Some(e) = this.buffer.as_mut() {
                        unsafe { Pin::new_unchecked(e) }
                    } else {
                        panic!("Buffer is not present");
                    };

                    match ready!(writer.poll_write(cx, buf)) {
                        Ok(n) => {
                            if n < buf.len() {
                                this.state =
                                    StateUpload::Writting(Bytes::copy_from_slice(&buf[n..]));
                            } else {
                                this.state = StateUpload::Reading;
                            }
                            this.written += n;
                        }
                        Err(e) => {
                            this.state = StateUpload::Done;
                            return Poll::Ready(Some(Err(e.into())));
                        }
                    }
                }
                StateUpload::Reading => {
                    let stream = unsafe { Pin::new_unchecked(&mut this.stream) };
                    match ready!(stream.poll_next(cx)) {
                        Some(Ok(ResultStream::New(meta))) => {
                            let mut path = (this.path)(meta.mime());

                            path.push(meta.file_name());
                            this.meta_file = Some(meta);
                            this.state = StateUpload::Create(File::create(path).boxed());
                            this.elapsed = Some(Instant::now());
                        }
                        Some(Ok(ResultStream::Bytes(bytes))) => {
                            this.state = StateUpload::Writting(bytes);
                        }
                        Some(Ok(ResultStream::Eof)) => {
                            this.state = StateUpload::Flush;
                        }
                        Some(Err(e)) => {
                            this.state = StateUpload::Done;
                            break Poll::Ready(Some(Err(e)));
                        }
                        None => {
                            this.state = StateUpload::Done;
                            break Poll::Ready(None);
                        }
                    }
                }
                StateUpload::Create(new_file) => match ready!(new_file.poll_unpin(cx)) {
                    Ok(file) => {
                        this.buffer = Some(Buffer::new(file));
                        this.state = StateUpload::Reading;
                    }
                    Err(e) => {
                        this.state = StateUpload::Done;
                        break Poll::Ready(Some(Err(e.into())));
                    }
                },
                StateUpload::Flush => {
                    let writer = unsafe { Pin::new_unchecked(this.buffer.as_mut().unwrap()) };
                    match ready!(writer.poll_flush(cx)) {
                        Ok(()) => {
                            this.state = StateUpload::Reading;
                            let size = this.written;
                            this.written = 0;

                            let elapsed = this
                                .elapsed
                                .take()
                                .map(|x| x.elapsed().as_secs())
                                .unwrap_or_default();

                            break Poll::Ready(Some(Ok(UploadResult::new(
                                size,
                                elapsed,
                                this.meta_file
                                    .take()
                                    .map(|x| x.file_name)
                                    .unwrap_or_default(),
                            ))));
                        }
                        Err(e) => {
                            this.state = StateUpload::Done;
                            break Poll::Ready(Some(Err(e.into())));
                        }
                    }
                }
                StateUpload::Done => break Poll::Ready(None),
            }
        }
    }
}
