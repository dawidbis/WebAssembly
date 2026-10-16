//! Ekspansja i walka (docs/gdd/zaopatrzenie.md „Wojsko w walce”, docs/gdd/budynki.md „Centrum
//! dowodzenia”): stałe do strojenia, stan frontu i typy raportów. Zasady (front, koszty, zaopatrzenie)
//! są w `game.rs`.
//!
//! Front to wojsko wysłane na **jedną albo kilka prowincji** naraz (ile – zależy od centrum dowodzenia):
//! co turę zajmuje kafle tych prowincji stykające się (bokiem) z naszymi – niczyje za koszt walki
//! z armią tubylczą, cudze według gęstości wojska obrońcy i stosunku zaopatrzenia obu stron. Front
//! sięga też przez wąską rzekę, przez jezioro i – jako desant morski – przez morze. Zużyte wojsko
//! ginie, resztka wraca do puli, gdy front się kończy.

use serde::{Deserialize, Serialize};

use crate::{economy::MILLI, goods::Supply, owners::Nation};

/// Armia tubylcza każdej prowincji: `NATIVES_PER_TILE` osób na kafel × mnożnik infrastruktury startowej
/// (%; indeks = poziom 1–3) – słaba, ma spowalniać, nie blokować.
pub const NATIVES_PER_TILE: u64 = 8;
pub const NATIVES_INFRA_PCT: [u64; 4] = [100, 80, 100, 130];
/// Kafel niczyj na nizinie bez lasu: `NATIVE_TILE_COST` + gęstość tubylców na kafel × `NATIVE_DEFENSE_PCT`%
/// (osoby wojska); tubylcy tracą gęstość za każdy kafel. Teren mnoży koszt, punkty szybkości i ustala
/// kolejność (`Game::terrain_pct`: las 125%, wyżyny 150%, wyżyny z lasem 190%).
pub const NATIVE_TILE_COST: u64 = 3;
pub const NATIVE_DEFENSE_PCT: u64 = 25;
/// Najmniej wojska, jakie można wysłać na front (osoby).
pub const MIN_ATTACK_TROOPS: u64 = 20;
/// Kolejność frontu (`front_score`, mniej = wcześniej): trudność terenu (%) + „zawirowanie taktyczne”
/// (losowo 0..`TURBULENCE`) − `SURROUND_BONUS` za każdy nasz kafel wokół (8 sąsiadów) – kafle
/// otoczone znikają od razu, a front przesuwa się spójnie.
pub const TURBULENCE: u64 = 60;
pub const SURROUND_BONUS: i64 = 45;
/// Kafel innego gracza: `ENEMY_TILE_COST` + gęstość wojska obrońcy na kafel × `DEFENSE_PCT`% ×
/// (zaopatrzenie obrońcy / zaopatrzenie atakującego). Obrońca traci gęstość wojska za każdy kafel.
pub const ENEMY_TILE_COST: u64 = 20;
pub const DEFENSE_PCT: u64 = 200;
/// Desant (kafel zajmowany z wody – jezioro, morze): koszt i punkty szybkości × `LANDING_PCT`%.
pub const LANDING_PCT: u64 = 150;
/// Szybkość frontu: punkty na turę (kafel niczyj – `NEUTRAL_TILE_POINTS`, cudzy – `ENEMY_TILE_POINTS`).
pub const POINTS_PER_TICK: u64 = 30;
pub const NEUTRAL_TILE_POINTS: u64 = 10;
pub const ENEMY_TILE_POINTS: u64 = 20;
/// Paliwo przyspiesza front: × (1 + `FUEL_SPEED_PCT`% × pokrycie paliwem).
pub const FUEL_SPEED_PCT: u64 = 50;
/// Zaopatrzenie wojskowe w walce na 1 000 wojska na dzień (w tysięcznych sztuki) – zużywane tylko, gdy
/// front styka się z innym graczem.
pub const SUPPLY_PER_1000_PER_DAY: u64 = 1000;
/// Paliwo na 1 000 wojska na dzień (w tysięcznych sztuki) bez zaopatrzenia wojskowego (także na ziemi
/// niczyjej); z zaopatrzeniem – `Military::fuel`.
pub const FUEL_NO_SUPPLY: u64 = 50;
/// Od progu centrum dowodzenia rodzaj zaopatrzenia zużywa tyle % paliwa mniej.
pub const FUEL_SAVING_PCT: u64 = 40;
/// Konie jako zamiennik paliwa: 1 koń = `HORSE_FUEL_PCT`% sztuki paliwa, najwyżej `HORSE_MAX_PCT`% potrzeby.
pub const HORSE_FUEL_PCT: u64 = 50;
pub const HORSE_MAX_PCT: u64 = 50;
/// Współczynnik siły (%) bez zaopatrzenia wojskowego.
pub const NO_SUPPLY_PCT: u64 = 25;

