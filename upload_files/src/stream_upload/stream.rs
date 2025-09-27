use bytes::Bytes;
use futures::{Stream, ready};
use mime::{Mime, Name};
use multer::{Field, Multipart};
use std::{pin::Pin, task::Poll};

use super::{MetaFile, error::UploadError};

#[derive(Debug)]
pub struct StreamUpload<'a> {
    multipart: Multipart<'a>,
    allowed: Vec<MimeAllowed>,
    state: State<'a>,
}

#[derive(Debug)]
pub enum MimeAllowed {
    #[allow(dead_code)]
    Any,
    #[allow(dead_code)]
    Mime(Mime),
    MediaType(Name<'static>),
    #[allow(dead_code)]
    SubType(Name<'static>),
}

#[derive(Debug)]
pub enum State<'a> {
    WaitingField,
    ReadingField(Pin<Box<Field<'a>>>),
    Done,
}

pub enum ResultStream {
    Bytes(Bytes),
    New(MetaFile),
    Eof,
}

impl<'a> StreamUpload<'a> {
    pub fn new(multipart: Multipart<'a>, allowed: Vec<MimeAllowed>) -> Self {
        Self {
            multipart,
            allowed,
            state: State::WaitingField,
        }
    }
}

impl Stream for StreamUpload<'_> {
    type Item = Result<ResultStream, UploadError>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        match &mut self.state {
            State::WaitingField => match ready!(self.multipart.poll_next_field(cx)) {
                Ok(Some(field)) => {
                    let Some(name) = field.file_name().map(ToString::to_string) else {
                        self.state = State::Done;
                        return Poll::Ready(Some(Err(UploadError::FileNameNotFound)));
                    };

                    let Some(content_type) = field.content_type().cloned() else {
                        return Poll::Ready(Some(Err(UploadError::MimeNotFound { file: name })));
                    };

                    if self.allowed.iter().any(|x| match x {
                        MimeAllowed::Any => true,
                        MimeAllowed::Mime(mime) => content_type.eq(mime),
                        MimeAllowed::MediaType(name) => content_type.type_().eq(name),
                        MimeAllowed::SubType(name) => content_type.subtype().eq(name),
                    }) {
                        self.state = State::ReadingField(Box::pin(field));

                        Poll::Ready(Some(Ok(ResultStream::New(MetaFile {
                            file_name: name,
                            mime: content_type,
                        }))))
                    } else {
                        self.state = State::Done;
                        Poll::Ready(Some(Err(UploadError::MimeNotAllowed {
                            file: name,
                            mime: content_type,
                        })))
                    }
                }
                Ok(None) => {
                    self.state = State::Done;
                    Poll::Ready(None)
                }
                Err(err) => {
                    self.state = State::Done;
                    Poll::Ready(Some(Err(err.into())))
                }
            },
            State::ReadingField(field) => match ready!(field.as_mut().poll_next(cx)) {
                Some(Ok(bytes)) => Poll::Ready(Some(Ok(ResultStream::Bytes(bytes)))),
                Some(Err(err)) => {
                    self.state = State::Done;
                    Poll::Ready(Some(Err(err.into())))
                }
                None => {
                    self.state = State::WaitingField;
                    Poll::Ready(Some(Ok(ResultStream::Eof)))
                }
            },
            State::Done => Poll::Ready(None),
        }
    }
}
