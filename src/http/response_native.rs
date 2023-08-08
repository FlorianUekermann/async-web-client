use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};

use async_http_codec::{BodyDecodeState, ResponseHead};
use futures::AsyncRead;
use http::HeaderValue;

use crate::Transport;

use super::error::HttpError;

pub struct ResponseRead {
    state: BodyDecodeState,
    transport: Option<Transport>,
    error: Option<HttpError>,
}

impl ResponseRead {
    pub(crate) fn new(transport: Transport, head: &ResponseHead) -> Result<Self, HttpError> {
        // TODO: Return HeaderValue in upstream error
        let state =
            BodyDecodeState::from_headers(head.headers()).map_err(|_err| HttpError::UnsupportedTransferEncoding(HeaderValue::from_static("TODO")))?;
        Ok(Self {
            state,
            transport: Some(transport),
            error: None,
        })
    }
    pub(crate) fn into_inner(self) -> Result<(BodyDecodeState, Transport), HttpError> {
        let ResponseRead { state, transport, error } = self;
        if let Some(err) = error {
            return Err(err);
        }
        Ok((state, transport.unwrap()))
    }
}

impl AsyncRead for ResponseRead {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        if let Some(err) = &self.error {
            return Poll::Ready(Err(err.clone().into()));
        }
        let mut transport = self.transport.take().unwrap();
        match self.state.poll_read(&mut transport, cx, buf) {
            Poll::Ready(Err(err)) => {
                // TODO: Return HeaderValue in upstream error
                self.error = Some(HttpError::IoError(err.into()));
                Poll::Ready(Err(self.error.clone().unwrap().into()))
            }
            p => {
                self.transport = Some(transport);
                p
            }
        }
    }
}
