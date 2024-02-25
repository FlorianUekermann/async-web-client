use futures::{future::FusedFuture, ready, AsyncRead, AsyncReadExt, Future};
use futures_rustls::rustls::ClientConfig;
use std::fmt::{Debug, Formatter};
use std::sync::Arc;
use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};

use self::body::IntoNonUnitRequestBody;
pub use self::body::IntoRequestBody;
pub use self::error::HttpError;

mod body;
mod common;
mod error;
mod request_native;
mod response_native;

type RequestSendInner<'a> = request_native::RequestSend<'a>;

pub trait RequestWithBodyExt<'a>: Sized {
    type B: IntoNonUnitRequestBody;
    #[cfg(any(feature = "ring", feature = "aws-lc-rs"))]
    fn send(self) -> RequestSend<'a> {
        let client_config = crate::DEFAULT_CLIENT_CONFIG.clone();
        self.send_with_client_config(client_config)
    }
    fn send_with_client_config(self, client_config: Arc<ClientConfig>) -> RequestSend<'a>;
}

pub trait RequestWithoutBodyExt<'a>: Sized {
    #[cfg(any(feature = "ring", feature = "aws-lc-rs"))]
    fn send<B: IntoRequestBody + 'a>(self, body: B) -> RequestSend<'a> {
        let client_config = crate::DEFAULT_CLIENT_CONFIG.clone();
        self.send_with_client_config(body, client_config)
    }
    fn send_with_client_config<B: IntoRequestBody + 'a>(self, body: B, client_config: Arc<ClientConfig>) -> RequestSend<'a>;
}

impl<'a, T: IntoNonUnitRequestBody + Clone> RequestWithBodyExt<'a> for &'a http::Request<T> {
    type B = T;
    fn send_with_client_config(self, client_config: Arc<ClientConfig>) -> RequestSend<'a> {
        let cloned_self = (*self).clone();
        let (read, len) = cloned_self.into_body().into_request_body();
        let body: (Pin<Box<dyn AsyncRead>>, _) = (Box::pin(read), len);
        let inner = RequestSendInner::new_with_client_config(self, body, client_config);
        RequestSend { inner }
    }
}

impl<'a> RequestWithoutBodyExt<'a> for &'a http::Request<()> {
    fn send_with_client_config<B: IntoRequestBody + 'a>(self, body: B, client_config: Arc<ClientConfig>) -> RequestSend<'a> {
        let (read, len) = body.into_request_body();
        let body: (Pin<Box<dyn AsyncRead>>, _) = (Box::pin(read), len);
        let inner = RequestSendInner::new_with_client_config(self, body, client_config);
        RequestSend { inner }
    }
}

pub struct RequestSend<'a> {
    inner: RequestSendInner<'a>,
}

impl Future for RequestSend<'_> {
    type Output = Result<http::Response<ResponseBody>, HttpError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let response = ready!(self.inner.poll(cx))?;
        Ok(response.map(|inner| ResponseBody { inner })).into()
    }
}

impl FusedFuture for RequestSend<'_> {
    fn is_terminated(&self) -> bool {
        self.inner.is_terminated()
    }
}

type ResponseBodyInner = response_native::ResponseBodyInner;

pub trait ResponseExt {
    #[doc(hidden)]
    fn body_as_async_read(&mut self) -> &mut ResponseBody;

    #[allow(async_fn_in_trait)]
    async fn body_vec(&mut self, limit: Option<usize>) -> io::Result<Vec<u8>> {
        let mut buf = Vec::new();
        match limit {
            None => {
                self.body_as_async_read().read_to_end(&mut buf).await?;
            }
            Some(l) => {
                self.body_as_async_read().take(l as u64).read_to_end(&mut buf).await?;
                if self.body_as_async_read().read(&mut [0u8]).await? > 0 {
                    return Err(io::ErrorKind::OutOfMemory.into());
                }
            }
        };
        Ok(buf)
    }

    #[allow(async_fn_in_trait)]
    async fn body_string(&mut self, limit: Option<usize>) -> io::Result<String> {
        let mut buf = String::new();
        match limit {
            None => {
                self.body_as_async_read().read_to_string(&mut buf).await?;
            }
            Some(l) => {
                self.body_as_async_read().take(l as u64).read_to_string(&mut buf).await?;
                if self.body_as_async_read().read(&mut [0u8]).await? > 0 {
                    return Err(io::ErrorKind::OutOfMemory.into());
                }
            }
        };
        Ok(buf)
    }
}

impl ResponseExt for http::Response<ResponseBody> {
    #[doc(hidden)]
    fn body_as_async_read(&mut self) -> &mut ResponseBody {
        self.body_mut()
    }
}

pub struct ResponseBody {
    inner: ResponseBodyInner,
}

#[cfg(feature = "websocket")]
impl ResponseBody {
    pub(crate) fn into_inner(self) -> Result<(async_http_codec::BodyDecodeState, crate::Transport), HttpError> {
        self.inner.into_inner()
    }
}

impl AsyncRead for ResponseBody {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl Debug for ResponseBody {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResponseBody").finish()
    }
}
