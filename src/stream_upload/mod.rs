use std::{fmt::Debug, path::PathBuf, pin::Pin, task::Poll, time::Instant};

use bytes::{Buf, Bytes, BytesMut};
use futures::{FutureExt, Stream, StreamExt, ready};
use mime::Mime;
use multer::{Field, Multipart};
use tokio::{fs::File, io::AsyncWrite};

const DEFAULT_BUFFER: usize = 8 * 1024;

pub struct Upload<'a> {
    stream: StreamUpload<'a>,
    path: PathBuf,
    buffer: Option<Buffer>,
    state: StateUpload,
    written: usize,
    elapsed: Option<Instant>,
    file_name: Option<String>,
}

#[derive(Debug)]
pub struct StreamUpload<'a> {
    multipart: Multipart<'a>,
    allowed: Vec<Mime>,
    state: State<'a>,
}

#[derive(Debug)]
pub enum State<'a> {
    WaitingField,
    ReadingField(Field<'a>),
    Done,
}

pub enum StateUpload {
    Writting(Bytes),
    Reading,
    Flush,
    Create(Pin<Box<dyn Future<Output = tokio::io::Result<File>> + Send>>),
    Done,
}

pub enum ResultStream {
    Bytes(Bytes),
    New(String),
    Eof,
}

impl<'a> StreamUpload<'a> {
    pub fn new(multipart: Multipart<'a>, allowed: Vec<Mime>) -> Self {
        Self {
            multipart,
            allowed,
            state: State::WaitingField,
        }
    }
}

impl Stream for StreamUpload<'_> {
    type Item = Result<ResultStream, StreamUploadError>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let this = unsafe { self.get_unchecked_mut() };

        match &mut this.state {
            State::WaitingField => match ready!(this.multipart.poll_next_field(cx)) {
                Ok(Some(field)) => {
                    let Some(name) = field.file_name().map(ToString::to_string) else {
                        return Poll::Ready(Some(Err(StreamUploadError)));
                    };

                    this.state = State::ReadingField(field);

                    Poll::Ready(Some(Ok(ResultStream::New(name))))
                }
                Ok(None) => {
                    this.state = State::Done;
                    Poll::Ready(None)
                }
                Err(_e) => {
                    this.state = State::Done;
                    Poll::Ready(Some(Err(StreamUploadError)))
                }
            },
            State::ReadingField(field) => match ready!(field.poll_next_unpin(cx)) {
                Some(Ok(bytes)) => Poll::Ready(Some(Ok(ResultStream::Bytes(bytes)))),
                Some(Err(_e)) => {
                    this.state = State::Done;
                    Poll::Ready(Some(Err(StreamUploadError)))
                }
                None => {
                    this.state = State::WaitingField;
                    Poll::Ready(Some(Ok(ResultStream::Eof)))
                }
            },
            State::Done => Poll::Ready(None),
        }
    }
}

#[derive(Debug)]
pub struct UploadResult {
    size: usize,
    elapsed: u64,
    file_name: String,
}

#[derive(Debug)]
pub struct StreamUploadError;

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
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        todo!()
    }
}

pub struct WriterError;

impl<'a> Upload<'a> {
    pub fn new(path: PathBuf, stream: StreamUpload<'a>) -> Self {
        Self {
            stream,
            path,
            buffer: None,
            state: StateUpload::Reading,
            written: 0,
            elapsed: None,
            file_name: None,
        }
    }
}

impl Stream for Upload<'_> {
    type Item = Result<UploadResult, StreamUploadError>;

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
                        panic!("aaa");
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
                        Err(_e) => {
                            this.state = StateUpload::Done;
                            return Poll::Ready(Some(Err(StreamUploadError)));
                        }
                    }
                }
                StateUpload::Reading => {
                    let stream = unsafe { Pin::new_unchecked(&mut this.stream) };
                    match ready!(stream.poll_next(cx)) {
                        Some(Ok(ResultStream::New(name))) => {
                            let mut path = this.path.clone();
                            path.push(&name);
                            this.file_name = Some(name);
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
                    Err(_e) => {
                        this.state = StateUpload::Done;
                        break Poll::Ready(Some(Err(StreamUploadError)));
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

                            break Poll::Ready(Some(Ok(UploadResult {
                                size,
                                elapsed,
                                file_name: this.file_name.take().unwrap(),
                            })));
                        }
                        Err(_e) => {
                            this.state = StateUpload::Done;
                            break Poll::Ready(Some(Err(StreamUploadError)));
                        }
                    }
                }
                StateUpload::Done => break Poll::Ready(None),
            }
        }
    }
}
