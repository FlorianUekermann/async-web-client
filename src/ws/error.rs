use std::sync::Arc;

use crate::HttpError;
use http::uri::InvalidUri;
use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum WsConnectError {
    #[error("invalid websocket upgrade request")]
    InvalidUpgradeRequest,
    #[error("invalid websocket upgrade response")]
    InvalidUpgradeResponse(Arc<http::Response<Box<dyn std::fmt::Debug>>>),
    #[error("invalid uri: {0:?}")]
    InvalidUrl(Arc<InvalidUri>),
    #[error("websocket upgrade request error: {0:?}")]
    UpgradeRequestHttpError(#[from] HttpError),
    #[error("nknown gloo error: {0:?}")]
    Other(()),
}

impl From<InvalidUri> for WsConnectError {
    fn from(value: InvalidUri) -> Self {
        WsConnectError::InvalidUrl(Arc::new(value))
    }
}
