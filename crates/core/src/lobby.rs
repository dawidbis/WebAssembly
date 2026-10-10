//! API lobby (meta-serwer, `crates/meta`): typy żądań i odpowiedzi `/api/*`. Wspólne dla Lambdy
//! i frontendu (typy TS generuje `cargo types`).

use serde::{Deserialize, Serialize};

/// Najdłuższa nazwa gracza / pokoju (znaki).
pub const MAX_NAME_CHARS: usize = 24;
/// Domyślny i największy limit graczy w pokoju.
pub const DEFAULT_MAX_PLAYERS: u16 = 8;
pub const MAX_PLAYERS: u16 = 16;

/// Pokój na liście w lobby.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct RoomSummary {
    pub id: String,
    pub name: String,
    pub seed: u32,
    /// Gracze online według ostatniego heartbeatu game-servera (0, gdy dawno go nie było).
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
