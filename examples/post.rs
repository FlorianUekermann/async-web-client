use async_web_client::http::start_request;
use futures::{AsyncReadExt, AsyncWriteExt};
use http::Request;

fn main() {
    #[cfg(target_arch = "wasm32")]
    {
        std::panic::set_hook(Box::new(console_error_panic_hook::hook));
        wasm_logger::init(wasm_logger::Config::new(log::Level::Info));
        wasm_bindgen_futures::spawn_local(run());
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        env_logger::init();
        smol::block_on(run());
    }
}

async fn run() {
    let request = Request::post("https://httpbin.org/anything")
        .body(())
        .unwrap();
    let mut req_writer = start_request(&request);
    req_writer.write_all(b"hello post").await.unwrap();

    let (response, mut resp_reader) = req_writer.response().await.unwrap();
    log::info!("response: {response:?}");
    let mut body = String::new();
    resp_reader.read_to_string(&mut body).await.unwrap();
    log::info!("response body: {body}");
}
