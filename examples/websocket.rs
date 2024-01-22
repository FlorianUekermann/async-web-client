use async_web_client::WsConnection;
use futures::{AsyncReadExt, AsyncWriteExt, StreamExt};

fn main() {
    env_logger::init();
    smol::block_on(run());
}

async fn run() {
    if let Err(err) = ws().await {
        log::error!("{:?}", err);
    } else {
        log::info!("no errors");
    }
}

async fn ws() -> Result<(), Box<dyn std::error::Error>> {
    let mut ws = WsConnection::connect_with_uri("https://ws.postman-echo.com/raw").await?;
    let mut writer = ws.send_text().await.ok_or("could not send")?;
    writer.write_all(b"hello ws").await?;
    writer.close().await?;
    let mut msg = ws.next().await.ok_or("stream ended")?;
    let mut data = Vec::new();
    msg.read_to_end(&mut data).await?;
    log::info!("received {:?} message: {:?}", msg.kind(), String::from_utf8_lossy(&data));
    Ok(())
}
