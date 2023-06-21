use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};

use futures::{future::FusedFuture, ready, AsyncRead, AsyncWrite, Future};

use self::error::HttpError;

#[cfg(target_arch = "wasm32")]
mod request_wasm;
#[cfg(target_arch = "wasm32")]
type RequestWriteInner = request_wasm::RequestWrite;
#[cfg(not(target_arch = "wasm32"))]
mod response_native;
#[cfg(not(target_arch = "wasm32"))]
type RequestWriteInner = request_native::RequestWrite;

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

pub struct RequestWrite {
    inner: RequestWriteInner,
}

impl RequestWrite {
    pub fn start<T>(request: &http::Request<T>) -> Self {
        let inner = RequestWriteInner::start(request);
        Self { inner }
    }
    pub async fn response(self) -> Result<(http::Response<()>, ResponseRead), HttpError> {
        let (response, inner) = self.inner.response().await?;
        Ok((response, ResponseRead { inner }))
    }
}

impl AsyncWrite for RequestWrite {
    fn poll_write(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_close(cx)
    }
}

pub struct ResponseRead {
    inner: ResponseReadInner,
}

impl AsyncRead for ResponseRead {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}
