use std::borrow::Cow;

use std::mem::replace;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use async_http_codec::internal::buffer_decode::BufferDecodeState;
use async_http_codec::internal::buffer_write::BufferWriteState;
use async_http_codec::internal::io_future::{IoFutureState, IoFutureWithOutputState};
use async_http_codec::{BodyEncodeState, RequestHead, ResponseHead};

use futures::{AsyncWrite, Future};

use http::uri::Scheme;
use http::{HeaderMap, HeaderValue, Method, Response, Uri, Version};

use crate::{Transport, TransportError};

use super::common::extract_origin;
use super::error::HttpError;
use super::response_native::ResponseRead;

pub(crate) enum RequestSend<'a> {
    Start {
        body: &'a [u8],
        method: Method,
        uri: &'a Uri,
        headers: &'a HeaderMap,
    },
    PendingConnect {
        body: &'a [u8],
        method: Method,
        uri: &'a Uri,
        headers: &'a HeaderMap,
        transport: Pin<Box<dyn Future<Output = Result<Transport, TransportError>>>>,
    },
    SendingHead {
        body: &'a [u8],
        write_state: BufferWriteState,
        transport: Transport,
    },
    SendingBody {
        body: &'a [u8],
        remaining: &'a [u8],
        write_state: BodyEncodeState,
        transport: Transport,
    },
    Flushing {
        transport: Transport,
    },
    ReceivingHead {
        transport: Transport,
        dec_state: BufferDecodeState<ResponseHead<'static>>,
    },
    Finished,
}

impl RequestSend<'_> {
    pub fn new(request: &http::Request<impl AsRef<[u8]>>) -> RequestSend<'_> {
        let body = request.body().as_ref();
        let uri = request.uri();
        let headers = request.headers();
        let method = request.method().clone();
        RequestSend::Start { method, body, uri, headers }
    }
    pub fn poll(&mut self, cx: &mut Context) -> Poll<Result<http::Response<ResponseRead>, HttpError>> {
        loop {
            let s = replace(self, RequestSend::Finished);
            match s {
                RequestSend::Start { method, body, uri, headers } => {
                    let (scheme, host, port) = extract_origin(uri, headers)?;
                    let https = match scheme {
                        _ if scheme == Some(Scheme::HTTP) => false,
                        _ if scheme == Some(Scheme::HTTPS) => true,
                        None => true,
                        Some(scheme) => return Poll::Ready(Err(HttpError::UnexpectedScheme(scheme))),
                    };
                    let port = port.unwrap_or(match https {
                        true => 443,
                        false => 80,
                    });
                    *self = RequestSend::PendingConnect {
                        body,
                        transport: Box::pin(async move { Transport::connect(https, &host, port).await }),
                        method,
                        uri,
                        headers,
                    }
                }
                RequestSend::PendingConnect {
                    body,
                    mut transport,
                    method,
                    uri,
                    headers,
                } => match transport.as_mut().poll(cx) {
                    Poll::Ready(Ok(transport)) => {
                        let (_scheme, host, port) = extract_origin(uri, headers)?;
                        let mut head = RequestHead::new(method, Cow::Borrowed(uri), Version::HTTP_11, Cow::Borrowed(headers));
                        if head.headers().get(http::header::HOST).is_none() {
                            let host = match port {
                                Some(port) => HeaderValue::from_str(&format!("{}:{}", host, port)).unwrap(),
                                None => HeaderValue::from_str(&host).unwrap(),
                            };
                            head.headers_mut().insert(http::header::HOST, host);
                        }
                        if head.headers().get(http::header::CONTENT_LENGTH).is_none() {
                            let length = HeaderValue::from_str(&format!("{}", body.len())).unwrap();
                            head.headers_mut().insert(http::header::CONTENT_LENGTH, length);
                        }
                        let write_state = head.encode_state();
                        *self = RequestSend::SendingHead {
                            write_state,
                            transport,
                            body,
                        };
                    }
                    Poll::Ready(Err(err)) => return Poll::Ready(Err(HttpError::ConnectError(err))),
                    Poll::Pending => {
                        *self = RequestSend::PendingConnect {
                            body,
                            method,
                            uri,
                            headers,
                            transport,
                        };
                        return Poll::Pending;
                    }
                },
                RequestSend::SendingHead {
                    mut write_state,
                    mut transport,
                    body,
                } => match write_state.poll(cx, &mut transport) {
                    Poll::Ready(Ok(())) => {
                        let write_state = BodyEncodeState::new(Some(body.len() as u64));
                        let remaining = body;
                        *self = RequestSend::SendingBody {
                            body,
                            write_state,
                            transport,
                            remaining,
                        }
                    }
                    Poll::Ready(Err(err)) => return Poll::Ready(Err(HttpError::IoError(Arc::new(err)))),
                    Poll::Pending => {
                        *self = RequestSend::SendingHead {
                            write_state,
                            transport,
                            body,
                        };
                        return Poll::Pending;
                    }
                },
                RequestSend::SendingBody {
                    mut write_state,
                    mut transport,
                    body,
                    mut remaining,
                } => match write_state.poll_write(&mut transport, cx, remaining) {
                    Poll::Ready(Ok(n)) => {
                        remaining = &remaining[n..];
                        match remaining.len() {
                            0 => *self = RequestSend::Flushing { transport },
                            _ => {
                                *self = RequestSend::SendingBody {
                                    write_state,
                                    transport,
                                    body,
                                    remaining,
                                }
                            }
                        }
                    }
                    Poll::Ready(Err(err)) => return Poll::Ready(Err(HttpError::IoError(Arc::new(err)))),
                    Poll::Pending => {
                        *self = RequestSend::SendingBody {
                            write_state,
                            transport,
                            body,
                            remaining,
                        };
                        return Poll::Pending;
                    }
                },
                RequestSend::Flushing { mut transport } => match Pin::new(&mut transport).poll_flush(cx) {
                    Poll::Ready(Ok(())) => {
                        let dec_state = ResponseHead::decode_state();
                        *self = RequestSend::ReceivingHead { dec_state, transport }
                    }
                    Poll::Ready(Err(err)) => return Poll::Ready(Err(HttpError::IoError(Arc::new(err)))),
                    Poll::Pending => {
                        *self = RequestSend::Flushing { transport };
                        return Poll::Pending;
                    }
                },
                RequestSend::ReceivingHead {
                    mut dec_state,
                    mut transport,
                } => match dec_state.poll(cx, &mut transport) {
                    Poll::Ready(Ok(head)) => {
                        let body = ResponseRead::new(transport, &head)?;
                        let parts: http::response::Parts = head.into();
                        return Poll::Ready(Ok(Response::from_parts(parts, body)));
                    }
                    Poll::Ready(Err(err)) => return Poll::Ready(Err(HttpError::IoError(Arc::new(err)))),
                    Poll::Pending => {
                        *self = RequestSend::ReceivingHead { transport, dec_state };
                        return Poll::Pending;
                    }
                },
                RequestSend::Finished => panic!("polled finished future"),
            }
        }
    }
    pub fn is_terminated(&self) -> bool {
        match self {
            RequestSend::Finished => true,
            _ => false,
        }
    }
}
