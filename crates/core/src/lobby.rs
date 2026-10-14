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
    /// Ustawienia mapy (opis na liście: rozmiar, proporcje). Brak u starszej Lambdy – frontend
    /// zgaduje wtedy rozmiar z `map_width`.
    #[serde(default)]
    pub map: MapSettings,
    /// Gracze w poczekalni według ostatniego heartbeatu game-servera (0, gdy dawno go nie było).
    pub players: u16,
    pub max_players: u16,
    /// Czas utworzenia (sekundy uniksowe).
    pub created_at: u64,
}

/// Rozmiar mapy (pole jak kwadrat 1000 / 1400 / 1800 kafli, kształt – `MapAspect`). Większa mapa =
/// dłuższe generowanie u klientów.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum MapSize {
    Small,
    #[default]
    Medium,
    Large,
}

/// Proporcje mapy (szerokość : wysokość). Pole mapy zależy tylko od rozmiaru, więc czas generowania
/// i liczba prowincji są takie same przy każdych proporcjach.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum MapAspect {
    /// 1:1.
    #[default]
    Square,
    /// 3:2.
    Wide,
    /// 2:1.
    Wider,
}

impl MapAspect {
    /// Szerokość i wysokość w tysięcznych boku kwadratu o tym samym polu (√r, 1/√r) i siatka chunków
    /// (chunki zostają mniej więcej kwadratowe, ok. 100 sztuk jak na mapie 10 × 10).
    fn shape(self) -> (u32, u32, u32, u32) {
        match self {
            MapAspect::Square => (1000, 1000, 10, 10),
            MapAspect::Wide => (1225, 816, 12, 8),
            MapAspect::Wider => (1414, 707, 14, 7),
        }
    }
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
    /// Brak w starszych biletach i heartbeatach = 1:1 (`serde(default)` na całej strukturze).
    pub aspect: MapAspect,
    /// 1–6.
    pub continents: u32,
    pub land: LandAmount,
    pub climate: Climate,
}

impl Default for MapSettings {
    fn default() -> Self {
        MapSettings {
            size: MapSize::Medium,
            aspect: MapAspect::Square,
            continents: 3,
            land: LandAmount::Standard,
            climate: Climate::Varied,
        }
    }
}

pub const MAX_CONTINENTS: u32 = 6;

