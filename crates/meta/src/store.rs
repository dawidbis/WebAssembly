//! Magazyn pokoi: DynamoDB w produkcji, pamięć w testach. Jedna tabela (docs/adr/0002):
//!
//! | pk            | sk     | atrybuty                                                                 |
//! |---------------|--------|--------------------------------------------------------------------------|
//! | `ROOM#<id>`   | `META` | id, name, config (JSON), maxPlayers, players, status, createdAt, lastSeen, ttl |
//!
//! | `USER#<karta>`| `META` | status = `online`, createdAt = ostatnie zgłoszenie obecności, ttl        |
//!
//! Indeks `byStatus` (status, createdAt): lista pokoi w poczekalni (`open`) od najnowszych i liczba kart
//! online (`online`, createdAt ≥ teraz − okno). `players`, `lastSeen`, `status` pokoi (`open` →
//! `playing` → `closed`) odświeża heartbeat game-servera; TTL usuwa stare wpisy.

use std::collections::HashMap;

use aws_sdk_dynamodb::types::AttributeValue;
use game_core::protocol::GameConfig;

pub const STATUS_OPEN: &str = "open";
pub const STATUS_PLAYING: &str = "playing";
pub const STATUS_ONLINE: &str = "online";
pub const STATUS_INDEX: &str = "byStatus";
/// Pokój bez heartbeatu znika po tym czasie (TTL DynamoDB).
pub const ROOM_TTL_SECS: u64 = 3600;

#[derive(Clone, Debug, PartialEq)]
pub struct Room {
    pub id: String,
    pub name: String,
    pub config: GameConfig,
    pub max_players: u16,
    pub players: u16,
    pub status: String,
    pub created_at: u64,
    pub last_seen: u64,
}

#[derive(Debug)]
pub struct StoreError(pub String);

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

pub(crate) trait RoomStore {
    async fn create(&self, room: &Room) -> Result<(), StoreError>;
    async fn get(&self, id: &str) -> Result<Option<Room>, StoreError>;
    /// Otwarte pokoje, od najnowszych.
    async fn list_open(&self, limit: i32) -> Result<Vec<Room>, StoreError>;
    /// Zgłoszenie obecności karty.
    async fn touch(&self, client_id: &str, now: u64) -> Result<(), StoreError>;
    /// Liczba kart (poza `except`), które zgłosiły się od `since`.
    async fn count_online(&self, since: u64, except: &str) -> Result<u32, StoreError>;
    /// Karta się zamknęła – wpis znika od razu.
    async fn forget(&self, client_id: &str) -> Result<(), StoreError>;
}

// --- DynamoDB ---

pub struct DynamoStore {
    pub client: aws_sdk_dynamodb::Client,
    pub table: String,
}

pub fn room_key(id: &str) -> String {
    format!("ROOM#{id}")
}

fn to_item(room: &Room) -> HashMap<String, AttributeValue> {
    let s = |v: &str| AttributeValue::S(v.to_string());
    let n = |v: u64| AttributeValue::N(v.to_string());
    HashMap::from([
        ("pk".into(), s(&room_key(&room.id))),
        ("sk".into(), s("META")),
        ("id".into(), s(&room.id)),
        ("name".into(), s(&room.name)),
        ("config".into(), s(&serde_json::to_string(&room.config).expect("GameConfig -> JSON"))),
        ("maxPlayers".into(), n(room.max_players.into())),
        ("players".into(), n(room.players.into())),
        ("status".into(), s(&room.status)),
        ("createdAt".into(), n(room.created_at)),
        ("lastSeen".into(), n(room.last_seen)),
        ("ttl".into(), n(room.created_at + ROOM_TTL_SECS)),
    ])
}

fn from_item(item: &HashMap<String, AttributeValue>) -> Result<Room, StoreError> {
    let s =
        |k: &str| item.get(k).and_then(|v| v.as_s().ok()).cloned().ok_or_else(|| StoreError(format!("brak pola {k}")));
    let n = |k: &str| -> Result<u64, StoreError> {
        item.get(k)
            .and_then(|v| v.as_n().ok())
            .and_then(|v| v.parse().ok())
            .ok_or_else(|| StoreError(format!("brak liczby {k}")))
    };
    Ok(Room {
        id: s("id")?,
        name: s("name")?,
        config: serde_json::from_str(&s("config")?).map_err(|e| StoreError(format!("config: {e}")))?,
        max_players: n("maxPlayers")? as u16,
        players: n("players")? as u16,
        status: s("status")?,
        created_at: n("createdAt")?,
        last_seen: n("lastSeen")?,
    })
}

fn err(e: impl std::fmt::Display) -> StoreError {
    StoreError(e.to_string())
}

impl RoomStore for DynamoStore {
    async fn create(&self, room: &Room) -> Result<(), StoreError> {
        self.client
            .put_item()
            .table_name(&self.table)
            .set_item(Some(to_item(room)))
            .condition_expression("attribute_not_exists(pk)")
            .send()
            .await
            .map_err(|e| err(aws_sdk_dynamodb::error::DisplayErrorContext(e)))?;
        Ok(())
    }

    async fn get(&self, id: &str) -> Result<Option<Room>, StoreError> {
        let out = self
            .client
            .get_item()
            .table_name(&self.table)
            .key("pk", AttributeValue::S(room_key(id)))
            .key("sk", AttributeValue::S("META".into()))
            .send()
            .await
            .map_err(|e| err(aws_sdk_dynamodb::error::DisplayErrorContext(e)))?;
        out.item().map(from_item).transpose()
    }

