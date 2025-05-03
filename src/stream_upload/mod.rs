use std::{pin::Pin, task::Poll};

use bytes::{Buf, BytesMut};
use futures::{Stream, StreamExt};
use mime::Mime;
use multer::{Field, Multipart};
use tokio::io::AsyncWrite;

const DEFAULT_BUFFER: usize = 8192;

#[derive(Debug)]
pub struct StreamUpload<'a, T: AsyncWrite> {
    multipart: Multipart<'a>,
    writer: Buffer<T>,
    allowed: Vec<Mime>,
    state: State<'a>,
}

#[derive(Debug)]
pub enum State<'a> {
    WaitingField,
    ReadingField(Field<'a>),
    Done,
}

impl<'a, T> StreamUpload<'a, T>
where
    T: AsyncWrite,
{
    pub fn new(multipart: Multipart<'a>, fd: T, allowed: Vec<Mime>) -> Self {
        Self {
            multipart,
            writer: Buffer::new(fd),
            allowed,
            state: State::WaitingField,
        }
    }
}

impl<'a, T> Stream for StreamUpload<'a, T>
where
    T: AsyncWrite,
{
    type Item = Result<UploadResult, StreamUploadError>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let this = unsafe { self.get_unchecked_mut() };

        loop {
            match &mut this.state {
                State::WaitingField => {
                    let mut multer = unsafe { Pin::new_unchecked(&mut this.multipart) };

                    match multer.poll_next_field(cx) {
                        Poll::Ready(Ok(Some(field))) => {
                            this.state = State::ReadingField(field);
                        }
                        Poll::Ready(Ok(None)) => {
                            this.state = State::Done;
                            break Poll::Ready(None);
                        }
                        Poll::Ready(Err(_)) => {
                            this.state = State::Done;
                            break Poll::Ready(Some(Err(StreamUploadError)));
                        }
                        Poll::Pending => break Poll::Pending,
                    }
                }
                State::ReadingField(field) => match field.poll_next_unpin(cx) {
                    Poll::Ready(Some(Ok(bytes))) => {
                        let writer = unsafe { Pin::new_unchecked(&mut this.writer) };
                        if let Poll::Ready(Err(_)) = writer.write(cx, &bytes) {
                            this.state = State::Done;
                            return Poll::Ready(Some(Err(StreamUploadError)));
                        }
                    }
                    Poll::Ready(Some(Err(_err))) => {
                        this.state = State::Done;
                        break Poll::Ready(Some(Err(StreamUploadError)));
                    }
                    Poll::Ready(None) => {
                        let writer = unsafe { Pin::new_unchecked(&mut this.writer) };
                        if let Poll::Ready(Err(_)) = writer.flush(cx) {
                            this.state = State::Done;
                            break Poll::Ready(Some(Err(StreamUploadError)));
                        } else {
                            this.state = State::WaitingField
                        }
                    }
                    Poll::Pending => break Poll::Pending,
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
    fn write(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        let this = unsafe { self.get_unchecked_mut() };
        let writer = unsafe { Pin::new_unchecked(&mut this.inner) };

        this.bytes.extend_from_slice(buf);

        match writer.poll_write(cx, &this.bytes) {
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
                } else {
                    this.bytes.advance(n);
                }

                Poll::Ready(Ok(buf.len()))
            }
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            Poll::Pending => Poll::Pending,
        }
    }

    fn flush(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        let this = unsafe { self.get_unchecked_mut() };
        let mut writer = unsafe { Pin::new_unchecked(&mut this.inner) };

        while !this.bytes.is_empty() {
            match writer.as_mut().poll_write(cx, &this.bytes) {
                Poll::Ready(Ok(n)) => this.bytes.advance(n),
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => return Poll::Pending,
            };
        }

        writer.poll_flush(cx)
    }

    fn shutdown(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        todo!()
    }
}

pub struct WriterError;
