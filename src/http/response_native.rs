use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};

use futures::AsyncRead;

pub struct ResponseRead {}

impl ResponseRead {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

impl AsyncRead for ResponseRead {
    fn poll_read(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        _buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        todo!()
    }
}
