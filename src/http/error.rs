use std::{io, sync::Arc};

use http::{uri::Scheme, HeaderValue, Method};
use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum HttpError {
    #[error("invalid header value: {0:?}")]
    InvalidHeaderValue(HeaderValue),
    #[error("invalid method: {0}")]
    InvalidMethod(Method),
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
    #[error("unknown gloo error: {0}")]
    Other(std::sync::Arc<gloo_net::Error>),
}

#[cfg(target_arch = "wasm32")]
impl From<gloo_net::Error> for HttpError {
    fn from(value: gloo_net::Error) -> Self {
        Self::Other(std::sync::Arc::new(value))
    }
}
