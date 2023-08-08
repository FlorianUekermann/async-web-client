use std::{io, sync::Arc};

use http::{uri::Scheme, HeaderValue, Method};
use thiserror::Error;

use crate::TransportError;

#[derive(Error, Debug, Clone)]
pub enum HttpError {
    #[error("invalid header value: {0:?}")]
    InvalidHeaderValue(HeaderValue),
    #[error("invalid method: {0}")]
    InvalidMethod(Method),
    #[cfg(target_arch = "wasm32")]
    #[error("network error")]
    NetworkError,
    #[error("redirect")]
    Redirect,
    #[cfg(not(target_arch = "wasm32"))]
    #[error("missing host in URI or host header")]
    MissingHost,
    #[cfg(not(target_arch = "wasm32"))]
    #[error("unexpected URI scheme: {0:?}")]
    UnexpectedScheme(Scheme),
    #[cfg(not(target_arch = "wasm32"))]
    #[error("unsupported transfer encoding: {0:?}")]
    UnsupportedTransferEncoding(HeaderValue),
    #[cfg(not(target_arch = "wasm32"))]
    #[error("connect error: {0:?}")]
    ConnectError(TransportError),
    #[cfg(not(target_arch = "wasm32"))]
    #[error("io error: {0:?}")]
    IoError(Arc<io::Error>),
    #[cfg(target_arch = "wasm32")]
    #[error("invalid url error: {0}")]
    InvalidUrl(Arc<gloo_net::Error>),
    #[cfg(target_arch = "wasm32")]
    #[error("unknown gloo error: {0}")]
    Other(std::sync::Arc<gloo_net::Error>),
}

#[cfg(target_arch = "wasm32")]
impl From<gloo_net::Error> for HttpError {
    fn from(value: gloo_net::Error) -> Self {
        Self::Other(std::sync::Arc::new(value))
    }
}

impl From<HttpError> for io::Error {
    fn from(value: HttpError) -> Self {
        let kind = match &value {
            #[cfg(target_arch = "wasm32")]
            HttpError::InvalidUrl(_) => io::ErrorKind::Unsupported,
            #[cfg(target_arch = "wasm32")]
            HttpError::NetworkError => io::ErrorKind::NotConnected,
            HttpError::InvalidHeaderValue(_) => io::ErrorKind::InvalidData,
            HttpError::InvalidMethod(_) => io::ErrorKind::InvalidData,
            HttpError::Redirect => io::ErrorKind::Unsupported,
            #[cfg(target_arch = "wasm32")]
            HttpError::Other(_) => io::ErrorKind::Other,
            #[cfg(not(target_arch = "wasm32"))]
            HttpError::MissingHost => io::ErrorKind::Unsupported,
            #[cfg(not(target_arch = "wasm32"))]
            HttpError::UnexpectedScheme(_) => io::ErrorKind::Unsupported,
            #[cfg(not(target_arch = "wasm32"))]
            HttpError::ConnectError(err) => match err {
                TransportError::InvalidDnsName(_) => io::ErrorKind::InvalidData,
                TransportError::TcpConnect(err) => err.kind(),
                TransportError::TlsConnect(err) => err.kind(),
            },
            #[cfg(not(target_arch = "wasm32"))]
            HttpError::IoError(err) => err.kind(),
            #[cfg(not(target_arch = "wasm32"))]
            HttpError::UnsupportedTransferEncoding(_) => io::ErrorKind::Unsupported,
        };
        io::Error::new(kind, value)
    }
}
