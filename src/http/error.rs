use std::sync::Arc;

use http::{HeaderValue, Method};
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
    #[error("unknown gloo error: {0}")]
    Other(Arc<gloo_net::Error>),
}

impl From<gloo_net::Error> for HttpError {
    fn from(value: gloo_net::Error) -> Self {
        Self::Other(Arc::new(value))
    }
}
