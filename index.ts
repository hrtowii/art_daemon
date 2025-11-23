const WS_URL = "wss://api.lanyard.rest/socket";
const DISCORD_ID = "413331641109446656";

let ws;
let heartbeat;
let lastArtUrl = null;
import { write } from "bun";
async function downloadImage(url, filepath) {
  const res = await fetch(url);
  if (!res.ok) return;
  const buf = await res.arrayBuffer();
  await Bun.write(filepath, Buffer.from(buf));
}

function setupWebSocket() {
  ws = new WebSocket(WS_URL);

  ws.onopen = () => {
    console.log("[music] connected to lanyard");
  };

  ws.onmessage = async (msg) => {
    const payload = JSON.parse(msg.data);

    if (payload.op === 1) {
      const interval = payload.d.heartbeat_interval;
      console.log("[music] received hello, heartbeat =", interval, "ms");

      if (heartbeat) clearInterval(heartbeat);

      heartbeat = setInterval(() => {
        try {
          ws.send(JSON.stringify({ op: 3 }));
        } catch {
          /* socket probably dead */
        }
      }, interval);

      ws.send(
        JSON.stringify({
          op: 2,
          d: { subscribe_to_ids: [DISCORD_ID] }
        })
      );

      return;
    }

    if (payload.op === 0) {
      const data = payload.d;

      if (!data.spotify) {
        lastArtUrl = null;
        return;
      }

      const art = data.spotify.album_art_url;
      const title = data.spotify.song;
      const artist = data.spotify.artist;

      if (art === lastArtUrl) return;
      lastArtUrl = art;

      console.log(`[music] downloading: ${title} — ${artist}`);

      const filename = "./album_art/current.jpg";
      console.log(`[music] → ${filename}`);

      await downloadImage(art, filename);
      await write("./output.txt", `${artist}\n${title}`)
    }
  };

  ws.onclose = () => {
    console.log("[music] ws closed, reconnecting in 5s...");

    if (heartbeat) clearInterval(heartbeat);

    setTimeout(() => {
      setupWebSocket();
    }, 5000);
  };

  ws.onerror = (err) => {
    console.log("[music] websocket error:", err);
    ws.close(); // triggers reconnect
  };
}

setupWebSocket();

