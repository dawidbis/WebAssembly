//! Serwer-przekaźnik tur (lockstep). Nie symuluje gry – zbiera intencje, stempluje je
//! ID gracza, co 100 ms rozsyła numerowaną turę i porównuje hashe stanu od klientów.
//!
//!   cargo run -p game-server [--features debug] -- [--dev] [--port 3000] [--seed 7] [--params p.json]
//!                                                  [--static-dir DIR] [--ticket-key ticket.pub.pem]
//!
//! `--params` – JSON z polami `MapGenParams` (jak w CLI `mapgen`), `--seed` – seed mapy (nadpisuje ten z pliku).
//!
//! Dwa tryby (docs/adr/0004):
//! - **otwarty** (bez klucza biletów – lokalnie): jeden pokój `default` z mapą z `--seed`/`--params`,
//! - **bilety** (`--ticket-key` / `TICKET_PUBLIC_KEY[_FILE]` – w chmurze): `/ws?ticket=<JWT>`, pokój
//!   i jego konfiguracja z biletu wystawionego przez lobby.
//!
//! Zmienne środowiskowe (systemd na EC2): `PORT`, `STATIC_DIR`, `TICKET_PUBLIC_KEY` (PEM) albo
//! `TICKET_PUBLIC_KEY_FILE`, `ORIGIN_VERIFY_SECRET` (wymagany nagłówek `X-Origin-Verify` – tylko
//! CloudFront go zna), `ROOM_IDLE_SECS`, `LOG_FORMAT=json`, `RUST_LOG`, `ROOMS_TABLE` (tabela pokoi
//! lobby – heartbeat co 5 s; tylko w buildzie z cechą `aws`).

mod heartbeat;
mod limits;
mod room;
mod rooms;

use std::{net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};

use axum::{
    Json, Router,
    extract::{
        Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
};
use futures_util::{SinkExt, StreamExt};
use game_core::{
    mapgen::{GENERATOR_VERSION, MapGenParams},
    protocol::{ClientMsg, GameConfig, ServerMsg},
};
use game_ticket::Verifier;
use serde::Deserialize;
use tokio::sync::{mpsc, oneshot};
use tower_http::{
    compression::Compression,
    services::{ServeDir, ServeFile},
};
use tracing::{info, warn};

use room::{RoomCmd, RoomHandle, RoomRules};
use rooms::{RegistryCmd, RegistryHandle, TicketUse};

/// Zbudowany frontend (`ng build`) – lokalnie serwowany z tej samej binarki. W chmurze frontend
/// jest w S3 za CloudFront, a katalogu nie ma.
const STATIC_DIR: &str = "web/dist/web/browser";
const MAX_MESSAGE_BYTES: usize = 16 * 1024;
/// Pokój w trybie otwartym (bez biletów).
const DEFAULT_ROOM: &str = "default";
const ORIGIN_HEADER: &str = "x-origin-verify";

#[derive(Clone)]
struct AppState {
    /// Limity połączeń (na adres IP za CloudFront i łącznie).
    conns: Arc<limits::ConnLimits>,
    rooms: RegistryHandle,
    /// Intencje debugowe dozwolone (`--dev`).
    dev: bool,
    /// `Some` = tryb biletów.
    verifier: Option<Arc<Verifier>>,
    /// `Some` = `/ws` wymaga nagłówka `X-Origin-Verify` z tą wartością.
    origin_secret: Option<Arc<str>>,
    /// Konfiguracja pokoju `default` (tryb otwarty).
    default_config: GameConfig,
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let env = |name: &str| std::env::var(name).ok().filter(|v| !v.is_empty());
    init_logging(env("LOG_FORMAT").as_deref() == Some("json"));

    let dev = args.iter().any(|a| a == "--dev");
    let port: u16 = arg("--port").or_else(|| env("PORT")).and_then(|p| p.parse().ok()).unwrap_or(3000);
    let mut map: MapGenParams = match arg("--params") {
        Some(path) => {
            let json = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
            serde_json::from_str(&json).unwrap_or_else(|e| panic!("{path}: {e}"))
        }
        None => MapGenParams::default(),
    };
    if let Some(seed) = arg("--seed") {
        map.seed = seed.parse().expect("--seed: liczba u32");
    }
    let static_dir = arg("--static-dir")
        .or_else(|| env("STATIC_DIR"))
        .map(PathBuf::from)
        .or_else(|| Some(PathBuf::from(STATIC_DIR)).filter(|d| d.is_dir()));
    let ticket_key = match arg("--ticket-key").or_else(|| env("TICKET_PUBLIC_KEY_FILE")) {
        Some(path) => Some(std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))),
        None => env("TICKET_PUBLIC_KEY"),
    };
    let verifier = ticket_key
        .map(|pem| Arc::new(Verifier::from_pem(&pem).expect("niepoprawny klucz publiczny biletów (PEM Ed25519)")));
    let idle = Duration::from_secs(env("ROOM_IDLE_SECS").and_then(|s| s.parse().ok()).unwrap_or(300));

    let state = AppState {
        conns: limits::ConnLimits::new(limits::PER_IP, limits::TOTAL),
        rooms: rooms::spawn(idle),
        dev,
        origin_secret: env("ORIGIN_VERIFY_SECRET").map(Arc::from),
        default_config: GameConfig { generator_version: GENERATOR_VERSION, map },
        verifier,
    };
    info!(
        port,
        dev,
        tickets = state.verifier.is_some(),
        origin_check = state.origin_secret.is_some(),
        static_dir = ?static_dir,
        seed = state.default_config.map.seed,
        "start serwera"
    );
    start_heartbeat(&state.rooms, env("ROOMS_TABLE")).await;

    let listener = tokio::net::TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], port))).await.expect("bind");
    axum::serve(listener, app(state, static_dir)).with_graceful_shutdown(shutdown_signal()).await.expect("server");
    info!("serwer zatrzymany");
}

