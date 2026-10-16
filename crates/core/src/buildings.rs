//! Budynki w prowincjach (docs/gdd/budynki.md): poziomy (nigdy nie maleją), infrastruktura (punkty na
//! budynki), koszt budowy (złoto + materiały inżynieryjne), czas budowy i obsada robotnikami.
//!
//! Budynki należą do prowincji – korzysta z nich jej właściciel (gracz z ponad 50% kafli,
//! `Owners::province_owner`). Zasady (walidacja budowy, rozliczenie tur) są w `game.rs`, tu – tabele
//! i stałe do strojenia oraz stan jednej prowincji.

use serde::{Deserialize, Serialize};

use crate::{
    economy::MILLI,
    factories::{IngredientReport, Inputs, NO_SHORTAGE},
    goods::{Supply, TICKS_PER_DAY},
    mapgen::{Biome, Province},
    war::AttackReport,
};

/// Rodzaj budynku.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum Building {
    /// Wydobycie surowca prowincji (kopalnia, pola uprawne… – typ z `goods::EXTRACTION`).
    Extraction,
    /// Fabryka zaopatrzenia (produkcja – etap 3).
    Factory,
    /// Miasto: +`CITY_POPULATION` do limitu populacji państwa za poziom.
    City,
    /// Uniwersytet: poziomy sumują się w państwie → progi technologii (etap 7).
    University,
    /// Umocnienia: poziom 1 od centrum dowodzenia 3, do 3 od CD 5 (`FORTIFICATION_CAP`).
    Fortifications,
    /// Port: tylko prowincja nad morzem; handel morski (etap 8).
    Port,
    /// Infrastruktura: punkty na budynki (`points`), sama ich nie zajmuje.
    Infrastructure,
    /// Centrum dowodzenia: tylko w stolicy (prowincja startowa), poziom 1 od startu, odblokowania
    /// (rodzaje zaopatrzenia, liczba ataków, przeprawy, umocnienia).
    CommandCenter,
}

pub const BUILDING_COUNT: usize = 8;

/// Wszystkie rodzaje w kolejności indeksów (`Building as usize`).
pub const BUILDINGS: [Building; BUILDING_COUNT] = [
    Building::Extraction,
    Building::Factory,
    Building::City,
    Building::University,
    Building::Fortifications,
    Building::Port,
    Building::Infrastructure,
    Building::CommandCenter,
];

/// Maksymalny poziom (kolejność `BUILDINGS`).
pub const MAX_LEVEL: [u8; BUILDING_COUNT] = [10, 10, 5, 5, 3, 3, 5, 6];
/// Punkty infrastruktury zajmowane przez poziom budynku.
pub const POINTS_PER_LEVEL: [u32; BUILDING_COUNT] = [10, 20, 25, 15, 10, 15, 0, 0];
/// Koszt poziomu 1 w złocie z interfejsu (w rdzeniu ×10); centrum dowodzenia – poziomu 2.
pub const BASE_GOLD: [u64; BUILDING_COUNT] = [10_000, 20_000, 40_000, 30_000, 15_000, 20_000, 15_000, 30_000];
/// Koszt poziomu 1 w materiałach inżynieryjnych (jednostki). Pierwszy poziom wydobycia i fabryki – bez
/// materiałów (`FREE_FIRST_LEVEL`), żeby start się nie zablokował.
pub const BASE_MATERIALS: [u64; BUILDING_COUNT] = [10, 20, 40, 30, 30, 25, 30, 30];
/// Budynki, których pierwszy poziom kosztuje tylko złoto.
pub const FREE_FIRST_LEVEL: [bool; BUILDING_COUNT] = [true, true, false, false, false, false, false, false];
/// Robotników do obsady jednego poziomu (osoby).
pub const STAFF_PER_LEVEL: [u64; BUILDING_COUNT] = [10_000, 20_000, 0, 15_000, 0, 5_000, 0, 0];
/// Koszt rośnie ×1,5 z każdym poziomem: `baza × 1,5^(poziom − 1)` – jako ułamek `GROWTH_NUM / GROWTH_DEN`.
const GROWTH_NUM: u64 = 3;
const GROWTH_DEN: u64 = 2;
/// Czas budowy poziomu: `BUILD_DAYS_BASE + BUILD_DAYS_PER_LEVEL × poziom` dni gry.
pub const BUILD_DAYS_BASE: u32 = 10;
pub const BUILD_DAYS_PER_LEVEL: u32 = 5;
/// Anulowanie budowy zwraca tyle procent zapłaconego kosztu.
pub const CANCEL_REFUND_PCT: u64 = 50;
/// Punkty infrastruktury: poziom 1 – `INFRA_BASE_POINTS`, każdy kolejny + `INFRA_POINTS_PER_LEVEL`.
pub const INFRA_BASE_POINTS: u32 = 100;
pub const INFRA_POINTS_PER_LEVEL: u32 = 50;
/// Startowy poziom infrastruktury: bazowo 2, korekty z cech prowincji, przycięty do 1–3.
pub const INFRA_START_BASE: i32 = 2;
pub const INFRA_START_MAX: u8 = 3;
/// Wydobycie za poziom budynku wydobycia: 1,0 jednostki na dzień (w tysięcznych).
pub const EXTRACTION_PER_LEVEL_PER_DAY: u64 = 1000;
/// Nawozy: zużycie na poziom pól uprawnych na dzień (w tysięcznych) i premia przy pełnym pokryciu (%).
pub const FERTILIZER_PER_LEVEL_PER_DAY: u64 = 200;
pub const FERTILIZER_BONUS_PCT: u64 = 50;
/// Najwyższy poziom umocnień przy poziomie centrum dowodzenia (indeks = poziom CD; 0 – niedostępne).
pub const FORTIFICATION_CAP: [u8; 7] = [0, 0, 0, 1, 1, 3, 3];
/// Miasto: + tyle osób do limitu populacji państwa za poziom.
pub const CITY_POPULATION: u64 = 25_000;

