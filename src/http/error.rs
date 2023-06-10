use std::{io, sync::Arc};

use http::{uri::Scheme, HeaderValue, Method};
use thiserror::Error;

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
    #[error("relative URI")]
    RelativeUri,
    #[cfg(not(target_arch = "wasm32"))]
    #[error("unexpected URI scheme: {0:?}")]
    UnexpectedScheme(Scheme),
    #[cfg(not(target_arch = "wasm32"))]
    #[error("connect error: {0:?}")]
    ConnectError(Arc<io::Error>),
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
            HttpError::InvalidHeaderValue(_) => io::ErrorKind::InvalidData,
            HttpError::InvalidMethod(_) => io::ErrorKind::InvalidData,
            HttpError::Redirect => io::ErrorKind::Unsupported,
            HttpError::RelativeUri => io::ErrorKind::Unsupported,
            HttpError::UnexpectedScheme(_) => io::ErrorKind::Unsupported,
            HttpError::ConnectError(err) => err.kind(),
            HttpError::IoError(err) => err.kind(),
        };
        io::Error::new(kind, value)
    }
}
