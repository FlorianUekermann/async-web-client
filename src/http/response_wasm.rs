use wasm_bindgen::JsCast;
use web_sys::{ReadableStream, ReadableStreamDefaultReader};

pub struct ResponseRead {
    body: Option<(ReadableStreamDefaultReader, ReadableStream)>,
}

impl ResponseRead {
    pub(crate) fn new(stream: Option<ReadableStream>) -> Self {
        let body = stream.map(|stream| (stream.get_reader().dyn_into().unwrap(), stream));
        Self { body }
    }
}
