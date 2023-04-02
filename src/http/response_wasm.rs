use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};

use futures::{ready, AsyncRead, FutureExt};
use js_sys::{Reflect, Uint8Array};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{ReadableStream, ReadableStreamDefaultReader};

pub struct ResponseRead {
    body: Option<ReadableStreamDefaultReader>,
    future: Option<JsFuture>,
    chunk: Option<(usize, Vec<u8>)>,
}

impl ResponseRead {
    pub(crate) fn new(stream: Option<ReadableStream>) -> Self {
        let body = stream.map(|stream| stream.get_reader().dyn_into().unwrap());
        Self {
            body,
            future: None,
            chunk: None,
        }
    }
}

impl AsyncRead for ResponseRead {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        loop {
            if let Some(chunk) = self.chunk.as_mut() {
                let start = chunk.0;
                let n = buf.len().min(chunk.1.len() - start);
                let end = start + n;
                buf[0..n].copy_from_slice(&chunk.1[start..end]);
                chunk.0 = end;
                if chunk.0 == chunk.1.len() {
                    self.chunk = None;
                }
                return Poll::Ready(Ok(n));
            }
            if let Some(fut) = self.future.as_mut() {
                let res = ready!(fut.poll_unpin(cx));
                self.future.take();
                match res {
                    Ok(ok) => {
                        let done: bool = Reflect::get(&ok, &JsValue::from_str("done"))
                            .unwrap()
                            .as_bool()
                            .unwrap();
                        if done {
                            self.body = None;
                            return Poll::Ready(Ok(0));
                        }
                        let value: Uint8Array = Reflect::get(&ok, &JsValue::from_str("value"))
                            .unwrap()
                            .dyn_into()
                            .unwrap();
                        self.chunk = Some((0, value.to_vec()));
                    }
                    Err(err) => todo!("{:?}", err),
                }
            }
            if let Some(body) = self.body.as_mut() {
                self.future = Some(body.read().into());
                continue;
            }
            return Poll::Ready(Ok(0));
        }
    }
}
