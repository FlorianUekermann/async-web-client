use futures::future::Either;
use futures::io::{empty, Cursor, Empty};
use futures::AsyncRead;

pub trait IntoRequestBody {
    type RequestBody: AsyncRead;
    fn into_request_body(self) -> (Self::RequestBody, u64);
}

pub trait IntoNonUnitRequestBody: IntoRequestBody {}

impl<'a, T: AsRef<[u8]>> IntoNonUnitRequestBody for &'a T {}
impl IntoNonUnitRequestBody for Vec<u8> {}
impl IntoNonUnitRequestBody for String {}
impl<T: IntoNonUnitRequestBody> IntoNonUnitRequestBody for Option<T> {}

impl<'a, T: AsRef<[u8]>> IntoRequestBody for &'a T {
    type RequestBody = &'a [u8];
    fn into_request_body(self) -> (Self::RequestBody, u64) {
        let slice = self.as_ref();
        (slice, slice.len() as u64)
    }
}

impl IntoRequestBody for Vec<u8> {
    type RequestBody = Cursor<Vec<u8>>;
    fn into_request_body(self) -> (Self::RequestBody, u64) {
        let len = self.len() as u64;
        (Cursor::new(self), len)
    }
}

impl IntoRequestBody for String {
    type RequestBody = Cursor<String>;
    fn into_request_body(self) -> (Self::RequestBody, u64) {
        let len = self.len() as u64;
        (Cursor::new(self), len)
    }
}

impl IntoRequestBody for () {
    type RequestBody = Empty;
    fn into_request_body(self) -> (Self::RequestBody, u64) {
        (empty(), 0)
    }
}

impl<T: IntoRequestBody> IntoRequestBody for Option<T> {
    type RequestBody = Either<Empty, T::RequestBody>;
    fn into_request_body(self) -> (Self::RequestBody, u64) {
        match self {
            None => (Either::Left(empty()), 0),
            Some(v) => {
                let (request_body, len) = v.into_request_body();
                (Either::Right(request_body), len)
            }
        }
    }
}

#[cfg(test)]
mod tests {

    use crate::prelude::*;
    use http::Request;
    
    #[test]
    fn test_without_body_send() {
	let request_empty_body = Request::post("http://postman-echo.com/post").body(()).unwrap();

	request_empty_body.send(&[3u8]);
	request_empty_body.send(());
    }

    #[test]
    fn test_with_body_send() {
	let request_with_body = Request::post("http://postman-echo.com/post").body(&[4u8]).unwrap();
	request_with_body.send();
    }
}
