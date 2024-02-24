use async_web_client::{prelude::*, RequestWithoutBodyExt};
use http::Request;

fn main() {
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
    let request = Request::post("http://postman-echo.com/post").body(())?;

    let mut response = request.send_with_body(&"hello post").await?;
    println!("response head: {response:#?}");
    let body = response.body_string(Some(1024)).await?;
    println!("response body: {body}");
    Ok(())
}
