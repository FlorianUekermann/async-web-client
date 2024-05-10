use async_web_client::prelude::*;
use http::Request;

fn main() {
    env_logger::init();
    smol::block_on(run());
}

async fn run() {
    if let Err(err) = post().await {
        println!("error {:?}", err);
    } else {
        println!("no errors");
    }
}

async fn post() -> Result<(), Box<dyn std::error::Error>> {
    let request = Request::post("https://postman-echo.com/post").body(())?;

    let mut response = request.send(&"hello post").await?;
    println!("response head: {response:#?}");
    let body = response.body_string(Some(1024)).await?;
    println!("response body: {body}");
    Ok(())
}
