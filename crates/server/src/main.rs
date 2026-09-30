//! Serwer-przekaźnik tur (lockstep). Nie symuluje gry – zbiera intencje, stempluje je
//! ID gracza, co 100 ms rozsyła numerowaną turę i porównuje hashe stanu od klientów.
//!
//!   cargo run -p game-server [--features debug] -- [--dev] [--port 3000]

mod room;

use std::net::SocketAddr;

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use game_core::protocol::ClientMsg;
use tokio::sync::{mpsc, oneshot};
use tower_http::{
    compression::Compression,
    services::{ServeDir, ServeFile},
};

use room::{RoomCmd, RoomHandle};

/// Zbudowany frontend (`ng build`), serwowany z tej samej binarki.
const STATIC_DIR: &str = "web/dist/web/browser";
const MAX_MESSAGE_BYTES: usize = 16 * 1024;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dev = args.iter().any(|a| a == "--dev");
    let port: u16 = args
        .iter()
        .position(|a| a == "--port")
        .and_then(|i| args.get(i + 1))
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);

    let room = room::spawn(dev);
    let app = Router::new()
        .route("/ws", get(ws_handler))
        // Pliki frontendu kompresowane w locie (brotli albo gzip, zależnie od przeglądarki) –
        // na wolnych łączach wczytanie skraca się mniej więcej o połowę.
        .fallback_service(Compression::new(
            ServeDir::new(STATIC_DIR).not_found_service(ServeFile::new(format!("{STATIC_DIR}/index.html"))),
        ))
        .with_state(room);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    println!("serwer na http://{addr} (dev: {dev})");
    let listener = tokio::net::TcpListener::bind(addr).await.expect("bind");
    axum::serve(listener, app).await.expect("server");
}

async fn ws_handler(ws: WebSocketUpgrade, State(room): State<RoomHandle>) -> impl IntoResponse {
    ws.max_message_size(MAX_MESSAGE_BYTES).on_upgrade(move |socket| client(socket, room))
}

/// Jedno połączenie = jeden task. Pisanie do gniazda robi osobny task,
/// żeby wolny klient nie blokował pokoju.
async fn client(socket: WebSocket, room: RoomHandle) {
    let (mut sink, mut stream) = socket.split();
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<String>();
    let (reply_tx, reply_rx) = oneshot::channel();
    if room.send(RoomCmd::Join { out: out_tx, reply: reply_tx }).is_err() {
        return;
    }
    let Ok(player) = reply_rx.await else { return };

    let writer = tokio::spawn(async move {
        while let Some(text) = out_rx.recv().await {
            if sink.send(Message::Text(text.into())).await.is_err() {
                break;
            }
        }
    });

    while let Some(Ok(msg)) = stream.next().await {
        match msg {
            Message::Text(text) => match serde_json::from_str::<ClientMsg>(text.as_str()) {
                Ok(msg) => {
                    let _ = room.send(RoomCmd::Client { player, msg });
                }
                Err(e) => eprintln!("gracz {player}: niepoprawna wiadomość: {e}"),
            },
            Message::Close(_) => break,
            _ => {}
        }
    }
    let _ = room.send(RoomCmd::Leave { player });
    writer.abort();
}
