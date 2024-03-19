use futures::future::Either;
use futures::io::{empty, Cursor, Empty};
use futures::AsyncRead;

pub trait IntoRequestBody {
    type RequestBody: AsyncRead;
    fn into_request_body(self) -> (Self::RequestBody, u64);
}

pub trait IntoNonUnitRequestBody: IntoRequestBody {}

impl<'a, T: AsRef<[u8]>> IntoNonUnitRequestBody for &'a T {}
impl<'a, T: AsyncRead> IntoNonUnitRequestBody for (T, u64) {}
impl IntoNonUnitRequestBody for Vec<u8> {}
impl IntoNonUnitRequestBody for String {}
impl<T: IntoNonUnitRequestBody> IntoNonUnitRequestBody for Option<T> {}

impl<'a, T: AsyncRead> IntoRequestBody for (T, u64) {
    type RequestBody = T;
    fn into_request_body(self) -> (Self::RequestBody, u64) {
        (self.0, self.1)
    }
}

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
    fn test_send_with_async_read_body() {
        Request::post("http://postman-echo.com/post").body((futures::io::Cursor::new(&[3u8]),1)).unwrap().send();
        Request::post("http://postman-echo.com/post").body(()).unwrap().send((futures::io::Cursor::new(&[3u8]),1));
    }
    #[test]
    fn test_send_with_as_ref() {
        Request::post("http://postman-echo.com/post").body(()).unwrap().send(&[3u8]);
        Request::post("http://postman-echo.com/post").body(&[3u8]).unwrap().send();
    }
    #[test]
    fn test_send_with_vec() {
        Request::post("http://postman-echo.com/post").body(()).unwrap().send(vec![1, 2, 3]);
        Request::post("http://postman-echo.com/post").body(vec![1, 2, 3]).unwrap().send();
    }
    #[test]
    fn test_send_with_string() {
        Request::post("http://postman-echo.com/post").body(()).unwrap().send("test".to_string());
        Request::post("http://postman-echo.com/post").body("test".to_string()).unwrap().send();
    }
    #[test]
    fn test_send_with_option() {
        Request::post("http://postman-echo.com/post").body(()).unwrap().send(Some(&[1, 2, 3]));
        Request::post("http://postman-echo.com/post").body(Some(&[1, 2, 3])).unwrap().send();
    }
}
