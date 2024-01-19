use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};

use futures::{future::FusedFuture, ready, AsyncRead, Future};

pub use self::error::HttpError;

#[cfg(target_arch = "wasm32")]
mod request_wasm;
#[cfg(not(target_arch = "wasm32"))]
mod response_native;

#[cfg(target_arch = "wasm32")]
mod response_wasm;
#[cfg(target_arch = "wasm32")]
type ResponseReadInner = response_wasm::ResponseRead;
#[cfg(not(target_arch = "wasm32"))]
mod request_native;
#[cfg(not(target_arch = "wasm32"))]
type ResponseReadInner = response_native::ResponseRead;

mod common;
mod error;

pub struct RequestSend<'a> {
    inner: request_native::RequestSend<'a>,
}

impl RequestSend<'_> {
    pub fn new(request: &http::Request<impl AsRef<[u8]>>) -> RequestSend<'_> {
        #[cfg(target_arch = "wasm32")]
        todo!();
        #[cfg(not(target_arch = "wasm32"))]
        {
            let inner = request_native::RequestSend::new(request);
            RequestSend { inner }
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
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
