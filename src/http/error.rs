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
}

impl From<HttpError> for io::Error {
    fn from(value: HttpError) -> Self {
        let kind = match &value {
            HttpError::InvalidHeaderValue(_) => io::ErrorKind::InvalidData,
            HttpError::InvalidMethod(_) => io::ErrorKind::InvalidData,
            HttpError::Redirect => io::ErrorKind::Unsupported,
            HttpError::MissingHost => io::ErrorKind::Unsupported,
            HttpError::UnexpectedScheme(_) => io::ErrorKind::Unsupported,
            HttpError::ConnectError(err) => match err {
                TransportError::InvalidDnsName(_) => io::ErrorKind::InvalidData,
                TransportError::TcpConnect(err) => err.kind(),
                TransportError::TlsConnect(err) => err.kind(),
            },
            HttpError::IoError(err) => err.kind(),
            HttpError::UnsupportedTransferEncoding(_) => io::ErrorKind::Unsupported,
        };
        io::Error::new(kind, value)
    }
}
