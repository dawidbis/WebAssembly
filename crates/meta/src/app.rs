//! Logika API lobby niezależna od Lambdy: `handle(metoda, ścieżka, treść)` → (status, JSON).
//!
//! - `GET  /api/rooms`            – otwarte pokoje (`RoomSummary[]`),
//! - `POST /api/rooms`            – nowy pokój (`CreateRoom` → `RoomSummary`),
//! - `POST /api/rooms/{id}/join`  – bilet do game-servera (`JoinRoom` → `JoinResponse`),
//! - `POST /api/presence`         – „jestem na stronie” (`PresenceUpdate` → `Presence`: liczba kart online).

use game_core::{
    lobby::{
        ApiError, CreateRoom, DEFAULT_MAX_PLAYERS, JoinResponse, JoinRoom, MAX_PLAYERS, PRESENCE_WINDOW_SECS, Presence,
        PresenceUpdate, RoomSummary, clean_name, valid_client_id,
    },
    mapgen::GENERATOR_VERSION,
    protocol::GameConfig,
};
use game_ticket::{Signer, TTL_SECS, TicketClaims};
use http::{Method, StatusCode};
use serde::{Serialize, de::DeserializeOwned};
use tracing::{error, info};

use crate::store::{Room, RoomStore, STATUS_OPEN, STATUS_PLAYING};

