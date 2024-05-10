use std::borrow::Cow;
use std::io::ErrorKind::UnexpectedEof;

use std::mem::replace;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use async_http_codec::internal::buffer_decode::BufferDecodeState;
use async_http_codec::internal::buffer_write::BufferWriteState;
use async_http_codec::internal::io_future::{IoFutureState, IoFutureWithOutputState};
use async_http_codec::{BodyEncodeState, RequestHead, ResponseHead};

use futures::{AsyncRead, AsyncWrite, Future};

use http::uri::{PathAndQuery, Scheme};
use http::{HeaderMap, HeaderValue, Method, Response, Uri, Version};

use crate::{ClientConfig, Transport, TransportError};

use super::common::extract_origin;
use super::error::HttpError;
use super::response_native::ResponseBodyInner;

pub(crate) enum RequestSend<'a> {
    Start {
        body: (Pin<Box<dyn AsyncRead + Send + 'a>>, u64),
        method: Method,
        uri: Uri,
        headers: HeaderMap,
        client_config: Arc<ClientConfig>,
    },
    PendingConnect {
        body: (Pin<Box<dyn AsyncRead + Send + 'a>>, u64),
        method: Method,
        uri: Uri,
        headers: HeaderMap,
        transport: Pin<Box<dyn Future<Output = Result<Transport, TransportError>> + Send>>,
    },
    SendingHead {
        body: (Pin<Box<dyn AsyncRead + Send + 'a>>, u64),
        write_state: BufferWriteState,
        transport: Transport,
    },
    SendingBody {
        body: (Pin<Box<dyn AsyncRead + Send + 'a>>, u64),
        buffer: (Vec<u8>, usize, usize),
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
    pub fn new_with_client_config<'a>(
        request: http::Request<()>,
        body: (Pin<Box<dyn AsyncRead + Send + 'a>>, u64),
        client_config: Arc<ClientConfig>,
    ) -> RequestSend<'a> {
        let uri = request.uri().clone();
        let headers = request.headers().clone();
        let method = request.method().clone();
        RequestSend::Start {
            method,
            body,
            uri,
            headers,
            client_config,
        }
    }
    pub fn poll(&mut self, cx: &mut Context) -> Poll<Result<http::Response<ResponseBodyInner>, HttpError>> {
        loop {
            let s = replace(self, RequestSend::Finished);
            match s {
                RequestSend::Start {
                    method,
                    body,
                    uri,
                    headers,
                    client_config,
                } => {
                    let (scheme, host, port) = extract_origin(&uri, &headers)?;
                    let https = match scheme {
                        _ if scheme == Some(Scheme::HTTP) => false,
                        _ if scheme == Some(Scheme::HTTPS) => true,
                        None => true,
                        Some(scheme) => return Poll::Ready(Err(HttpError::UnexpectedScheme(scheme))),
                    };
                    let https = https.then_some(client_config);
                    let port = port.unwrap_or(match https {
                        Some(_) => 443,
                        None => 80,
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
                        let (_scheme, host, port) = extract_origin(&uri, &headers)?;
                        let mut path_and_query = uri.path_and_query().cloned().unwrap_or_else(|| PathAndQuery::from_static("/"));
                        if path_and_query == PathAndQuery::from_static("") {
                            path_and_query = PathAndQuery::from_static("/");
                        }
                        let mut head = RequestHead::new(method, Cow::Owned(path_and_query.into()), Version::HTTP_11, Cow::Borrowed(&headers));
                        if head.headers().get(http::header::HOST).is_none() {
                            let host = match port {
                                Some(port) => HeaderValue::from_str(&format!("{}:{}", host, port)).unwrap(),
                                None => HeaderValue::from_str(&host).unwrap(),
                            };
                            head.headers_mut().insert(http::header::HOST, host);
                        }
                        if head.headers().get(http::header::CONTENT_LENGTH).is_none() {
                            let length = HeaderValue::from_str(&format!("{}", body.1)).unwrap();
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
                        let write_state = BodyEncodeState::new(Some(body.1));
                        *self = RequestSend::SendingBody {
                            buffer: (vec![0u8; 1 << 14], 0, 0),
                            body,
                            write_state,
                            transport,
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
                    mut buffer,
                    mut write_state,
                    mut transport,
                    mut body,
                } => {
                    if buffer.2 == 0 {
                        if body.1 == 0 {
                            *self = RequestSend::Flushing { transport }
                        } else {
                            let max = (buffer.0.len() as u64).min(body.1) as usize;
                            match body.0.as_mut().poll_read(cx, &mut buffer.0[0..max]) {
                                Poll::Ready(Ok(0)) => return Poll::Ready(Err(HttpError::IoError(Arc::new(UnexpectedEof.into())))),
                                Poll::Ready(Ok(n)) => {
                                    buffer.2 = n;
                                    *self = RequestSend::SendingBody {
                                        buffer,
                                        write_state,
                                        transport,
                                        body,
                                    };
                                }
                                Poll::Ready(Err(err)) => return Poll::Ready(Err(HttpError::IoError(Arc::new(err)))),
                                Poll::Pending => {
                                    *self = RequestSend::SendingBody {
                                        buffer,
                                        write_state,
                                        transport,
                                        body,
                                    };
                                    return Poll::Pending;
                                }
                            }
                        }
                    } else {
                        match write_state.poll_write(&mut transport, cx, &buffer.0[buffer.1..buffer.2]) {
                            Poll::Ready(Ok(n)) => {
                                body.1 -= n as u64;
                                buffer.1 += n;
                                if buffer.1 == buffer.2 {
                                    buffer.1 = 0;
                                    buffer.2 = 0;
                                }

                                *self = RequestSend::SendingBody {
                                    write_state,
                                    transport,
                                    body,
                                    buffer,
                                }
                            }
                            Poll::Ready(Err(err)) => return Poll::Ready(Err(HttpError::IoError(Arc::new(err)))),
                            Poll::Pending => {
                                *self = RequestSend::SendingBody {
                                    write_state,
                                    transport,
                                    body,
                                    buffer,
                                };
                                return Poll::Pending;
                            }
                        }
                    }
                }
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
                        let body = ResponseBodyInner::new(transport, &head)?;
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
