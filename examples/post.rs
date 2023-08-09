use async_web_client::RequestSend;
use futures::AsyncReadExt;
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
    if let Err(err) = post().await {
        log::error!("{:?}", err);
    } else {
        log::info!("no errors");
    }
}

async fn post() -> Result<(), Box<dyn std::error::Error>> {
    let request = Request::post("http://postman-echo.com/post").body(())?;
    let request = request.map(|_| &b"hello post"[..]);

    let response = RequestSend::new(&request).await?;
    let (parts, mut body_reader) = response.into_parts();
    log::info!("response: {parts:?}");
    let mut body = String::new();
    body_reader.read_to_string(&mut body).await?;
    log::info!("response body: {body}");
    Ok(())
}
