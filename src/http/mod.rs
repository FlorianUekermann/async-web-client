use self::body::IntoNonUnitRequestBody;
pub use self::body::IntoRequestBody;
pub use self::error::HttpError;
use futures::{future::FusedFuture, ready, AsyncRead, AsyncReadExt, Future};
use futures_rustls::rustls::ClientConfig;
use serde::de::DeserializeOwned;
use std::fmt::{Debug, Formatter};
use std::io::ErrorKind::InvalidData;
use std::sync::Arc;
use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};
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
    fn send<B: IntoRequestBody + 'a>(&self, body: B) -> RequestSend<'a> {
        let client_config = crate::DEFAULT_CLIENT_CONFIG.clone();
        self.send_with_client_config(body, client_config)
    }
    fn send_with_client_config<B: IntoRequestBody + 'a>(&self, body: B, client_config: Arc<ClientConfig>) -> RequestSend<'a>;
}

pub trait RequestExt {
    type Body;
    fn swap_body<T>(self, body: T) -> (http::Request<T>, Self::Body);
}

impl<B> RequestExt for http::Request<B> {
    type Body = B;
    fn swap_body<T>(self, body: T) -> (http::Request<T>, Self::Body) {
        let (head, old_body) = self.into_parts();
        (http::Request::from_parts(head, body), old_body)
    }
}

impl<'a, T: IntoNonUnitRequestBody + 'a> RequestWithBodyExt<'a> for http::Request<T> {
    type B = T;
    fn send_with_client_config(self, client_config: Arc<ClientConfig>) -> RequestSend<'a> {
        let (this, body) = self.swap_body(());
        this.send_with_client_config(body, client_config)
    }
}

impl<'a> RequestWithoutBodyExt<'a> for http::Request<()> {
    fn send_with_client_config<B: IntoRequestBody + 'a>(&self, body: B, client_config: Arc<ClientConfig>) -> RequestSend<'a> {
        let (read, len) = body.into_request_body();
        let body: (Pin<Box<dyn AsyncRead + Send>>, _) = (Box::pin(read), len);
        let inner = RequestSendInner::new_with_client_config(self.clone(), body, client_config);
        RequestSend { inner }
    }
}

pub struct RequestSend<'a>
where
    Self: Send,
{
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

pub struct ResponseBody {
    inner: ResponseBodyInner,
}

#[cfg(feature = "websocket")]
impl ResponseBody {
    pub(crate) fn into_inner(self) -> Result<(async_http_codec::BodyDecodeState, crate::Transport), HttpError> {
        self.inner.into_inner()
    }
}
impl ResponseBody {
    pub async fn bytes(&mut self, limit: Option<usize>) -> Result<Vec<u8>, io::Error> {
        let mut result = Vec::new();
        match limit {
            None => {
                self.read_to_end(&mut result).await?;
            }
            Some(l) => {
                self.take(l as u64).read_to_end(&mut result).await?;
                if self.read(&mut [0u8]).await? > 0 {
                    return Err(io::ErrorKind::OutOfMemory.into());
                }
            }
        };
        Ok(result)
    }

    pub async fn string(&mut self, limit: Option<usize>) -> Result<String, io::Error> {
        let mut result = String::new();
        match limit {
            None => {
                self.read_to_string(&mut result).await?;
            }
            Some(l) => {
                self.take(l as u64).read_to_string(&mut result).await?;
                if self.read(&mut [0u8]).await? > 0 {
                    return Err(io::ErrorKind::OutOfMemory.into());
                }
            }
        };
        Ok(result)
    }
    #[cfg(feature = "json")]
    pub async fn json<T: DeserializeOwned>(&mut self, limit: Option<usize>) -> Result<T, io::Error> {
        let json_string = self.string(limit).await?;
        let result = serde_json::from_str(&json_string).map_err(|error| HttpError::IoError(io::Error::new(InvalidData, error).into()))?;
        Ok(result)
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
