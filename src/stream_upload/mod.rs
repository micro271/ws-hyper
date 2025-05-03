use std::{pin::Pin, task::Poll};

use futures::{Stream, StreamExt};
use mime::Mime;
use multer::{Field, Multipart};
use tokio::io::{AsyncWrite, BufWriter};

const DEFAULT_BUFFER: usize = 8192;

#[derive(Debug)]
pub struct StreamUpload<'a, T: AsyncWrite> {
    multipart: Multipart<'a>,
    writer: BufWriter<T>,
    capacity: usize,
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
            writer: BufWriter::with_capacity(DEFAULT_BUFFER, fd),
            capacity: DEFAULT_BUFFER,
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
                        }
                        Poll::Ready(Err(_)) => break Poll::Ready(Some(Err(StreamUploadError))),
                        Poll::Pending => break Poll::Pending,
                    }
                }
                State::ReadingField(field) => match field.poll_next_unpin(cx) {
                    Poll::Ready(Some(Ok(bytes))) => {
                        let writer = unsafe { Pin::new_unchecked(&mut this.writer) };
                        if let Poll::Ready(Err(_)) = writer.poll_write(cx, &bytes) {
                            return Poll::Ready(Some(Err(StreamUploadError)));
                        }
                    }
                    Poll::Ready(Some(Err(_err))) => {
                        this.state = State::Done;
                        break Poll::Ready(Some(Err(StreamUploadError)));
                    }
                    Poll::Ready(None) => {
                        let writer = unsafe { Pin::new_unchecked(&mut this.writer) };
                        if let Poll::Ready(Err(_)) = writer.poll_flush(cx) {
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
