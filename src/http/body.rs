use futures::future::Either;
use futures::io::{empty, Cursor, Empty};
use futures::AsyncRead;

pub trait IntoRequestBody {
    type RequestBody: AsyncRead;
    fn into_request_body(self) -> (Self::RequestBody, u64);
}

pub trait NotEmptyBody: IntoRequestBody {}

impl<'a, T: AsRef<[u8]>> NotEmptyBody for &'a T {}
impl NotEmptyBody for Vec<u8> {}
impl NotEmptyBody for String {}
impl<T: NotEmptyBody> NotEmptyBody for Option<T> {}

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
