//! Ekspansja i walka (docs/gdd/zaopatrzenie.md „Wojsko w walce”, plan – etap 6): stałe do strojenia,
//! stan ataku i typy raportów. Zasady (front, koszty, zaopatrzenie) są w `game.rs`.
//!
//! Atak to wojsko wysłane na **prowincję**: co turę front zajmuje kafle tej prowincji stykające się
//! (bokiem) z naszymi – niczyje za stały koszt, cudze według gęstości wojska obrońcy i stosunku
//! zaopatrzenia obu stron. Zużyte wojsko ginie, resztka wraca do puli, gdy atak się kończy.

use serde::{Deserialize, Serialize};

use crate::{economy::MILLI, goods::Supply, owners::Nation};

/// Koszt zajęcia kafla niczyjego (osoby wojska); wyżyny drożej (%).
pub const NEUTRAL_TILE_COST: u64 = 20;
pub const HIGHLANDS_COST_PCT: u64 = 150;
/// Kafel innego gracza: `ENEMY_TILE_COST` + gęstość wojska obrońcy na kafel × `DEFENSE_PCT`% ×
/// (zaopatrzenie obrońcy / zaopatrzenie atakującego). Obrońca traci gęstość wojska za każdy kafel.
pub const ENEMY_TILE_COST: u64 = 20;
pub const DEFENSE_PCT: u64 = 200;
/// Szybkość frontu: punkty na turę (kafel niczyj – `NEUTRAL_TILE_POINTS`, cudzy – `ENEMY_TILE_POINTS`).
pub const POINTS_PER_TICK: u64 = 30;
pub const NEUTRAL_TILE_POINTS: u64 = 10;
pub const ENEMY_TILE_POINTS: u64 = 20;
/// Paliwo przyspiesza front: × (1 + `FUEL_SPEED_PCT`% × pokrycie paliwem).
pub const FUEL_SPEED_PCT: u64 = 50;
/// Zużycie w ataku na 1 000 wojska na dzień (w tysięcznych sztuki): zaopatrzenie wojskowe (tylko gdy
/// front styka się z innym graczem) i paliwo (zawsze).
pub const SUPPLY_PER_1000_PER_DAY: u64 = 1000;
pub const FUEL_PER_1000_PER_DAY: u64 = 500;
/// Konie jako zamiennik paliwa: 1 koń = `HORSE_FUEL_PCT`% sztuki paliwa, najwyżej `HORSE_MAX_PCT`% potrzeby.
pub const HORSE_FUEL_PCT: u64 = 50;
pub const HORSE_MAX_PCT: u64 = 50;
/// Współczynnik siły (%) bez zaopatrzenia i z lekkim, standardowym, ciężkim.
pub const NO_SUPPLY_PCT: u64 = 25;
pub const SUPPLY_PCT: [(Supply, u64); 3] =
    [(Supply::HeavyMilitary, 140), (Supply::StandardMilitary, 100), (Supply::LightMilitary, 60)];
/// Ile ataków naraz przy centrum dowodzenia 1 (więcej – od CD 2, etap 5).
pub const MAX_ATTACKS: usize = 1;
/// Ziarno kolejności zajmowania kafli frontu (XOR z kaflem i tickiem).
pub const FRONT_SALT: u64 = 0x4652_4f4e_5400_0001;

/// Trwający atak państwa na prowincję.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attack {
    pub nation: Nation,
    pub province: u16,
    /// Wojsko w ataku (w tysięcznych osoby).
    pub troops: u64,
    /// Kafle zajęte w tym ataku.
    pub taken: u32,
}

impl Attack {
    pub fn hash_bytes(&self) -> impl Iterator<Item = u8> {
        let mut b = [0u8; 15];
        b[0] = self.nation;
        b[1..3].copy_from_slice(&self.province.to_le_bytes());
        b[3..11].copy_from_slice(&self.troops.to_le_bytes());
        b[11..15].copy_from_slice(&self.taken.to_le_bytes());
        b.into_iter()
    }
}

/// Koszt kafla niczyjego (w tysięcznych osoby).
pub fn neutral_cost(highlands: bool) -> u64 {
    let cost = NEUTRAL_TILE_COST * MILLI;
    if highlands { cost * HIGHLANDS_COST_PCT / 100 } else { cost }
}

/// Koszt kafla obrońcy przy jego gęstości wojska `density` (tysięczne osoby na kafel) i współczynnikach
/// siły obrońcy i atakującego (%).
pub fn enemy_cost(density: u64, defender_pct: u64, attacker_pct: u64) -> u64 {
    ENEMY_TILE_COST * MILLI + density * DEFENSE_PCT / 100 * defender_pct / attacker_pct.max(1)
}

/// Kolejność kafli frontu: deterministyczny skrót (SplitMix64) kafla i ticka – fala idzie równo po
/// całym froncie zamiast od jednego brzegu.
pub fn front_key(tile: usize, tick: u32) -> u64 {
    let mut z = (tile as u64) ^ ((tick as u64) << 32) ^ FRONT_SALT;
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Dlaczego nie można zaatakować prowincji (interfejs pokazuje powód).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum AttackBlock {
    /// Przed końcem wyboru startu albo gracz bez państwa.
    NotPlaying,
    /// Prowincja za tunelem (tunele – od centrum dowodzenia 3).
    Tunnel,
    /// Cała prowincja już nasza.
    Owned,
    /// Brak styku naszego terytorium z prowincją (przez rzekę – od centrum dowodzenia 2).
    NoContact,
    /// Trwa już inny atak (więcej naraz – od centrum dowodzenia 2).
    Limit,
    /// Za mało wojska.
    NoTroops,
}

/// Atak na prowincję dla ramki prowincji (z punktu widzenia oglądającego).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct AttackReport {
    /// `None` – można atakować (albo dosłać wojsko do trwającego ataku).
    pub blocked: Option<AttackBlock>,
    /// Kafle prowincji do zajęcia: niczyje i innych graczy.
    pub neutral_tiles: u32,
    pub enemy_tiles: u32,
    /// Szacunkowy koszt kafla (osoby wojska): niczyjego i cudzego (przy obecnym zaopatrzeniu).
    pub neutral_cost: f64,
    pub enemy_cost: f64,
    /// Wojsko dostępne do wysłania (osoby).
    pub available: f64,
    /// Trwający atak oglądającego: wojsko (osoby) i zajęte kafle.
    pub troops: Option<f64>,
    pub taken: u32,
}

/// Trwający atak dla paska zasobów.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct AttackInfo {
    pub province: u16,
    pub troops: f64,
    pub taken: u32,
}
