//! Logika API lobby niezależna od Lambdy: `handle(metoda, ścieżka, treść)` → (status, JSON).
//!
//! - `GET  /api/rooms`            – otwarte pokoje (`RoomSummary[]`),
//! - `POST /api/rooms`            – nowy pokój (`CreateRoom` → `RoomSummary`),
//! - `POST /api/rooms/{id}/join`  – bilet do game-servera (`JoinRoom` → `JoinResponse`).

use game_core::{
    lobby::{ApiError, CreateRoom, DEFAULT_MAX_PLAYERS, JoinResponse, JoinRoom, MAX_PLAYERS, RoomSummary, clean_name},
    mapgen::{GENERATOR_VERSION, MapGenParams},
    protocol::GameConfig,
};
use game_ticket::{Signer, TTL_SECS, TicketClaims};
use http::{Method, StatusCode};
use serde::{Serialize, de::DeserializeOwned};
use tracing::{error, info};

use crate::store::{Room, RoomStore, STATUS_OPEN};

/// Ile pokoi pokazuje lobby.
const LIST_LIMIT: i32 = 50;
/// Liczba graczy z heartbeatu jest aktualna tylko tyle sekund (heartbeat co 15 s).
const PLAYERS_FRESH_SECS: u64 = 60;
/// Największa akceptowana treść żądania (bajty).
const MAX_BODY: usize = 4 * 1024;

pub struct App<S> {
    pub store: S,
    pub signer: Signer,
    /// Źródło losowości (ID pokoi, ID biletów, seedy) – podmieniane w testach.
    pub random: fn() -> u64,
}

pub struct Reply {
    pub status: StatusCode,
    pub body: String,
}

fn json<T: Serialize>(status: StatusCode, value: &T) -> Reply {
    Reply { status, body: serde_json::to_string(value).expect("JSON") }
}

fn fail(status: StatusCode, msg: &str) -> Reply {
    json(status, &ApiError { error: msg.to_string() })
}

fn parse<T: DeserializeOwned>(body: &[u8]) -> Result<T, Reply> {
    if body.len() > MAX_BODY {
        return Err(fail(StatusCode::PAYLOAD_TOO_LARGE, "za duże żądanie"));
    }
    serde_json::from_slice(body).map_err(|_| fail(StatusCode::BAD_REQUEST, "niepoprawny JSON"))
}

fn summary(room: &Room, now: u64) -> RoomSummary {
    let fresh = now.saturating_sub(room.last_seen) <= PLAYERS_FRESH_SECS;
    RoomSummary {
        id: room.id.clone(),
        name: room.name.clone(),
        seed: room.config.map.seed,
        players: if fresh { room.players } else { 0 },
        max_players: room.max_players,
        created_at: room.created_at,
    }
}

impl<S: RoomStore> App<S> {
    pub async fn handle(&self, method: &Method, path: &str, body: &[u8], now: u64) -> Reply {
        let segments: Vec<&str> = path.trim_matches('/').split('/').collect();
        let result = match (method, segments.as_slice()) {
            (&Method::GET, ["api", "rooms"]) => self.list(now).await,
            (&Method::POST, ["api", "rooms"]) => self.create(body, now).await,
            (&Method::POST, ["api", "rooms", id, "join"]) => self.join(id, body, now).await,
            _ => Err(fail(StatusCode::NOT_FOUND, "nie ma takiego zasobu")),
        };
        result.unwrap_or_else(|reply| reply)
    }

    async fn list(&self, now: u64) -> Result<Reply, Reply> {
        let rooms = self.store.list_open(LIST_LIMIT).await.map_err(store_error)?;
        Ok(json(StatusCode::OK, &rooms.iter().map(|r| summary(r, now)).collect::<Vec<_>>()))
    }

    async fn create(&self, body: &[u8], now: u64) -> Result<Reply, Reply> {
        let req: CreateRoom = parse(body)?;
        let name = clean_name(&req.name).ok_or_else(|| fail(StatusCode::BAD_REQUEST, "pusta nazwa pokoju"))?;
        let max_players = req.max_players.unwrap_or(DEFAULT_MAX_PLAYERS);
        if !(1..=MAX_PLAYERS).contains(&max_players) {
            return Err(fail(StatusCode::BAD_REQUEST, "limit graczy poza zakresem 1–16"));
        }
        let seed = req.seed.unwrap_or_else(|| (self.random)() as u32);
        let room = Room {
            id: format!("{:016x}", (self.random)()),
            name,
            config: GameConfig {
                generator_version: GENERATOR_VERSION,
                map: MapGenParams { seed, ..Default::default() },
            },
            max_players,
            players: 0,
            status: STATUS_OPEN.into(),
            created_at: now,
            last_seen: now,
        };
        self.store.create(&room).await.map_err(store_error)?;
        info!(room = %room.id, seed, "utworzono pokój");
        Ok(json(StatusCode::CREATED, &summary(&room, now)))
    }

    async fn join(&self, id: &str, body: &[u8], now: u64) -> Result<Reply, Reply> {
        let req: JoinRoom = parse(body)?;
        let player = clean_name(&req.player_name).ok_or_else(|| fail(StatusCode::BAD_REQUEST, "pusta nazwa gracza"))?;
        let room = self
            .store
            .get(id)
            .await
            .map_err(store_error)?
            .filter(|r| r.status == STATUS_OPEN)
            .ok_or_else(|| fail(StatusCode::NOT_FOUND, "nie ma takiego pokoju"))?;
        let info = summary(&room, now);
        if info.players >= room.max_players {
            return Err(fail(StatusCode::CONFLICT, "pokój jest pełny"));
        }
        let claims = TicketClaims {
            room: room.id.clone(),
            name: player,
            config: room.config.clone(),
            jti: format!("{:016x}", (self.random)()),
            iat: now,
            exp: now + TTL_SECS,
        };
        let ticket = self.signer.sign(&claims).map_err(|e| {
            error!(error = %e, "podpis biletu");
            fail(StatusCode::INTERNAL_SERVER_ERROR, "błąd serwera")
        })?;
        Ok(json(StatusCode::OK, &JoinResponse { ws_path: format!("/ws?ticket={ticket}"), room: info }))
    }
}

