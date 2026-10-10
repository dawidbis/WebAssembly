//! API lobby (meta-serwer, `crates/meta`): typy żądań i odpowiedzi `/api/*`. Wspólne dla Lambdy
//! i frontendu (typy TS generuje `cargo types`).

use serde::{Deserialize, Serialize};

/// Najdłuższa nazwa gracza / pokoju (znaki).
pub const MAX_NAME_CHARS: usize = 24;
/// Domyślny i największy limit graczy w pokoju.
pub const DEFAULT_MAX_PLAYERS: u16 = 8;
pub const MAX_PLAYERS: u16 = 16;

/// Pokój na liście w lobby (tylko pokoje przed startem gry).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct RoomSummary {
    pub id: String,
    pub name: String,
    pub seed: u32,
    /// Gracze w poczekalni według ostatniego heartbeatu game-servera (0, gdy dawno go nie było).
    pub players: u16,
    pub max_players: u16,
    /// Czas utworzenia (sekundy uniksowe).
    pub created_at: u64,
}

/// `POST /api/rooms`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct CreateRoom {
    pub name: String,
    /// Seed mapy; brak = losowy.
    #[cfg_attr(feature = "ts", ts(optional))]
    pub seed: Option<u32>,
    #[cfg_attr(feature = "ts", ts(optional))]
    pub max_players: Option<u16>,
}

/// `POST /api/rooms/{id}/join`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct JoinRoom {
    pub player_name: String,
    /// ID karty (`PresenceUpdate::client_id`) – po starcie gry wraca tylko ten, kto był w poczekalni.
    pub client_id: String,
}

/// Odpowiedź na dołączenie: ścieżka WebSocketu z biletem (`/ws?ticket=…`, ważny 60 s, jednorazowy).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct JoinResponse {
    pub ws_path: String,
    pub room: RoomSummary,
}

/// Treść każdej odpowiedzi z błędem.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ApiError {
    pub error: String,
}

/// `POST /api/presence` – „jestem na stronie” (co `PRESENCE_EVERY_SECS` z każdej karty).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct PresenceUpdate {
    /// Losowe ID karty (nie konto) – ten sam gracz w dwóch kartach liczy się dwa razy.
    pub client_id: String,
}

/// Liczba kart widzianych w ostatnich `PRESENCE_WINDOW_SECS` sekundach.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Presence {
    pub online: u32,
}

/// Co ile sekund karta zgłasza obecność.
pub const PRESENCE_EVERY_SECS: u64 = 15;
/// Gracz bez zgłoszenia dłużej niż tyle sekund znika z listy (trzy zgłoszenia zapasu).
pub const PRESENCE_WINDOW_SECS: u64 = 45;

/// Poprawne ID karty: 8–64 znaki `[A-Za-z0-9-]`.
pub fn valid_client_id(id: &str) -> bool {
    (8..=64).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

/// Nazwa po oczyszczeniu: bez znaków sterujących, przycięta, najwyżej `MAX_NAME_CHARS` znaków.
/// `None`, gdy nic nie zostało.
pub fn clean_name(raw: &str) -> Option<String> {
    let name: String =
        raw.chars().filter(|c| !c.is_control()).collect::<String>().trim().chars().take(MAX_NAME_CHARS).collect();
    let name = name.trim_end().to_string();
    (!name.is_empty()).then_some(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_cleaned() {
        assert_eq!(clean_name("  Ala  "), Some("Ala".into()));
        assert_eq!(clean_name("A\u{0}l\na"), Some("Ala".into()));
        assert_eq!(clean_name("   "), None);
        assert_eq!(clean_name(&"x".repeat(100)).unwrap().chars().count(), MAX_NAME_CHARS);
        // Wielobajtowe znaki liczone jako znaki, nie bajty.
        assert_eq!(clean_name(&"ż".repeat(30)).unwrap().chars().count(), MAX_NAME_CHARS);
    }
}
