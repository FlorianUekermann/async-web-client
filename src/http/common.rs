use http::{
    uri::{Authority, Scheme},
    HeaderMap, Uri,
};

use super::error::HttpError;

pub(crate) fn extract_origin(uri: &Uri, headers: &HeaderMap) -> Result<(Option<Scheme>, String, Option<u16>), HttpError> {
    if let Some(auth) = uri.authority() {
        return Ok((uri.scheme().cloned(), auth.host().to_string(), auth.port_u16()));
    }
    if let Some(header) = headers.get(http::header::HOST) {
        if let Some(auth) = Authority::try_from(header.as_bytes()).ok() {
            if auth.as_str().len() == auth.host().len() + 1usize + auth.port().map(|p| p.as_str().len()).unwrap_or(0) {
                return Ok((None, auth.host().to_string(), auth.port_u16()));
            }
        }
    }
    return Err(HttpError::MissingHost);
}