/// Rodzaj zaopatrzenia wojskowego w walce: siła (%), od poziomu centrum dowodzenia `upgrade` – siła
/// `upgraded_pct` i paliwo × (100 − `FUEL_SAVING_PCT`)%; paliwo na 1 000 wojska na dzień (tysięczne).
#[derive(Clone, Copy, Debug)]
pub struct Military {
    pub supply: Supply,
    pub pct: u64,
    pub upgraded_pct: u64,
    pub upgrade: u8,
    pub fuel: u64,
}

/// Zaopatrzenie wojskowe od najlepszego (w tej kolejności zużywane).
pub const MILITARY: [Military; 3] = [
    Military { supply: Supply::HeavyMilitary, pct: 140, upgraded_pct: 160, upgrade: 6, fuel: 400 },
    Military { supply: Supply::StandardMilitary, pct: 100, upgraded_pct: 120, upgrade: 4, fuel: 200 },
    Military { supply: Supply::LightMilitary, pct: 60, upgraded_pct: 80, upgrade: 3, fuel: 100 },
];

impl Military {
    /// Siła (%) przy poziomie centrum dowodzenia `command`.
    pub fn strength(&self, command: u8) -> u64 {
        if command >= self.upgrade { self.upgraded_pct } else { self.pct }
    }

    /// Paliwo na 1 000 wojska na dzień (tysięczne) przy poziomie centrum dowodzenia `command`.
    pub fn fuel(&self, command: u8) -> u64 {
        if command >= self.upgrade { self.fuel * (100 - FUEL_SAVING_PCT) / 100 } else { self.fuel }
    }
}

/// Limity frontów według poziomu centrum dowodzenia (indeks = poziom; `usize::MAX` – bez limitu):
/// frontów naraz, prowincji na jednym froncie, desantów morskich naraz (desant to też front).
pub const MAX_FRONTS: [usize; 7] = [2, 2, 2, 3, usize::MAX, usize::MAX, usize::MAX];
pub const MAX_FRONT_PROVINCES: [usize; 7] = [1, 1, 2, 3, 4, usize::MAX, usize::MAX];
pub const MAX_SEA_LANDINGS: [usize; 7] = [0, 0, 1, usize::MAX, usize::MAX, usize::MAX, usize::MAX];
/// Przeprawa przez rzekę (od centrum dowodzenia 1): front sięga przez najwyżej tyle kafli rzeki w linii
/// prostej. Desant przez jezioro – też od 1.
pub const RIVER_CROSSING_TILES: usize = 4;
/// Ziarno kolejności zajmowania kafli frontu (XOR z kaflem i tickiem).
pub const FRONT_SALT: u64 = 0x4652_4f4e_5400_0001;

/// `Attack::supply`: front nie styka się z graczem (zaopatrzenie wojskowe nie jest zużywane).
pub const NOT_FIGHTING: u8 = u8::MAX;
/// `Attack::supply`: walka bez zaopatrzenia wojskowego.
pub const NO_SUPPLY: u8 = u8::MAX - 1;

/// Trwający front państwa na jedną albo kilka prowincji.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attack {
    pub nation: Nation,
    /// Prowincje frontu (zajęte w całości odpadają; front kończy się, gdy nie zostanie żadna).
    pub provinces: Vec<u16>,
    /// Wojsko na froncie (w tysięcznych osoby).
    pub troops: u64,
    /// Kafle zajęte na tym froncie.
    pub taken: u32,
    /// Zaopatrzenie wojskowe zużyte w ostatniej turze (najwięcej sztuk; `Supply as u8`, `NO_SUPPLY`,
    /// `NOT_FIGHTING`).
    pub supply: u8,
    /// Desant morski (front przez morze – wlicza się do `MAX_SEA_LANDINGS`).
    pub landing: bool,
}

impl Attack {
    pub fn hash_bytes(&self) -> impl Iterator<Item = u8> {
        let mut b = vec![self.nation, self.provinces.len() as u8];
        b.extend(self.provinces.iter().flat_map(|p| p.to_le_bytes()));
        b.extend(self.troops.to_le_bytes());
        b.extend(self.taken.to_le_bytes());
        b.extend([self.supply, self.landing as u8]);
        b.into_iter()
    }
}

