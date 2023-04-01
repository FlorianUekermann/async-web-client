use self::error::HttpError;

#[cfg(target_arch = "wasm32")]
mod request_wasm;
#[cfg(target_arch = "wasm32")]
type RequestStreamInner = request_wasm::RequestWrite;

#[cfg(target_arch = "wasm32")]
mod response_wasm;
#[cfg(target_arch = "wasm32")]
type ResponseReadInner = response_wasm::ResponseRead;

mod error;

pub fn start_request<T>(request: &http::Request<T>) -> RequestWrite {
    RequestWrite::start(&request)
}

pub struct RequestWrite {
    inner: RequestStreamInner,
}

impl RequestWrite {
    pub fn start<T>(request: &http::Request<T>) -> Self {
        let inner = RequestStreamInner::start(request);
        Self { inner }
    }
    pub async fn response(self) -> Result<(http::Response<()>, ResponseRead), HttpError> {
        let (response, inner) = self.inner.response().await?;
        Ok((response, ResponseRead { inner }))
    }
}

pub struct ResponseRead {
    inner: ResponseReadInner,
}