/// Trwająca budowa w prowincji (jedna naraz).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Construction {
    pub building: Building,
    /// Poziom, który powstanie.
    pub level: u8,
    /// Tury budowy już wykonane i potrzebne (budowa stoi, gdy prowincja nie ma właściciela).
    pub done: u32,
    pub total: u32,
    /// Zapłacone złoto (surowe) i materiały (w tysięcznych) – do zwrotu przy anulowaniu.
    pub gold: u64,
    pub materials: u64,
}

/// Budynki jednej prowincji.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProvinceBuildings {
    /// Poziom każdego rodzaju (indeks `Building as usize`); infrastruktura od startu ≥ 1.
    pub levels: [u8; BUILDING_COUNT],
    pub construction: Option<Construction>,
    /// Co produkuje fabryka (domyślnie materiały inżynieryjne – potrzebne do budowy).
    pub product: Supply,
    /// Tury przezbrojenia, które zostały (fabryka wtedy nie produkuje).
    pub retool: u32,
    /// Produkcja fabryki w ostatniej turze (w tysięcznych) i składnik, którego zabrakło
    /// (`factories::NO_SHORTAGE` – pełna).
    pub output: u64,
    pub short: u8,
    /// Wybrane wejścia składników (automatycznie – do wyczerpania, ręcznie – do zmiany przez gracza).
    pub inputs: Inputs,
}

impl ProvinceBuildings {
    pub fn new(infrastructure: u8) -> Self {
        let mut levels = [0; BUILDING_COUNT];
        levels[Building::Infrastructure as usize] = infrastructure;
        ProvinceBuildings {
            levels,
            construction: None,
            product: Supply::EngineeringMaterials,
            retool: 0,
            output: 0,
            short: NO_SHORTAGE,
            inputs: Inputs::default(),
        }
    }

    pub fn level(&self, building: Building) -> u8 {
        self.levels[building as usize]
    }

    /// Punkty infrastruktury prowincji (z jej poziomu).
    pub fn points(&self) -> u32 {
        points(self.level(Building::Infrastructure))
    }

    /// Punkty zajęte przez budynki (z trwającą budową – jej poziom już się liczy).
    pub fn used_points(&self) -> u32 {
        let built: u32 = BUILDINGS.iter().map(|&b| self.level(b) as u32 * POINTS_PER_LEVEL[b as usize]).sum();
        built + self.construction.as_ref().map_or(0, |c| POINTS_PER_LEVEL[c.building as usize])
    }

    /// Robotników potrzebnych do pełnej obsady (osoby).
    pub fn staff(&self) -> u64 {
        BUILDINGS.iter().map(|&b| self.level(b) as u64 * STAFF_PER_LEVEL[b as usize]).sum()
    }

