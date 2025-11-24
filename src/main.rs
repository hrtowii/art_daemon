use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio::fs;
use tokio::task::JoinHandle;
use tokio::time::Duration;
use tokio::{task, time};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use anyhow::{Context, Result};

const ID: &str = "413331641109446656";
const LANYARD: &str = "wss://api.lanyard.rest/socket";

async fn ping<S>(write: &mut S)
where
    S: SinkExt<Message> + Unpin,
{
    let heartbeat_json = serde_json::json!({ "op": 3 }).to_string();
    let _ = write.send(Message::Text(heartbeat_json.into())).await;
}

#[tokio::main]
async fn main() -> Result<()> {
    let (ws_stream, _response) = connect_async(LANYARD).await.context("connection failed")?;
    let (mut write, mut read) = ws_stream.split();

    let init = read.next()
        .await.context("could not read init message")??.into_text()?;
    let a = init.as_str();
    let init_json: serde_json::Value = serde_json::from_str(a)?;
    let mut heartbeat_interval = 0;
    if init_json["op"] == 1 {
        heartbeat_interval = init_json["d"]["heartbeat_interval"]
            .as_u64()
            .context("invalid heartbeat interval")?;
    }
    let subscribe = serde_json::json!({"op": 2, "d": {"subscribe_to_ids": [ID]}}).to_string();
    write.send(subscribe.into()).await.context("err connecting")?;

    let heartbeat = task::spawn(async move {
        let mut interval = time::interval(Duration::from_millis(heartbeat_interval));
        loop {
            interval.tick().await;
            println!("pinging");
            ping(&mut write).await;
        }
    });
    let polling: JoinHandle<Result<()>> = task::spawn(async move {
        while let Some(msg) = read.next().await {
            let txt = msg?.into_text()?;
            let json: Value = serde_json::from_str(&txt)?;
            if json["op"] == 0
                && let Some(spotify) = json["d"]["spotify"].as_object()
                && let (Some(art_url), Some(artist), Some(title)) = (
                    spotify.get("album_art_url").and_then(Value::as_str),
                    spotify.get("artist").and_then(Value::as_str),
                    spotify.get("song").and_then(Value::as_str),
                )
            {
                // println!("{}, {}, {}", art_url, artist, title);
                fs::create_dir_all("./album_art").await?;
                let resp = reqwest::get(art_url).await.context("err downloading")?;
                let bytes = resp.bytes().await.context("err reading bytes")?;
                fs::write("./album_art/current.jpg", bytes).await?;
                fs::write("./output.txt", format!("{}\n{}", artist, title)).await?;
            }
        }
        Ok(())
    });

    tokio::try_join!(
        heartbeat,
        polling,
    )?;
    Ok(())
}
