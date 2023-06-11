use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use async_http_codec::internal::buffer_write::BufferWriteState;
use async_http_codec::internal::io_future::IoFutureState;
use async_http_codec::{BodyEncodeState, RequestHead, ResponseHead};
use async_net::TcpStream;
use futures::future::poll_fn;
use futures::{ready, AsyncWrite, Future};
use http::header::TRANSFER_ENCODING;
use http::uri::Scheme;

use super::error::HttpError;
use super::response_native::ResponseRead;

pub struct RequestWrite {
    error: Option<HttpError>,
    pending_connect: Option<Pin<Box<dyn Future<Output = io::Result<TcpStream>>>>>,
    pending_head: Option<BufferWriteState>,
    transport: Option<TcpStream>,
    body_encode_state: Option<BodyEncodeState>,
}

impl RequestWrite {
    pub fn start<T>(request: &http::Request<T>) -> Self {
        let https = match request.uri().scheme() {
            Some(scheme) => match scheme {
                _ if scheme == &Scheme::HTTP => false,
                _ if scheme == &Scheme::HTTPS => true,
                scheme => return Self::error(HttpError::UnexpectedScheme(scheme.clone())),
            },
            None => true,
        };
        let host = match request.uri().host() {
            Some(host) => host.to_string(),
            None => return Self::error(HttpError::RelativeUri),
        };
        let port = match request.uri().port_u16() {
            Some(port) => port,
            None => match https {
                true => 443u16,
                false => 80u16,
            },
        };
        let mut head = RequestHead::ref_request(request);
        head.headers_mut()
            .insert(TRANSFER_ENCODING, "chunked".parse().unwrap());
        Self {
            error: None,
            pending_connect: Some(Box::pin(TcpStream::connect((host, port)))),
            pending_head: Some(head.encode_state()),
            transport: None,
            body_encode_state: Some(BodyEncodeState::new(None)),
        }
    }
    pub async fn response(mut self) -> Result<(http::Response<()>, ResponseRead), HttpError> {
        if let Err(_) = poll_fn(|cx| Pin::new(&mut self).poll_close(cx)).await {
            return Err(self.error.unwrap().clone());
        }
        let t = self.transport.take().unwrap();
        let (t, head) = match ResponseHead::decode(t).await {
            Ok((t, head)) => (t, head),
            Err(err) => return Err(HttpError::IoError(err.into())), // TODO: better errors upstream
        };
        let resp = ResponseRead::new(t, &head)?;
        Ok((head.into(), resp))
    }
    fn error(err: HttpError) -> Self {
        Self {
            error: Some(err),
            pending_connect: None,
            pending_head: None,
            transport: None,
            body_encode_state: None,
        }
    }
    fn poll_before_body(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), HttpError>> {
        if let Some(fut) = &mut self.pending_connect {
            match ready!(fut.as_mut().poll(cx)) {
                Ok(transport) => {
                    self.transport = Some(transport);
                    self.pending_connect = None;
                }
                Err(err) => {
                    let err = HttpError::ConnectError(Arc::new(err));
                    *self = Self::error(err.clone());
                    return Poll::Ready(Err(err));
                }
            }
        }
        let transport = self.transport.as_mut().unwrap();
        if let Some(state) = &mut self.pending_head {
            match ready!(state.poll(cx, transport)) {
                Ok(()) => self.pending_head = None,
                Err(err) => {
                    let err = HttpError::IoError(Arc::new(err));
                    *self = Self::error(err.clone());
                    return Poll::Ready(Err(err));
                }
            }
        }
        Poll::Ready(Ok(()))
    }
    fn already_closed() -> io::Error {
        io::Error::new(io::ErrorKind::NotConnected, "already closed")
    }
}

impl AsyncWrite for RequestWrite {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        if let Some(err) = self.error.clone() {
            return Poll::Ready(Err(err.into()));
        }
        if let Err(err) = ready!(self.poll_before_body(cx)) {
            return Poll::Ready(Err(err.into()));
        }
        let t = match self.transport.take() {
            Some(t) => t,
            None => return Poll::Ready(Err(Self::already_closed())),
        };
        let mut w = self.body_encode_state.take().unwrap().into_async_write(t);
        let p = match Pin::new(&mut w).poll_write(cx, buf) {
            Poll::Ready(Err(err)) => {
                let err = HttpError::IoError(err.into());
                *self = Self::error(err.clone());
                Poll::Ready(Err(err.into()))
            }
            p => p,
        };
        let (t, s) = w.checkpoint();
        self.body_encode_state = Some(s);
        if self.error.is_none() {
            self.transport = Some(t);
        }
        p
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        if let Some(err) = self.error.clone() {
            return Poll::Ready(Err(err.into()));
        }
        if let Err(err) = ready!(self.poll_before_body(cx)) {
            return Poll::Ready(Err(err.into()));
        }
        let t = match self.transport.take() {
            Some(t) => t,
            None => return Poll::Ready(Err(Self::already_closed())),
        };
        let mut w = self.body_encode_state.take().unwrap().into_async_write(t);
        let p = match Pin::new(&mut w).poll_flush(cx) {
            Poll::Ready(Err(err)) => {
                let err = HttpError::IoError(err.into());
                *self = Self::error(err.clone());
                Poll::Ready(Err(err.into()))
            }
            p => p,
        };
        let (t, s) = w.checkpoint();
        self.body_encode_state = Some(s);
        if self.error.is_none() {
            self.transport = Some(t);
        }
        p
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        if let Some(err) = self.error.clone() {
            return Poll::Ready(Err(err.into()));
        }
        if let Err(err) = ready!(self.poll_before_body(cx)) {
            return Poll::Ready(Err(err.into()));
        }
        let t = match self.transport.take() {
            Some(t) => t,
            None => return Poll::Ready(Err(Self::already_closed())),
        };
        let mut w = self.body_encode_state.take().unwrap().into_async_write(t);
        let p = match Pin::new(&mut w).poll_close(cx) {
            Poll::Ready(Ok(())) => {
                drop(self.transport.take());
                Poll::Ready(Ok(()))
            }
            Poll::Ready(Err(err)) => {
                let err = HttpError::IoError(err.into());
                *self = Self::error(err.clone());
                Poll::Ready(Err(err.into()))
            }
            p => p,
        };
        let (t, s) = w.checkpoint();
        self.body_encode_state = Some(s);
        if self.error.is_none() {
            self.transport = Some(t);
        }
        p
    }
}
