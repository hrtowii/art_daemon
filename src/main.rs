use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio::fs;
use tokio::time::Duration;
use tokio::{task, time};
use tokio_tungstenite::{connect_async, tungstenite::Message};
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
async fn main() {
    let (ws_stream, response) = connect_async(LANYARD).await.expect("connection failed");
    let (mut write, mut read) = ws_stream.split();

    let init = read.next().await.unwrap().unwrap().into_text().unwrap();
    let a = init.as_str();
    let init_json: serde_json::Value = serde_json::from_str(a).unwrap();
    let mut heartbeat_interval = 0;
    if init_json["op"] == 1 {
        heartbeat_interval = init_json["d"]["heartbeat_interval"]
            .as_number()
            .unwrap()
            .as_u64()
            .unwrap();
    }
    let subscribe = serde_json::json!({"op": 2, "d": {"subscribe_to_ids": [ID]}}).to_string();
    if let Err(e) = write.send(subscribe.into()).await {
        eprintln!("err connecting: {}", e);
    }

    let heartbeat = task::spawn(async move {
        let mut interval = time::interval(Duration::from_millis(heartbeat_interval));
        loop {
            interval.tick().await;
            println!("pinging");
            ping(&mut write).await;
        }
    });
    //     "spotify": Object {
    //     "album": String("Pinkerton - Deluxe Edition"),
    //     "album_art_url": String("https://i.scdn.co/image/ab67616d0000b273b34530ac4a80275e3ff2faab"),
    //     "artist": String("Weezer"),
    //     "song": String("I Just Threw Out The Love Of My Dreams"),
    //     "timestamps": Object {
    //         "end": Number(1764006422968),
    //         "start": Number(1764006265008),
    //     },
    //     "track_id": String("35SRuRfp5BvD1yArmXKNHO"),
    // },
    let polling = task::spawn(async move {
        while let Some(msg) = read.next().await {
            if let Ok(Message::Text(txt)) = msg {
                let json: Value = serde_json::from_str(&txt).unwrap();
                if json["op"] == 0
                    && let Some(spotify) = json["d"]["spotify"].as_object()
                        && let (Some(art_url), Some(artist), Some(title)) = (
                            spotify.get("album_art_url").and_then(|v| v.as_str()),
                            spotify.get("artist").and_then(|v| v.as_str()),
                            spotify.get("song").and_then(|v| v.as_str()),
                        ) {
                            // println!("{}, {}, {}", art_url, artist, title);
                            fs::create_dir_all("./album_art").await.unwrap();
                            match reqwest::get(art_url).await {
                                Ok(resp) => {
                                    let bytes = resp.bytes().await.unwrap();
                                    fs::write("./album_art/current.jpg", bytes).await.unwrap();
                                }
                                Err(_) => println!("err downloading"),
                            }
                            fs::write("./output.txt", format!("{}\n{}", artist, title))
                                .await
                                .unwrap();
                        }
            }
        }
    });

    tokio::join!(
        async {
            heartbeat.await.unwrap();
        },
        async {
            polling.await.unwrap();
        }
    );
}