    async fn list_open(&self, limit: i32) -> Result<Vec<Room>, StoreError> {
        let out = self
            .client
            .query()
            .table_name(&self.table)
            .index_name(STATUS_INDEX)
            .key_condition_expression("#s = :open")
            .expression_attribute_names("#s", "status")
            .expression_attribute_values(":open", AttributeValue::S(STATUS_OPEN.into()))
            .scan_index_forward(false)
            .limit(limit)
            .send()
            .await
            .map_err(|e| err(aws_sdk_dynamodb::error::DisplayErrorContext(e)))?;
        out.items().iter().map(from_item).collect()
    }

    async fn touch(&self, client_id: &str, now: u64) -> Result<(), StoreError> {
        let s = |v: &str| AttributeValue::S(v.to_string());
        let n = |v: u64| AttributeValue::N(v.to_string());
        self.client
            .put_item()
            .table_name(&self.table)
            .item("pk", s(&format!("USER#{client_id}")))
            .item("sk", s("META"))
            .item("status", s(STATUS_ONLINE))
            .item("createdAt", n(now))
            .item("ttl", n(now + 600))
            .send()
            .await
            .map_err(|e| err(aws_sdk_dynamodb::error::DisplayErrorContext(e)))?;
        Ok(())
    }

    async fn count_online(&self, since: u64, except: &str) -> Result<u32, StoreError> {
        let mut count = 0;
        let mut start = None;
        loop {
            let out = self
                .client
                .query()
                .table_name(&self.table)
                .index_name(STATUS_INDEX)
                .key_condition_expression("#s = :online AND createdAt >= :since")
                .expression_attribute_names("#s", "status")
                .expression_attribute_values(":online", AttributeValue::S(STATUS_ONLINE.into()))
                .expression_attribute_values(":since", AttributeValue::N(since.to_string()))
                .filter_expression("pk <> :me")
                .expression_attribute_values(":me", AttributeValue::S(format!("USER#{except}")))
                .select(aws_sdk_dynamodb::types::Select::Count)
                .set_exclusive_start_key(start)
                .send()
                .await
                .map_err(|e| err(aws_sdk_dynamodb::error::DisplayErrorContext(e)))?;
            count += out.count() as u32;
            start = out.last_evaluated_key().cloned();
            if start.is_none() {
                return Ok(count);
            }
        }
    }

    async fn forget(&self, client_id: &str) -> Result<(), StoreError> {
        self.client
            .delete_item()
            .table_name(&self.table)
            .key("pk", AttributeValue::S(format!("USER#{client_id}")))
            .key("sk", AttributeValue::S("META".into()))
            .send()
            .await
            .map_err(|e| err(aws_sdk_dynamodb::error::DisplayErrorContext(e)))?;
        Ok(())
    }
}

// --- Pamięć (testy) ---

#[cfg(test)]
#[derive(Default)]
pub struct MemoryStore {
    pub rooms: std::sync::Mutex<Vec<Room>>,
    pub presence: std::sync::Mutex<std::collections::BTreeMap<String, u64>>,
}

#[cfg(test)]
impl RoomStore for MemoryStore {
    async fn create(&self, room: &Room) -> Result<(), StoreError> {
        let mut rooms = self.rooms.lock().unwrap();
        if rooms.iter().any(|r| r.id == room.id) {
            return Err(StoreError("pokój już istnieje".into()));
        }
        rooms.push(room.clone());
        Ok(())
    }

    async fn get(&self, id: &str) -> Result<Option<Room>, StoreError> {
        Ok(self.rooms.lock().unwrap().iter().find(|r| r.id == id).cloned())
    }

    async fn list_open(&self, limit: i32) -> Result<Vec<Room>, StoreError> {
        let mut open: Vec<Room> =
            self.rooms.lock().unwrap().iter().filter(|r| r.status == STATUS_OPEN).cloned().collect();
        open.sort_by_key(|r| std::cmp::Reverse(r.created_at));
        open.truncate(limit as usize);
        Ok(open)
    }

    async fn touch(&self, client_id: &str, now: u64) -> Result<(), StoreError> {
        self.presence.lock().unwrap().insert(client_id.to_string(), now);
        Ok(())
    }

    async fn count_online(&self, since: u64, except: &str) -> Result<u32, StoreError> {
        Ok(self.presence.lock().unwrap().iter().filter(|(id, t)| **t >= since && id.as_str() != except).count() as u32)
    }

    async fn forget(&self, client_id: &str) -> Result<(), StoreError> {
        self.presence.lock().unwrap().remove(client_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use game_core::mapgen::{GENERATOR_VERSION, MapGenParams};

    use super::*;

    #[test]
    fn item_round_trip() {
        let room = Room {
            id: "abc".into(),
            name: "Pokój Ali".into(),
            config: GameConfig {
                generator_version: GENERATOR_VERSION,
                map: MapGenParams { seed: 9, ..Default::default() },
            },
            max_players: 4,
            players: 2,
            status: STATUS_OPEN.into(),
            created_at: 100,
            last_seen: 120,
        };
        let item = to_item(&room);
        assert_eq!(item["pk"].as_s().unwrap(), "ROOM#abc");
        assert_eq!(item["ttl"].as_n().unwrap(), &(100 + ROOM_TTL_SECS).to_string());
        assert_eq!(from_item(&item).unwrap(), room);
    }
}