/// Startowa armia tubylcza prowincji (w tysięcznych osoby) z jej wielkości i infrastruktury startowej.
pub fn natives(area: u32, infrastructure: u8) -> u64 {
    area as u64 * NATIVES_PER_TILE * MILLI * NATIVES_INFRA_PCT[(infrastructure as usize).min(3)] / 100
}

/// Koszt kafla niczyjego na nizinie (tysięczne osoby) przy gęstości tubylców `density` (tysięczne na kafel).
pub fn native_cost(density: u64) -> u64 {
    NATIVE_TILE_COST * MILLI + density * NATIVE_DEFENSE_PCT / 100
}

/// Kolejność kafla frontu (mniej = wcześniej): teren (%), nasze kafle wokół (0–8) i skrót `key`
/// (`front_key`) jako „zawirowanie taktyczne”.
pub fn front_score(terrain_pct: u64, around: u32, key: u64) -> i64 {
    (terrain_pct + key % (TURBULENCE + 1)) as i64 - SURROUND_BONUS * around as i64
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

/// Dlaczego nie można zaatakować (interfejs pokazuje powód).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum AttackBlock {
    /// Przed końcem wyboru startu albo gracz bez państwa.
    NotPlaying,
    /// Prowincja za tunelem (budowa tuneli – w przyszłości).
    Tunnel,
    /// Cała prowincja już nasza.
    Owned,
    /// Brak styku naszego terytorium z żadną z prowincji (ani przez rzekę, jezioro, morze).
    NoContact,
    /// Za dużo frontów naraz (więcej – od wyższego centrum dowodzenia).
    Limit,
    /// Za dużo prowincji na jednym froncie (więcej – od wyższego centrum dowodzenia).
    Provinces,
    /// Desant morski: od centrum dowodzenia 2 (jeden naraz), od 3 bez limitu.
    Landing,
    /// Prowincje należą do różnych trwających frontów.
    OtherFront,
    /// Za mało wojska.
    NoTroops,
}

/// Atak na prowincję dla ramki prowincji (z punktu widzenia oglądającego).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct AttackReport {
    /// `None` – można atakować (albo dosłać wojsko do trwającego frontu).
    pub blocked: Option<AttackBlock>,
    /// Kafle prowincji do zajęcia: niczyje i innych graczy.
    pub neutral_tiles: u32,
    pub enemy_tiles: u32,
    /// Szacunkowy koszt kafla na nizinie (osoby wojska): niczyjego (tubylcy) i cudzego (przy obecnym
    /// zaopatrzeniu).
    pub neutral_cost: f64,
    pub enemy_cost: f64,
    /// Armia tubylcza, która została w prowincji (osoby).
    pub natives: f64,
    /// Wojsko dostępne do wysłania (osoby).
    pub available: f64,
    /// Trwający front oglądającego z tą prowincją: wojsko (osoby), zajęte kafle, prowincje frontu.
    pub troops: Option<f64>,
    pub taken: u32,
    pub front: Vec<u16>,
}

/// Trwający front dla paska zasobów.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct AttackInfo {
    pub provinces: Vec<u16>,
    pub troops: f64,
    pub taken: u32,
}

/// Strona walki na froncie (obrońca w prowincjach frontu).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct FrontSide {
    pub player: u16,
    /// Wojsko (osoby): atakujący – na tym froncie, obrońca – w kraju (broni całego państwa).
    pub troops: f64,
    /// Zaopatrzenie wojskowe w użyciu (`None` – bez zaopatrzenia).
    pub supply: Option<Supply>,
    /// Kafle prowincji frontu tej strony.
    pub tiles: u32,
}

/// Aktywny front oglądającego (jego albo na jego prowincje) – panel frontów.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct FrontReport {
    pub provinces: Vec<u16>,
    pub attacker: FrontSide,
    /// Inni właściciele kafli prowincji frontu (pusta lista – ziemia niczyja).
    pub defenders: Vec<FrontSide>,
    /// Część prowincji frontu zajęta przez atakującego (0–1), kafle niczyje i tubylcy (osoby).
    pub occupation: f64,
    pub neutral_tiles: u32,
    pub natives: f64,
    /// Desant morski.
    pub landing: bool,
    /// Środek frontu (kafel) – „Idź do”.
    pub center_x: f64,
    pub center_y: f64,
    /// Czy atakuje oglądający.
    pub mine: bool,
}
