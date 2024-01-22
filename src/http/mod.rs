use futures::{future::FusedFuture, ready, AsyncRead, Future};
use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};

pub use self::error::HttpError;

mod common;
mod error;
mod request_native;
mod response_native;

type ResponseReadInner = response_native::ResponseRead;

pub struct RequestSend<'a> {
    inner: request_native::RequestSend<'a>,
}

impl RequestSend<'_> {
    #[cfg(any(feature = "ring", feature = "aws-lc-rs"))]
    pub fn new(request: &http::Request<impl AsRef<[u8]>>) -> RequestSend<'_> {
        let inner = request_native::RequestSend::new(request);
        RequestSend { inner }
    }
    pub fn new_with_client_config(request: &http::Request<impl AsRef<[u8]>>, client_config: std::sync::Arc<crate::ClientConfig>) -> RequestSend<'_> {
        let inner = request_native::RequestSend::new_with_client_config(request, client_config);
        RequestSend { inner }
    }
}

impl Future for RequestSend<'_> {
    type Output = Result<http::Response<ResponseRead>, HttpError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let response = ready!(self.inner.poll(cx))?;
        Ok(response.map(|inner| ResponseRead { inner })).into()
    }
}

impl FusedFuture for RequestSend<'_> {
    fn is_terminated(&self) -> bool {
        self.inner.is_terminated()
    }
}

pub struct ResponseRead {
    inner: ResponseReadInner,
}

#[cfg(feature = "websocket")]
impl ResponseRead {
    pub(crate) fn into_inner(self) -> Result<(async_http_codec::BodyDecodeState, crate::Transport), HttpError> {
        self.inner.into_inner()
    }
}

impl AsyncRead for ResponseRead {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}