fn store_error(e: crate::store::StoreError) -> Reply {
    error!(error = %e, "magazyn pokoi");
    fail(StatusCode::INTERNAL_SERVER_ERROR, "błąd serwera")
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use game_ticket::{Verifier, test_keys};

    use super::*;
    use crate::store::MemoryStore;

    fn counter() -> u64 {
        static N: AtomicU64 = AtomicU64::new(1);
        N.fetch_add(1, Ordering::Relaxed)
    }

    fn app() -> App<MemoryStore> {
        App {
            store: MemoryStore::default(),
            signer: Signer::from_pem(&test_keys::pair(1).private).unwrap(),
            random: counter,
        }
    }

    async fn call(app: &App<MemoryStore>, method: Method, path: &str, body: &str, now: u64) -> (StatusCode, String) {
        let r = app.handle(&method, path, body.as_bytes(), now).await;
        (r.status, r.body)
    }

    #[tokio::test]
    async fn create_list_join_issue_valid_ticket() {
        let app = app();
        // Prawdziwy zegar: bilet przechodzi pełną weryfikację game-servera (podpis i `exp`).
        let now = game_ticket::now_secs();
        let (status, body) = call(&app, Method::POST, "/api/rooms", r#"{"name":"  Pokój Ali ","seed":7}"#, now).await;
        assert_eq!(status, StatusCode::CREATED);
        let room: RoomSummary = serde_json::from_str(&body).unwrap();
        assert_eq!((room.name.as_str(), room.seed, room.max_players), ("Pokój Ali", 7, DEFAULT_MAX_PLAYERS));

        let (status, body) = call(&app, Method::GET, "/api/rooms", "", now).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(serde_json::from_str::<Vec<RoomSummary>>(&body).unwrap(), vec![room.clone()]);

        let path = format!("/api/rooms/{}/join", room.id);
        let (status, body) = call(&app, Method::POST, &path, r#"{"playerName":"Ola"}"#, now).await;
        assert_eq!(status, StatusCode::OK);
        let join: JoinResponse = serde_json::from_str(&body).unwrap();
        let ticket = join.ws_path.strip_prefix("/ws?ticket=").unwrap();
        let claims = Verifier::from_pem(&test_keys::pair(1).public).unwrap().verify(ticket).unwrap();
        assert_eq!((claims.room.as_str(), claims.name.as_str(), claims.config.map.seed), (room.id.as_str(), "Ola", 7));
        assert_eq!(claims.exp - claims.iat, TTL_SECS);
    }

    #[tokio::test]
    async fn bad_requests_are_rejected() {
        let app = app();
        assert_eq!(call(&app, Method::POST, "/api/rooms", "nie json", 0).await.0, StatusCode::BAD_REQUEST);
        assert_eq!(call(&app, Method::POST, "/api/rooms", r#"{"name":"  "}"#, 0).await.0, StatusCode::BAD_REQUEST);
        assert_eq!(
            call(&app, Method::POST, "/api/rooms", r#"{"name":"a","maxPlayers":0}"#, 0).await.0,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            call(&app, Method::POST, "/api/rooms", r#"{"name":"a","maxPlayers":99}"#, 0).await.0,
            StatusCode::BAD_REQUEST
        );
        let big = format!(r#"{{"name":"{}"}}"#, "x".repeat(MAX_BODY));
        assert_eq!(call(&app, Method::POST, "/api/rooms", &big, 0).await.0, StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(
            call(&app, Method::POST, "/api/rooms/nie-ma/join", r#"{"playerName":"a"}"#, 0).await.0,
            StatusCode::NOT_FOUND
        );
        assert_eq!(call(&app, Method::DELETE, "/api/rooms", "", 0).await.0, StatusCode::NOT_FOUND);
        assert_eq!(call(&app, Method::GET, "/api/inne", "", 0).await.0, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn full_room_refuses_and_stale_heartbeat_counts_zero() {
        let app = app();
        let (_, body) = call(&app, Method::POST, "/api/rooms", r#"{"name":"mały","maxPlayers":2}"#, 1000).await;
        let room: RoomSummary = serde_json::from_str(&body).unwrap();
        app.store.rooms.lock().unwrap()[0].players = 2; // heartbeat: 2 graczy
        let path = format!("/api/rooms/{}/join", room.id);
        assert_eq!(call(&app, Method::POST, &path, r#"{"playerName":"c"}"#, 1010).await.0, StatusCode::CONFLICT);
        // Heartbeat sprzed ponad minuty – liczba graczy nieaktualna, wejście dozwolone.
        assert_eq!(
            call(&app, Method::POST, &path, r#"{"playerName":"c"}"#, 1000 + PLAYERS_FRESH_SECS + 1).await.0,
            StatusCode::OK
        );
        let (_, list) = call(&app, Method::GET, "/api/rooms", "", 1000 + PLAYERS_FRESH_SECS + 1).await;
        assert_eq!(serde_json::from_str::<Vec<RoomSummary>>(&list).unwrap()[0].players, 0);
    }
}
