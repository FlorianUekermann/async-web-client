use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use async_http_codec::internal::buffer_write::BufferWriteState;
use async_http_codec::internal::io_future::IoFutureState;
use async_http_codec::{BodyEncodeState, RequestHead};
use async_net::TcpStream;
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
    pub async fn response(self) -> Result<(http::Response<()>, ResponseRead), HttpError> {
        ResponseRead::new();
        todo!()
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
}

impl AsyncWrite for RequestWrite {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        _buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        todo!()
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        todo!()
    }

    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        todo!()
    }
}
