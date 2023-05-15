use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use async_http_codec::internal::buffer_write::BufferWriteState;
use async_http_codec::RequestHead;
use async_net::TcpStream;
use futures::{AsyncWrite, Future};
use http::uri::Scheme;

use super::error::HttpError;
use super::response_native::ResponseRead;

pub struct RequestWrite {
    error: Option<HttpError>,
    pending_connect: Option<Box<dyn Future<Output = io::Result<TcpStream>>>>,
    pending_head: Option<BufferWriteState>,
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
        Self {
            error: None,
            pending_connect: Some(Box::new(TcpStream::connect((host, port)))),
            pending_head: Some(RequestHead::ref_request(request).encode_state()),
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
        }
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