    pub fn hash_bytes(&self) -> impl Iterator<Item = u8> + '_ {
        let c = self.construction.as_ref();
        let construction = c.map_or([0u8; 26], |c| {
            let mut b = [0u8; 26];
            b[0] = c.building as u8 + 1;
            b[1] = c.level;
            b[2..6].copy_from_slice(&c.done.to_le_bytes());
            b[6..10].copy_from_slice(&c.total.to_le_bytes());
            b[10..18].copy_from_slice(&c.gold.to_le_bytes());
            b[18..26].copy_from_slice(&c.materials.to_le_bytes());
            b
        });
        let mut factory = [0u8; 14];
        factory[0] = self.product as u8;
        factory[1..5].copy_from_slice(&self.retool.to_le_bytes());
        factory[5..13].copy_from_slice(&self.output.to_le_bytes());
        factory[13] = self.short;
        let inputs = self.inputs.choice.into_iter().chain([self.inputs.manual]);
        self.levels.into_iter().chain(construction).chain(factory).chain(inputs)
    }
}

/// Dlaczego nie można budować (interfejs pokazuje powód przy przycisku).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum BuildBlock {
    /// Przed końcem wyboru startu.
    NotPlaying,
    /// Gracz nie ma ponad 50% kafli prowincji.
    NotOwner,
    MaxLevel,
    /// W prowincji trwa inna budowa.
    Busy,
    /// Brak wolnych punktów infrastruktury.
    NoPoints,
    /// Wymaga wyższego centrum dowodzenia (umocnienia – od CD 3).
    Locked,
    /// Centrum dowodzenia tylko w stolicy.
    NotCapital,
    /// Port tylko nad morzem.
    NotCoastal,
    NoGold,
    NoMaterials,
}

/// Trwająca budowa dla interfejsu.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ConstructionReport {
    pub building: Building,
    pub level: u8,
    /// Tury wykonane i potrzebne; `paused` – prowincja bez właściciela (> 50% kafli), budowa stoi.
    pub done: u32,
    pub total: u32,
    pub paused: bool,
}

/// Następny poziom budynku: koszt, czas i czy gracz może go zbudować.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct BuildOption {
    pub building: Building,
    /// Obecny i maksymalny poziom.
    pub level: u8,
    pub max_level: u8,
    /// Koszt następnego poziomu: złoto z interfejsu (już ÷10) i materiały inżynieryjne (jednostki).
    pub gold: f64,
    pub materials: f64,
    pub days: u32,
    /// Punkty infrastruktury i robotnicy na poziom.
    pub points: u32,
    pub staff: f64,
    /// `None` – można budować.
    pub blocked: Option<BuildBlock>,
}

/// Budynki prowincji dla ramki prowincji (z punktu widzenia gracza `viewer`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ProvinceReport {
    pub province: u16,
    /// Startowy (bazowy) poziom infrastruktury – z cech prowincji.
    pub base_infrastructure: u8,
    /// Cena surowca prowincji (złoto z interfejsu za jednostkę; do rynku – cena bazowa, złoto: 0).
    pub price: f64,
    /// O ile prowincja podnosi limit populacji właściciela (osoby: jego kafle + miasta; 0 – niczyja).
    pub population_cap: f64,
    /// Punkty infrastruktury: wszystkie i zajęte.
    pub points: u32,
    pub used_points: u32,
    pub construction: Option<ConstructionReport>,
    /// Wszystkie rodzaje budynków (kolejność `BUILDINGS`) – poziomy i możliwość budowy.
    pub options: Vec<BuildOption>,
    /// Czy prowincja należy do oglądającego (może budować i anulować).
    pub mine: bool,
    /// Nazwa budynku wydobycia (z surowca prowincji, np. „Kopalnia”).
    pub extraction: String,
    /// Wydobycie prowincji na dzień (jednostki surowca; złoto – złoto z interfejsu); 0 – bez właściciela.
    pub yield_per_day: f64,
    pub factory: FactoryReport,
    pub attack: AttackReport,
}

/// Rodzaj zaopatrzenia do wyboru w fabryce.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ProductOption {
    pub supply: Supply,
    pub name: String,
    /// Receptura na 1 sztukę (np. „0,5 Cement + 0,3 Drewno + …”).
    pub recipe: String,
    /// Wymagany poziom centrum dowodzenia, gdy rodzaj jest jeszcze niedostępny (`None` – dostępny).
    pub command_level: Option<u8>,
}

/// Fabryka prowincji dla interfejsu.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct FactoryReport {
    pub product: Supply,
    /// Dni przezbrojenia, które zostały (0 – produkuje).
    pub retool_days: f64,
    /// Produkcja na dzień: ostatnia (sztuki) i pełna przy tej obsadzie, gdyby starczyło surowców.
    pub output_per_day: f64,
    pub capacity_per_day: f64,
    /// Składnik, którego zabrakło – wejścia do wyboru (np. „Cement” albo „Paliwo | Konie”).
    pub missing: Option<String>,
    /// Numer składnika, którego zabrakło.
    pub short: Option<u8>,
    /// Składniki produktu z wejściem, które fabryka bierze teraz.
    pub ingredients: Vec<IngredientReport>,
    pub products: Vec<ProductOption>,
}

