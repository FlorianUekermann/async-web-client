use async_web_client::RequestSend;
use futures::AsyncReadExt;
use http::Request;

fn main() {
    env_logger::init();
    smol::block_on(run());
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
