use async_web_client::http::RequestSend;
use futures::{AsyncReadExt, AsyncWriteExt};
use http::{header::HOST, Request};

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
    if let Err(err) = post().await {
        log::error!("{:?}", err);
    }
}

async fn post() -> Result<(), Box<dyn std::error::Error>> {
    //    let request = Request::post("/anything").header(HOST, "httpbin.org:80").body(())?;
    let request = Request::post("http://postman-echo.com/post").body(())?;
    let request = Request::post("http://httpbin.org/anything").body(())?;
    let request = request.map(|_| &b"hello post"[..]);

    let response = RequestSend::new(&request).await?;
    let (parts, mut body_reader) = response.into_parts();
    dbg!();
    log::info!("response: {parts:?}");
    dbg!();
    let mut body = String::new();
    dbg!();
    body_reader.read_to_string(&mut body).await?;
    dbg!();
    log::info!("response body: {body}");
    Ok(())
}