/// Punkty infrastruktury na poziomie `level` (0 – brak).
pub fn points(level: u8) -> u32 {
    if level == 0 { 0 } else { INFRA_BASE_POINTS + (level as u32 - 1) * INFRA_POINTS_PER_LEVEL }
}

/// Koszt budowy poziomu `level` (od 1): złoto surowe (interfejs ÷10) i materiały w tysięcznych.
pub fn cost(building: Building, level: u8) -> (u64, u64) {
    let k = building as usize;
    if building == Building::CommandCenter {
        // Poziom 2: baza, każdy kolejny ×2 (poziom 1 stolica ma od startu).
        let double = 1u64 << level.saturating_sub(2);
        return (BASE_GOLD[k] * 10 * double, BASE_MATERIALS[k] * MILLI * double);
    }
    let mut gold = BASE_GOLD[k] * 10;
    let mut materials = if level == 1 && FREE_FIRST_LEVEL[k] { 0 } else { BASE_MATERIALS[k] * MILLI };
    for _ in 1..level {
        gold = gold * GROWTH_NUM / GROWTH_DEN;
        materials = materials * GROWTH_NUM / GROWTH_DEN;
    }
    (gold, materials)
}

/// Tury budowy poziomu `level`.
pub fn build_ticks(level: u8) -> u32 {
    (BUILD_DAYS_BASE + BUILD_DAYS_PER_LEVEL * level as u32) * TICKS_PER_DAY as u32
}

/// Startowy poziom infrastruktury z cech prowincji: bazowo 2; −1 za trudny biom (pustynia, lądolód,
/// tundra, las deszczowy); −1 za przewagę wyżyn albo sąsiedztwo gór; +1 za rzekę albo morze; 1–3.
pub fn start_infrastructure(province: &Province, highland_tiles: u32) -> u8 {
    let hard =
        [Biome::Desert, Biome::IceSheet, Biome::Tundra, Biome::Rainforest].iter().any(|&b| province.biome == b as u8);
    let rough = province.mountains || highland_tiles * 2 > province.area;
    let water = province.river || province.coastal;
    let level = INFRA_START_BASE - hard as i32 - rough as i32 + water as i32;
    level.clamp(1, INFRA_START_MAX as i32) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn costs_grow_by_half_and_first_extraction_needs_only_gold() {
        assert_eq!(cost(Building::Extraction, 1), (100_000, 0));
        assert_eq!(cost(Building::Extraction, 2), (150_000, 15 * MILLI));
        assert_eq!(cost(Building::Extraction, 3), (225_000, 22_500));
        assert_eq!(cost(Building::Factory, 1), (200_000, 0));
        assert_eq!(cost(Building::City, 1), (400_000, 40 * MILLI));
        // Czas: 10 + 5 × poziom dni.
        assert_eq!(build_ticks(1), 150);
        assert_eq!(build_ticks(10), 600);
    }

    #[test]
    fn infrastructure_points_and_usage() {
        assert_eq!((points(1), points(2), points(5)), (100, 150, 300));
        let mut b = ProvinceBuildings::new(1);
        b.levels[Building::Extraction as usize] = 3;
        b.levels[Building::Factory as usize] = 2;
        b.levels[Building::City as usize] = 1;
        assert_eq!(b.used_points(), 95, "przykład z GDD");
        assert_eq!(b.staff(), 3 * 10_000 + 2 * 20_000);
    }

    #[test]
    fn start_infrastructure_follows_province_features() {
        let p = |biome: Biome, coastal: bool, mountains: bool| Province {
            biome: biome as u8,
            coastal,
            mountains,
            area: 100,
            ..Default::default()
        };
        assert_eq!(start_infrastructure(&p(Biome::Steppe, false, false), 0), 2);
        assert_eq!(start_infrastructure(&p(Biome::Steppe, true, false), 0), 3);
        assert_eq!(start_infrastructure(&p(Biome::Desert, false, true), 0), 1, "przycięte do 1");
        assert_eq!(start_infrastructure(&p(Biome::Steppe, false, false), 60), 1, "przewaga wyżyn");
        assert_eq!(start_infrastructure(&p(Biome::Tundra, true, false), 0), 2);
    }
}