impl MapSettings {
    /// Parametry generatora: domyślne z nadpisanymi najważniejszymi polami.
    pub fn params(&self, seed: u32) -> MapGenParams {
        let mut p = MapGenParams { seed, ..Default::default() };
        let side = self.size.side();
        let (w, h, cols, rows) = self.aspect.shape();
        // Zaokrąglenie do 10 kafli; liczby całkowite – ten sam wynik wszędzie.
        let scaled = |k: u32| (side * k / 1000 + 5) / 10 * 10;
        (p.width, p.height) = (scaled(w), scaled(h));
        (p.chunk_cols, p.chunk_rows) = (cols, rows);
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

    /// Ustawienia, z których powstały parametry (odwrotność `params`; pola spoza ustawień są pomijane,
    /// nietypowe wartości – najbliższa opcja).
    pub fn from_params(p: &MapGenParams) -> Self {
        let area = p.width as u64 * p.height as u64;
        let size = match area {
            ..1_440_000 => MapSize::Small,
            1_440_000..2_560_000 => MapSize::Medium,
            _ => MapSize::Large,
        };
        // Progi w połowie między proporcjami (w tysięcznych): 1,25 i 1,75.
        let aspect = match p.width as u64 * 1000 / p.height.max(1) as u64 {
            ..1250 => MapAspect::Square,
            1250..1750 => MapAspect::Wide,
            _ => MapAspect::Wider,
        };
        let land = match p.land_ratio {
            r if r < 0.55 => LandAmount::Islands,
            r if r < 0.74 => LandAmount::Standard,
            _ => LandAmount::Pangea,
        };
        let climate = if p.biome_polar == 0.0 && p.biome_tropical > 0.0 {
            Climate::Warm
        } else if p.biome_tropical == 0.0 && p.biome_polar > 0.0 {
            Climate::Cold
        } else {
            Climate::Varied
        };
        MapSettings { size, aspect, continents: p.continents.clamp(1, MAX_CONTINENTS), land, climate }
    }
}

impl MapSize {
    /// Bok kwadratu o polu mapy (kafle).
    pub fn side(self) -> u32 {
        match self {
            MapSize::Small => 1000,
            MapSize::Medium => 1400,
            MapSize::Large => 1800,
        }
    }
}

/// Ustawienia pokoju, które gospodarz może zmienić w poczekalni (`ClientMsg::Settings`). Game-server
/// jest ich źródłem prawdy od utworzenia pokoju; lobby dostaje zmiany heartbeatem.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct RoomSettings {
    pub name: String,
    pub map: MapSettings,
    pub seed: u32,
    pub max_players: u16,
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
        let p = MapSettings {
            size: MapSize::Large,
            aspect: MapAspect::Square,
            continents: 99,
            land: LandAmount::Islands,
            climate: Climate::Cold,
        }
        .params(9);
        assert_eq!((p.width, p.height, p.continents, p.land_ratio), (1800, 1800, MAX_CONTINENTS, 0.45));
        assert_eq!((p.biome_tropical, p.biome_polar), (0.0, 0.9));
        assert_eq!(p.mountain_share, base.mountain_share);
        assert_eq!(p.sanitized(), p, "wartości w zakresach generatora");
    }

    #[test]
    fn aspect_keeps_the_map_area() {
        for size in [MapSize::Small, MapSize::Medium, MapSize::Large] {
            let square = size.side() * size.side();
            for (aspect, ratio) in [(MapAspect::Square, 1.0), (MapAspect::Wide, 1.5), (MapAspect::Wider, 2.0)] {
                let p = MapSettings { size, aspect, ..MapSettings::default() }.params(1);
                let area = p.width * p.height;
                assert!(area.abs_diff(square) * 100 < square, "{size:?} {aspect:?}: {}×{}", p.width, p.height);
                assert!((p.width as f64 / p.height as f64 - ratio).abs() < 0.02, "{size:?} {aspect:?}");
                assert_eq!((p.width % 10, p.height % 10), (0, 0));
                // Chunki mniej więcej kwadratowe.
                let chunk = (p.width as f64 / p.chunk_cols as f64) / (p.height as f64 / p.chunk_rows as f64);
                assert!((chunk - 1.0).abs() < 0.05, "{size:?} {aspect:?}: chunk {chunk}");
                assert_eq!(p.sanitized(), p);
            }
        }
        let p = MapSettings { aspect: MapAspect::Wider, ..MapSettings::default() }.params(1);
        assert_eq!((p.width, p.height, p.chunk_cols, p.chunk_rows), (1980, 990, 14, 7));
    }

    #[test]
    fn map_settings_round_trip_through_params() {
        for size in [MapSize::Small, MapSize::Medium, MapSize::Large] {
            for aspect in [MapAspect::Square, MapAspect::Wide, MapAspect::Wider] {
                for land in [LandAmount::Islands, LandAmount::Standard, LandAmount::Pangea] {
                    for climate in [Climate::Varied, Climate::Warm, Climate::Cold] {
                        for continents in 1..=MAX_CONTINENTS {
                            let s = MapSettings { size, aspect, continents, land, climate };
                            assert_eq!(MapSettings::from_params(&s.params(5)), s);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn settings_without_aspect_are_square() {
        let s: MapSettings = serde_json::from_str(r#"{"size":"large","continents":2}"#).unwrap();
        assert_eq!((s.size, s.aspect, s.continents), (MapSize::Large, MapAspect::Square, 2));
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
