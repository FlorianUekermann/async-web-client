use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures::AsyncWrite;
use gloo_net::http::{Headers, RequestCache, RequestRedirect, ResponseType};
use http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri};
use js_sys::Uint8Array;

use super::error::HttpError;
use super::response_wasm::ResponseRead;

pub struct RequestWrite {
    url: Uri,
    method: Method,
    headers: HeaderMap,
    body: Vec<u8>,
}

impl RequestWrite {
    pub fn start<T>(request: &http::Request<T>) -> Self {
        let headers = request.headers().clone();
        let url = request.uri().clone();
        let method = request.method().clone();
        Self {
            url,
            method,
            headers,
            body: Vec::new(),
        }
    }
    pub async fn response(self) -> Result<(http::Response<()>, ResponseRead), HttpError> {
        let RequestWrite {
            url,
            method,
            headers,
            body,
        } = self;
        let mut req = gloo_net::http::Request::new(&url.to_string())
            .method(Self::method(method)?)
            .cache(RequestCache::NoStore)
            .redirect(RequestRedirect::Manual)
            .headers(Self::headers(headers)?);
        if body.len() > 0 {
            let arr = Uint8Array::new_with_length(body.len().try_into().unwrap());
            arr.copy_from(&body);
            req = req.body(Some(&arr));
        }
        let resp = req.send().await?;
        match resp.type_() {
            ResponseType::Cors | ResponseType::Basic => {}
            ResponseType::Error => return Err(HttpError::NetworkError),
            ResponseType::Opaqueredirect => return Err(HttpError::Redirect),
            _ => unreachable!(),
        }

        let mut response = http::Response::new(());
        *response.status_mut() = StatusCode::from_u16(resp.status()).unwrap();
        *response.version_mut() = http::Version::HTTP_09;
        let headers = response.headers_mut();
        for (name, value) in resp.headers().entries() {
            let name = HeaderName::from_bytes(name.as_bytes()).unwrap();
            let value = HeaderValue::from_str(&value).unwrap();
            headers.append(name, value);
        }
        Ok((response, ResponseRead::new(resp.body())))
    }
    fn headers(map: HeaderMap) -> Result<Headers, HttpError> {
        let headers = Headers::new();
        let mut last_name = HeaderName::from_static("");
        for (name, value) in map {
            let name = name.unwrap_or(last_name);
            last_name = name.clone();
            if let Ok(value) = value.to_str() {
                headers.append(name.as_str(), value)
            } else {
                return Err(HttpError::InvalidHeaderValue(value));
            }
        }

        Ok(headers)
    }
    fn method(method: Method) -> Result<gloo_net::http::Method, HttpError> {
        use gloo_net::http::Method::*;
        match method.as_str() {
            "OPTIONS" => Ok(OPTIONS),
            "GET" => Ok(GET),
            "POST" => Ok(POST),
            "PUT" => Ok(PUT),
            "DELETE" => Ok(DELETE),
            "HEAD" => Ok(DELETE),
            "TRACE" => Ok(TRACE),
            "CONNECT" => Ok(CONNECT),
            "PATCH" => Ok(PATCH),
            _ => Err(HttpError::InvalidMethod(method)),
        }
    }
}

impl AsyncWrite for RequestWrite {
    fn poll_write(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.body.extend_from_slice(buf);
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}
