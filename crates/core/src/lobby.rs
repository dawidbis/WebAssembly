//! API lobby (meta-serwer, `crates/meta`): typy żądań i odpowiedzi `/api/*`. Wspólne dla Lambdy
//! i frontendu (typy TS generuje `cargo types`).

use serde::{Deserialize, Serialize};

use crate::mapgen::MapGenParams;

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
    /// Szerokość mapy w kaflach i liczba kontynentów (do opisu na liście).
    pub map_width: u32,
    pub continents: u32,
    /// Gracze w poczekalni według ostatniego heartbeatu game-servera (0, gdy dawno go nie było).
    pub players: u16,
    pub max_players: u16,
    /// Czas utworzenia (sekundy uniksowe).
    pub created_at: u64,
}

/// Rozmiar mapy (szerokość = wysokość w kaflach). Większa mapa = dłuższe generowanie u klientów.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum MapSize {
    Small,
    #[default]
    Medium,
    Large,
}

/// Ile lądu względem wody.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum LandAmount {
    /// Dużo wody, rozrzucone lądy.
    Islands,
    #[default]
    Standard,
    /// Mało wody, zwarte lądy.
    Pangea,
}

/// Przewaga typów klimatu (wagi typów kontynentów).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum Climate {
    #[default]
    Varied,
    /// Tropiki i pustynie.
    Warm,
    /// Kontynentalny i polarny.
    Cold,
}

/// Uproszczone ustawienia mapy przy zakładaniu lobby – najważniejsze parametry generatora.
/// Resztę `MapGenParams` biorą z wartości domyślnych (te same, które stroi panel debugu).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase", default)]
pub struct MapSettings {
    pub size: MapSize,
    /// 1–6.
    pub continents: u32,
    pub land: LandAmount,
    pub climate: Climate,
}

impl Default for MapSettings {
    fn default() -> Self {
        MapSettings { size: MapSize::Medium, continents: 3, land: LandAmount::Standard, climate: Climate::Varied }
    }
}

pub const MAX_CONTINENTS: u32 = 6;

impl MapSettings {
    /// Parametry generatora: domyślne z nadpisanymi najważniejszymi polami.
    pub fn params(&self, seed: u32) -> MapGenParams {
        let mut p = MapGenParams { seed, ..Default::default() };
        let side = match self.size {
            MapSize::Small => 1000,
            MapSize::Medium => 1400,
            MapSize::Large => 1800,
        };
        (p.width, p.height) = (side, side);
        p.continents = self.continents.clamp(1, MAX_CONTINENTS);
        p.land_ratio = match self.land {
            LandAmount::Islands => 0.45,
            LandAmount::Standard => 0.65,
            LandAmount::Pangea => 0.82,
        };
        match self.climate {
            Climate::Varied => {}
            Climate::Warm => {
                (p.biome_tropical, p.biome_dry, p.biome_temperate) = (1.0, 0.9, 0.5);
                (p.biome_continental, p.biome_polar) = (0.15, 0.0);
            }
            Climate::Cold => {
                (p.biome_tropical, p.biome_dry, p.biome_temperate) = (0.0, 0.2, 0.5);
                (p.biome_continental, p.biome_polar) = (1.0, 0.9);
            }
        }
        p
    }
}

/// `POST /api/rooms`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct CreateRoom {
    pub name: String,
    /// Ustawienia mapy; brak = domyślne.
    #[cfg_attr(feature = "ts", ts(optional))]
    pub map: Option<MapSettings>,
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

/// `POST /api/presence` – „jestem na stronie” (co `PRESENCE_EVERY_SECS` z każdej karty);
/// `POST /api/presence/leave` – karta się zamyka (`navigator.sendBeacon`), znika z licznika od razu.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct PresenceUpdate {
    /// Losowe ID karty (nie konto) – ten sam gracz w dwóch kartach liczy się dwa razy.
    pub client_id: String,
}

/// Liczba **innych** kart widzianych w ostatnich `PRESENCE_WINDOW_SECS` sekundach – pytająca karta
/// dolicza siebie (+1) sama, więc wynik nie zależy od opóźnienia indeksu po jej zapisie.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Presence {
    pub others: u32,
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
    fn map_settings_override_only_their_fields() {
        let base = MapGenParams { seed: 9, ..Default::default() };
        assert_eq!(MapSettings::default().params(9), base, "domyślne ustawienia = domyślne parametry");
        let p = MapSettings { size: MapSize::Large, continents: 99, land: LandAmount::Islands, climate: Climate::Cold }
            .params(9);
        assert_eq!((p.width, p.height, p.continents, p.land_ratio), (1800, 1800, MAX_CONTINENTS, 0.45));
        assert_eq!((p.biome_tropical, p.biome_polar), (0.0, 0.9));
        assert_eq!(p.mountain_share, base.mountain_share);
        assert_eq!(p.sanitized(), p, "wartości w zakresach generatora");
    }

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
