use bytes::Bytes;
use futures::{Stream, StreamExt, ready};
use mime::{Mime, Name};
use multer::{Field, Multipart};
use std::task::Poll;

use super::error::UploadError;

#[derive(Debug)]
pub struct StreamUpload<'a> {
    multipart: Multipart<'a>,
    allowed: Vec<MimeAllowed>,
    state: State<'a>,
}

#[derive(Debug)]
pub enum MimeAllowed {
    Any,
    Mime(Mime),
    MediaType(Name<'static>),
    SubType(Name<'static>),
}

#[derive(Debug)]
pub enum State<'a> {
    WaitingField,
    ReadingField(Field<'a>),
    Done,
}

pub enum ResultStream {
    Bytes(Bytes),
    New(String),
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
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let this = unsafe { self.get_unchecked_mut() };

        match &mut this.state {
            State::WaitingField => match ready!(this.multipart.poll_next_field(cx)) {
                Ok(Some(field)) => {
                    let Some(name) = field.file_name().map(ToString::to_string) else {
                        this.state = State::Done;
                        return Poll::Ready(Some(Err(UploadError::FileNameNotFound)));
                    };

                    let Some(content_type) = field.content_type() else {
                        return Poll::Ready(Some(Err(UploadError::MimeNotFound(name))));
                    };

                    if this.allowed.iter().any(|x| match x {
                        MimeAllowed::Any => true,
                        MimeAllowed::Mime(mime) => mime == content_type,
                        MimeAllowed::MediaType(name) => content_type.type_().eq(name),
                        MimeAllowed::SubType(name) => content_type.subtype().eq(name),
                    }) {
                        this.state = State::ReadingField(field);

                        Poll::Ready(Some(Ok(ResultStream::New(name))))
                    } else {
                        this.state = State::Done;
                        Poll::Ready(Some(Err(UploadError::MimeNotAllowed {
                            file: name,
                            mime: content_type.clone(),
                        })))
                    }
                }
                Ok(None) => {
                    this.state = State::Done;
                    Poll::Ready(None)
                }
                Err(err) => {
                    this.state = State::Done;
                    Poll::Ready(Some(Err(err.into())))
                }
            },
            State::ReadingField(field) => match ready!(field.poll_next_unpin(cx)) {
                Some(Ok(bytes)) => Poll::Ready(Some(Ok(ResultStream::Bytes(bytes)))),
                Some(Err(err)) => {
                    this.state = State::Done;
                    Poll::Ready(Some(Err(err.into())))
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