fn app(state: AppState, static_dir: Option<PathBuf>) -> Router {
    let router = Router::new().route("/ws", get(ws_handler)).route("/health", get(health)).with_state(state);
    match static_dir {
        // Pliki frontendu kompresowane w locie (brotli albo gzip, zależnie od przeglądarki) –
        // na wolnych łączach wczytanie skraca się mniej więcej o połowę.
        Some(dir) => router.fallback_service(Compression::new(
            ServeDir::new(&dir).not_found_service(ServeFile::new(dir.join("index.html"))),
        )),
        None => router,
    }
}

/// Heartbeat pokoi do tabeli lobby – tylko w buildzie z cechą `aws` i z `ROOMS_TABLE`.
#[cfg(feature = "aws")]
async fn start_heartbeat(rooms: &RegistryHandle, table: Option<String>) {
    let Some(table) = table else { return };
    let config = aws_config::load_from_env().await;
    info!(table, every_secs = heartbeat::HEARTBEAT_SECS, "heartbeat pokoi do lobby");
    heartbeat::spawn(
        rooms.clone(),
        heartbeat::dynamo::DynamoSink { client: aws_sdk_dynamodb::Client::new(&config), table },
    );
}

#[cfg(not(feature = "aws"))]
async fn start_heartbeat(_rooms: &RegistryHandle, table: Option<String>) {
    if table.is_some() {
        warn!("ROOMS_TABLE ustawione, ale build bez cechy `aws` – heartbeat wyłączony");
    }
}

fn init_logging(json: bool) {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into());
    let fmt = tracing_subscriber::fmt().with_env_filter(filter);
    if json { fmt.json().init() } else { fmt.init() }
}

/// Ctrl+C lokalnie, SIGTERM od systemd przy wdrożeniu.
async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        let mut sig = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).expect("SIGTERM");
        sig.recv().await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