/// Ile pokoi pokazuje lobby.
const LIST_LIMIT: i32 = 50;
/// Liczba graczy z heartbeatu jest aktualna tylko tyle sekund (heartbeat co 5 s).
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
        map_width: room.config.map.width,
        continents: room.config.map.continents,
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
            (&Method::POST, ["api", "presence"]) => self.presence(body, now).await,
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
            config: GameConfig { generator_version: GENERATOR_VERSION, map: req.map.unwrap_or_default().params(seed) },
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
        if !valid_client_id(&req.client_id) {
            return Err(fail(StatusCode::BAD_REQUEST, "niepoprawne ID karty"));
        }
        // Do trwającej gry (`playing`) bilet też dostanie – ale wpuści tylko kogoś z poczekalni
        // (game-server zna karty graczy); to droga powrotu po zerwanym połączeniu.
        let room = self
            .store
            .get(id)
            .await
            .map_err(store_error)?
            .filter(|r| r.status == STATUS_OPEN || r.status == STATUS_PLAYING)
            .ok_or_else(|| fail(StatusCode::NOT_FOUND, "nie ma takiego pokoju"))?;
        let info = summary(&room, now);
        if room.status == STATUS_OPEN && info.players >= room.max_players {
            return Err(fail(StatusCode::CONFLICT, "pokój jest pełny"));
        }
        let claims = TicketClaims {
            room: room.id.clone(),
            name: player,
            client: req.client_id,
            max_players: room.max_players,
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

    async fn presence(&self, body: &[u8], now: u64) -> Result<Reply, Reply> {
        let req: PresenceUpdate = parse(body)?;
        if !valid_client_id(&req.client_id) {
            return Err(fail(StatusCode::BAD_REQUEST, "niepoprawne ID karty"));
        }
        self.store.touch(&req.client_id, now).await.map_err(store_error)?;
        let online = self.store.count_online(now.saturating_sub(PRESENCE_WINDOW_SECS)).await.map_err(store_error)?;
        Ok(json(StatusCode::OK, &Presence { online }))
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
    use crate::store::{MemoryStore, STATUS_PLAYING};

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
        let (status, body) =
            call(&app, Method::POST, &path, r#"{"playerName":"Ola","clientId":"karta-0001"}"#, now).await;
        assert_eq!(status, StatusCode::OK);
        let join: JoinResponse = serde_json::from_str(&body).unwrap();
        let ticket = join.ws_path.strip_prefix("/ws?ticket=").unwrap();
        let claims = Verifier::from_pem(&test_keys::pair(1).public).unwrap().verify(ticket).unwrap();
        assert_eq!((claims.room.as_str(), claims.name.as_str(), claims.config.map.seed), (room.id.as_str(), "Ola", 7));
        assert_eq!(claims.exp - claims.iat, TTL_SECS);
        assert_eq!((claims.client.as_str(), claims.max_players), ("karta-0001", DEFAULT_MAX_PLAYERS));
    }

    #[tokio::test]
    async fn map_settings_shape_the_room_config() {
        let app = app();
        let body =
            r#"{"name":"duża","seed":3,"map":{"size":"large","continents":5,"land":"islands","climate":"warm"}}"#;
        let (status, json) = call(&app, Method::POST, "/api/rooms", body, 1000).await;
        assert_eq!(status, StatusCode::CREATED);
        let room: RoomSummary = serde_json::from_str(&json).unwrap();
        assert_eq!((room.map_width, room.continents), (1800, 5));
        let stored = app.store.rooms.lock().unwrap()[0].config.map.clone();
        assert_eq!((stored.land_ratio, stored.biome_polar, stored.seed), (0.45, 0.0, 3));
        // Brak ustawień = mapa domyślna.
        let (_, json) = call(&app, Method::POST, "/api/rooms", r#"{"name":"zwykła"}"#, 1000).await;
        let room: RoomSummary = serde_json::from_str(&json).unwrap();
        assert_eq!((room.map_width, room.continents), (1400, 3));
    }

    #[tokio::test]
    async fn presence_counts_recent_tabs() {
        let app = app();
        let body = |id: &str| format!(r#"{{"clientId":"{id}"}}"#);
        call(&app, Method::POST, "/api/presence", &body("karta-aaaa"), 1000).await;
        let (status, json) = call(&app, Method::POST, "/api/presence", &body("karta-bbbb"), 1010).await;
        assert_eq!((status, json.as_str()), (StatusCode::OK, r#"{"online":2}"#));
        // Karta bez zgłoszenia dłużej niż okno obecności wypada z liczby.
        let (_, json) =
            call(&app, Method::POST, "/api/presence", &body("karta-bbbb"), 1000 + PRESENCE_WINDOW_SECS + 1).await;
        assert_eq!(json, r#"{"online":1}"#);
        assert_eq!(call(&app, Method::POST, "/api/presence", &body("zła karta!"), 0).await.0, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn started_game_still_issues_tickets_for_returning_players() {
        let app = app();
        let (_, body) = call(&app, Method::POST, "/api/rooms", r#"{"name":"gra","maxPlayers":1}"#, 1000).await;
        let room: RoomSummary = serde_json::from_str(&body).unwrap();
        {
            let mut rooms = app.store.rooms.lock().unwrap();
            rooms[0].status = STATUS_PLAYING.into();
            rooms[0].players = 1;
        }
        // Gra trwa i jest „pełna” – bilet i tak wychodzi; o powrocie decyduje game-server (karty z poczekalni).
        let path = format!("/api/rooms/{}/join", room.id);
        assert_eq!(
            call(&app, Method::POST, &path, r#"{"playerName":"a","clientId":"karta-0001"}"#, 1001).await.0,
            StatusCode::OK
        );
        // Gra w toku nie jest na liście lobby.
        assert_eq!(call(&app, Method::GET, "/api/rooms", "", 1001).await.1, "[]");
        assert_eq!(
            call(&app, Method::POST, &path, r#"{"playerName":"a","clientId":"x"}"#, 1001).await.0,
            StatusCode::BAD_REQUEST
        );
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
            call(&app, Method::POST, "/api/rooms/nie-ma/join", r#"{"playerName":"a","clientId":"karta-0001"}"#, 0)
                .await
                .0,
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
        assert_eq!(
            call(&app, Method::POST, &path, r#"{"playerName":"c","clientId":"karta-0001"}"#, 1010).await.0,
            StatusCode::CONFLICT
        );
        // Heartbeat sprzed ponad minuty – liczba graczy nieaktualna, wejście dozwolone.
        assert_eq!(
            call(
                &app,
                Method::POST,
                &path,
                r#"{"playerName":"c","clientId":"karta-0001"}"#,
                1000 + PLAYERS_FRESH_SECS + 1
            )
            .await
            .0,
            StatusCode::OK
        );
        let (_, list) = call(&app, Method::GET, "/api/rooms", "", 1000 + PLAYERS_FRESH_SECS + 1).await;
        assert_eq!(serde_json::from_str::<Vec<RoomSummary>>(&list).unwrap()[0].players, 0);
    }
}
