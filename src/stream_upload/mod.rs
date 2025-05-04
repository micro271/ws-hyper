use std::{pin::Pin, task::Poll, time::Duration};

use bytes::{Buf, Bytes, BytesMut};
use futures::{Stream, StreamExt, ready};
use mime::Mime;
use multer::{Field, Multipart};
use tokio::io::AsyncWrite;

const DEFAULT_BUFFER: usize = 64 * 1024;

#[derive(Debug)]
pub struct Upload<'a, T> {
    stream: StreamUpload<'a>,
    buffer: Buffer<T>,
    state: StateUpload,
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

#[derive(Debug)]
pub enum StateUpload {
    Writting(Bytes),
    Reading,
    Flush,
    Done,
}

pub enum ResultStream {
    Bytes(Bytes),
    EOF,
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

impl<'a> Stream for StreamUpload<'a> {
    type Item = Result<ResultStream, StreamUploadError>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let this = unsafe { self.get_unchecked_mut() };
        let change = false;

        loop {
            match &mut this.state {
                State::WaitingField => match ready!(this.multipart.poll_next_field(cx)) {
                    Ok(Some(field)) => {
                        println!("Obtenemos un field");
                        this.state = State::ReadingField(field);
                    }
                    Ok(None) => {
                        this.state = State::Done;
                        break Poll::Ready(None);
                    }
                    Err(_e) => {
                        this.state = State::Done;
                        break Poll::Ready(Some(Err(StreamUploadError)));
                    }
                },
                State::ReadingField(field) => match ready!(field.poll_next_unpin(cx)) {
                    Some(Ok(bytes)) => break Poll::Ready(Some(Ok(ResultStream::Bytes(bytes)))),
                    Some(Err(_e)) => {
                        this.state = State::Done;
                        break Poll::Ready(Some(Err(StreamUploadError)));
                    }
                    None => {
                        this.state = State::WaitingField;
                        break Poll::Ready(Some(Ok(ResultStream::EOF)));
                    }
                },
                State::Done => break Poll::Ready(None),
            }
        }
    }
}

#[derive(Debug)]
pub struct UploadResult {
    size: usize,
    elapsed: u64,
}

#[derive(Debug)]
pub struct StreamUploadError;

#[derive(Debug)]
pub struct Buffer<T> {
    inner: T,
    bytes: BytesMut,
    capacity: usize,
}

impl<T> Buffer<T>
where
    T: AsyncWrite,
{
    pub fn new(writer: T) -> Self {
        Buffer {
            inner: writer,
            bytes: BytesMut::new(),
            capacity: DEFAULT_BUFFER,
        }
    }
}

impl<T: AsyncWrite> Buffer<T> {
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

impl<T: AsyncWrite> AsyncWrite for Buffer<T> {
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

impl<'a, T> Upload<'a, T>
where
    T: AsyncWrite,
{
    pub fn new(file: T, stream: StreamUpload<'a>) -> Self {
        Self {
            stream,
            buffer: Buffer::new(file),
            state: StateUpload::Reading,
        }
    }
}

impl<'a, T> Stream for Upload<'a, T>
where
    T: AsyncWrite,
{
    type Item = Result<UploadResult, StreamUploadError>;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        let this = unsafe { self.get_unchecked_mut() };

        loop {
            match &this.state {
                StateUpload::Writting(buf) => {
                    let mut writer = unsafe { Pin::new_unchecked(&mut this.buffer) };
                    match ready!(writer.as_mut().poll_write(cx, buf)) {
                        Ok(n) => {
                            if n < buf.len() {
                                panic!("Write {n} but, the number of byte is {}", buf.len());
                            }

                            this.state = StateUpload::Reading;
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
                        Some(Ok(ResultStream::Bytes(bytes))) => {
                            this.state = StateUpload::Writting(bytes);
                        }
                        Some(Ok(ResultStream::EOF)) => {
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
                StateUpload::Flush => {
                    let writer = unsafe { Pin::new_unchecked(&mut this.buffer) };
                    match ready!(writer.poll_flush(cx)) {
                        Ok(()) => {
                            this.state = StateUpload::Reading;
                            break Poll::Ready(Some(Ok(UploadResult {
                                size: 0,
                                elapsed: 0,
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