async fn health(State(app): State<AppState>) -> Response {
    let (reply, rx) = oneshot::channel();
    let _ = app.rooms.send(RegistryCmd::Count { reply });
    match rx.await {
        Ok(rooms) => Json(serde_json::json!({
            "status": "ok",
            "rooms": rooms,
            "generatorVersion": GENERATOR_VERSION,
            "tickets": app.verifier.is_some(),
        }))
        .into_response(),
        Err(_) => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}

#[derive(Deserialize)]
struct WsQuery {
    ticket: Option<String>,
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(app): State<AppState>,
    Query(query): Query<WsQuery>,
    headers: HeaderMap,
) -> Response {
    if let Some(secret) = &app.origin_secret {
        let ok = headers.get(ORIGIN_HEADER).is_some_and(|v| constant_time_eq(v.as_bytes(), secret.as_bytes()));
        if !ok {
            return StatusCode::FORBIDDEN.into_response();
        }
    }
    // Adres klienta tylko zza CloudFront (nagłówek originu już sprawdzony); lokalnie – bez limitu na IP.
    let ip = app
        .origin_secret
        .as_ref()
        .and_then(|_| headers.get("cloudfront-viewer-address"))
        .and_then(|v| v.to_str().ok())
        .map(limits::viewer_ip);
    let Some(guard) = app.conns.acquire(ip) else {
        warn!(ip, "limit połączeń");
        return (StatusCode::TOO_MANY_REQUESTS, "za dużo połączeń").into_response();
    };
    // Pokój z biletu ma poczekalnię i limit graczy; pokój `default` (tryb otwarty) startuje od razu.
    let open_rules = RoomRules { dev: app.dev, auto_start: true, max_players: u16::MAX };
    let (id, name, config, rules, client_id, ticket) = match (&app.verifier, query.ticket) {
        (Some(verifier), Some(token)) => match verifier.verify(&token) {
            Ok(c) => {
                let rules = RoomRules { dev: app.dev, auto_start: false, max_players: c.max_players };
                (c.room, c.name, c.config, rules, Some(c.client), Some(TicketUse { jti: c.jti, exp: c.exp }))
            }
            Err(e) => {
                warn!(error = %e, "odrzucony bilet");
                return (StatusCode::UNAUTHORIZED, "niepoprawny bilet").into_response();
            }
        },
        (Some(_), None) => return (StatusCode::UNAUTHORIZED, "brak biletu").into_response(),
        (None, _) => (DEFAULT_ROOM.to_string(), "gość".to_string(), app.default_config.clone(), open_rules, None, None),
    };

    let (reply, rx) = oneshot::channel();
    if app.rooms.send(RegistryCmd::Open { id, config: Box::new(config), rules, ticket, reply }).is_err() {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    let room = match rx.await {
        Ok(Ok(room)) => room,
        Ok(Err(e)) => return (StatusCode::CONFLICT, format!("{e:?}")).into_response(),
        Err(_) => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };
    ws.max_message_size(MAX_MESSAGE_BYTES).on_upgrade(move |socket| async move {
        client(socket, room, name, client_id).await;
        drop(guard); // miejsce na połączenie zwolnione dopiero po jego końcu
    })
}

/// Porównanie sekretu bez wczesnego wyjścia (czas nie zdradza, ile znaków się zgadza).
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Jedno połączenie = jeden task. Pisanie do gniazda robi osobny task,
/// żeby wolny klient nie blokował pokoju.
async fn client(socket: WebSocket, room: RoomHandle, name: String, client_id: Option<String>) {
    let (mut sink, mut stream) = socket.split();
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<String>();
    let (reply_tx, reply_rx) = oneshot::channel();
    if room.send(RoomCmd::Join { out: out_tx, name, client: client_id, reply: reply_tx }).is_err() {
        return;
    }
    let player = match reply_rx.await {
        Ok(Ok(player)) => player,
        Ok(Err(reason)) => {
            // Pokój pełny / gra trwa bez tego gracza: powód dla klienta i koniec połączenia.
            let refused = serde_json::to_string(&ServerMsg::Refused { reason }).unwrap();
            let _ = sink.send(Message::Text(refused.into())).await;
            let _ = sink.close().await;
            return;
        }
        Err(_) => return,
    };

    let writer = tokio::spawn(async move {
        while let Some(text) = out_rx.recv().await {
            if sink.send(Message::Text(text.into())).await.is_err() {
                break;
            }
        }
    });

    // Zwykły klient: ~1 wiadomość/s (hash co 10 tur) i pojedyncze intencje – zalewanie = rozłączenie.
    let mut bucket = limits::MessageBucket::new(20.0, 60.0);
    while let Some(Ok(msg)) = stream.next().await {
        if !bucket.take() {
            warn!(player, "za dużo wiadomości – rozłączam");
            break;
        }
        match msg {
            Message::Text(text) => match serde_json::from_str::<ClientMsg>(text.as_str()) {
                Ok(msg) => {
                    let _ = room.send(RoomCmd::Client { player, msg });
                }
                Err(e) => warn!(player, error = %e, "niepoprawna wiadomość"),
            },
            Message::Close(_) => break,
            _ => {}
        }
    }
    let _ = room.send(RoomCmd::Leave { player });
    writer.abort();
}

#[cfg(test)]
mod tests {
    use game_core::protocol::PlayerId;
    use game_ticket::{Signer, TicketClaims, now_secs, test_keys};
    use tokio_tungstenite::tungstenite::{self, client::IntoClientRequest};

    use super::*;

    const SECRET: &str = "s3cret";

    fn config(seed: u32) -> GameConfig {
        GameConfig {
            generator_version: GENERATOR_VERSION,
            map: MapGenParams { seed, width: 200, height: 160, ..Default::default() },
        }
    }

    async fn serve(verifier: Option<&str>, secret: Option<&str>) -> SocketAddr {
        let state = AppState {
            rooms: rooms::spawn(Duration::from_secs(60)),
            dev: false,
            conns: limits::ConnLimits::new(limits::PER_IP, limits::TOTAL),
            verifier: verifier.map(|pem| Arc::new(Verifier::from_pem(pem).unwrap())),
            origin_secret: secret.map(Arc::from),
            default_config: config(42),
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(axum::serve(listener, app(state, None)).into_future());
        addr
    }

    fn ticket(room: &str, seed: u32, jti: &str) -> String {
        let now = now_secs();
        let claims = TicketClaims {
            room: room.into(),
            name: "t".into(),
            client: format!("karta-{jti}"),
            max_players: 8,
            config: config(seed),
            jti: jti.into(),
            iat: now,
            exp: now + 60,
        };
        Signer::from_pem(&test_keys::pair(1).private).unwrap().sign(&claims).unwrap()
    }

    /// Łączy się i zwraca `Welcome` (ID gracza, seed mapy) albo status HTTP odmowy.
    async fn join(addr: SocketAddr, ticket: Option<&str>, secret: Option<&str>) -> Result<(PlayerId, u32), u16> {
        let url = match ticket {
            Some(t) => format!("ws://{addr}/ws?ticket={t}"),
            None => format!("ws://{addr}/ws"),
        };
        let mut req = url.into_client_request().unwrap();
        if let Some(s) = secret {
            req.headers_mut().insert(ORIGIN_HEADER, s.parse().unwrap());
        }
        let (mut ws, _) = match tokio_tungstenite::connect_async(req).await {
            Ok(ok) => ok,
            Err(tungstenite::Error::Http(resp)) => return Err(resp.status().as_u16()),
            Err(e) => panic!("{e}"),
        };
        let msg = ws.next().await.unwrap().unwrap();
        match serde_json::from_str::<ServerMsg>(msg.to_text().unwrap()).unwrap() {
            ServerMsg::Welcome { player, config, .. } => {
                // Gniazdo zostaje otwarte do końca testu – gracz dalej jest w pokoju.
                std::mem::forget(ws);
                Ok((player, config.map.seed))
            }
            other => panic!("oczekiwano Welcome, jest {other:?}"),
        }
    }

    #[tokio::test]
    async fn tickets_route_players_to_their_rooms() {
        let addr = serve(Some(&test_keys::pair(1).public), Some(SECRET)).await;
        let s = Some(SECRET);
        assert_eq!(join(addr, Some(&ticket("a", 1, "j1")), s).await, Ok((0, 1)));
        assert_eq!(join(addr, Some(&ticket("b", 2, "j2")), s).await, Ok((0, 2)));
        assert_eq!(join(addr, Some(&ticket("a", 1, "j3")), s).await, Ok((1, 1)));
    }

    #[tokio::test]
    async fn bad_requests_are_refused() {
        let addr = serve(Some(&test_keys::pair(1).public), Some(SECRET)).await;
        let s = Some(SECRET);
        let good = ticket("a", 1, "j1");
        // Bez nagłówka CloudFront / z innym sekretem.
        assert_eq!(join(addr, Some(&good), None).await, Err(403));
        assert_eq!(join(addr, Some(&good), Some("zly")).await, Err(403));
        // Bez biletu, ze złym biletem.
        assert_eq!(join(addr, None, s).await, Err(401));
        assert_eq!(join(addr, Some("abc.def.ghi"), s).await, Err(401));
        // Bilet jednorazowy; inna konfiguracja dla istniejącego pokoju.
        assert!(join(addr, Some(&good), s).await.is_ok());
        assert_eq!(join(addr, Some(&good), s).await, Err(409));
        assert_eq!(join(addr, Some(&ticket("a", 9, "j2")), s).await, Err(409));
    }

    #[tokio::test]
    async fn open_mode_uses_default_room() {
        let addr = serve(None, None).await;
        assert_eq!(join(addr, None, None).await, Ok((0, 42)));
        assert_eq!(join(addr, None, None).await, Ok((1, 42)));
    }
}
