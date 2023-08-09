use std::{
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use async_ws::{
    connection::WsConfig,
    http::{check_upgrade_response, is_upgrade_request, upgrade_request},
};
use futures::{AsyncReadExt, Stream};
use http::Response;

use crate::{RequestSend, Transport};

mod error;

use error::*;

pub type WsMessageKind = async_ws::message::WsMessageKind;
pub type WsSend = async_ws::connection::WsSend<Transport>;
pub type WsConnectionError = async_ws::connection::WsConnectionError;
pub type WsMessageReader = async_ws::connection::WsMessageReader<Transport>;
pub type WsMessageWriter = async_ws::connection::WsMessageWriter<Transport>;

pub struct WsConnection {
    inner: async_ws::connection::WsConnection<Transport>,
}

impl WsConnection {
    pub async fn connect_with_uri<T>(uri: T) -> Result<Self, WsConnectError>
    where
        http::Uri: TryFrom<T>,
        <http::Uri as TryFrom<T>>::Error: Into<http::uri::InvalidUri>,
    {
        let uri: http::Uri = uri.try_into().map_err(Into::into)?;
        let mut request = Self::connect_request_builder().body("").unwrap();
        *request.uri_mut() = uri;
        Self::connect(&request).await
    }
    pub async fn connect(request: &http::Request<impl AsRef<[u8]>>) -> Result<Self, WsConnectError> {
        if !is_upgrade_request(request) {
            return Err(WsConnectError::InvalidUpgradeRequest);
        }
        let response = RequestSend::new(request).await?;
        if !check_upgrade_response(request, &response) {
            let (head, body_reader) = response.into_parts();
            let mut buf = Vec::new();
            let result = body_reader.take(1 << 14).read_to_end(&mut buf).await;
            let result: Box<dyn std::fmt::Debug + Send + Sync> = match String::from_utf8(buf) {
                Ok(str) => Box::new(result.map(move |_| str)),
                Err(err) => Box::new(result.map(move |_| err.into_bytes())),
            };
            let response = Response::from_parts(head, result);
            return Err(WsConnectError::InvalidUpgradeResponse(response.into()));
        }
        let transport = response.into_body().into_inner()?.1;
        let inner = async_ws::connection::WsConnection::with_config(transport, WsConfig::client());
        Ok(Self { inner })
    }
    pub fn connect_request_builder() -> http::request::Builder {
        upgrade_request()
    }
    pub fn send(&self, kind: WsMessageKind) -> WsSend {
        self.inner.send(kind)
    }
    pub fn send_text(&self) -> WsSend {
        self.send(WsMessageKind::Text)
    }
    pub fn send_binary(&self) -> WsSend {
        self.send(WsMessageKind::Binary)
    }
    pub fn err(&self) -> Option<Arc<WsConnectionError>> {
        self.inner.err()
    }
}

impl Stream for WsConnection {
    type Item = WsMessageReader;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.get_mut().inner).poll_next(cx)
    }
}
