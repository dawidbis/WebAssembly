//! Generator map z seeda. Czysty Rust, zero zależności od webu.
//! Ten sam seed + te same parametry + ta sama wersja generatora = identyczna mapa
//! (natywnie i w wasm), bo używamy własnego RNG i `libm` w fastnoise-lite.

mod biome;
mod hydro;
mod layout;
mod mountains;
mod ocean;
mod provinces;
mod vegetation;
mod relief;
mod util;

use serde::{Deserialize, Serialize};

pub use biome::{dominant_kind, kind_weights};
pub use provinces::{Province, Provinces};

/// Zwiększaj przy każdej zmianie algorytmu – stare seedy dają wtedy inne mapy,
/// więc wersja musi trafić do konfiguracji gry i do replayów.
pub const GENERATOR_VERSION: u32 = 14;

/// Typy kafli. Wartości muszą zgadzać się z `web/src/app/render/terrain.ts`.
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Terrain {
    Ocean = 0,
    Lake = 1,
    River = 2,
    Plains = 3,
    Highlands = 4,
    Mountains = 5,
}

impl Terrain {
    pub fn is_land(self) -> bool {
        matches!(self, Terrain::Plains | Terrain::Highlands | Terrain::Mountains)
    }
}

/// Typy klimatu – grupy biomów. Kontynent dostaje jeden typ (albo dwa), a rodzaj biomu
/// wewnątrz typu wynika z chłodu i wilgotności kafla. Szanse i pary działają na poziomie typów.
/// Wartości muszą zgadzać się z `web/src/app/render/terrain.ts`.
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BiomeType {
    Tropical = 0,
    Dry = 1,
    Temperate = 2,
    Continental = 3,
    Polar = 4,
}

impl BiomeType {
    pub const ALL: [BiomeType; 5] =
        [BiomeType::Tropical, BiomeType::Dry, BiomeType::Temperate, BiomeType::Continental, BiomeType::Polar];

    /// Numer bitu pary typów w `MapGenParams::biome_pairs` (kolejność jak `BIOME_PAIRS`).
    /// `None` dla tego samego typu.
    pub fn pair_bit(a: BiomeType, b: BiomeType) -> Option<u32> {
        let (a, b) = if (a as u8) < (b as u8) { (a, b) } else { (b, a) };
        BIOME_PAIRS.iter().position(|&pair| pair == (a, b)).map(|i| i as u32)
    }
}

/// Rodzaje biomów (to one są zapisane na kaflu). Kolejność: pogrupowane według typu.
/// Wartości muszą zgadzać się z `web/src/app/render/terrain.ts`.
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Biome {
    /// Tropikalny: las deszczowy.
    Rainforest = 0,
    /// Tropikalny: sawanna.
    Savanna = 1,
    /// Suchy: pustynia.
    Desert = 2,
    /// Suchy: step.
    Steppe = 3,
    /// Umiarkowany: śródziemnomorski.
    Mediterranean = 4,
    /// Umiarkowany: subtropikalny (wilgotny).
    Subtropical = 5,
    /// Umiarkowany: oceaniczny.
    Oceanic = 6,
    /// Kontynentalny z gorącym latem.
    HotSummer = 7,
    /// Kontynentalny z ciepłym latem.
    WarmSummer = 8,
    /// Kontynentalny: borealny (tajga).
    Boreal = 9,
    /// Polarny: tajga przyprószona śniegiem.
    Taiga = 10,
    /// Polarny: tundra.
    Tundra = 11,
    /// Polarny: lądolód.
    IceSheet = 12,
}

impl Biome {
    pub const COUNT: usize = 13;
    pub const ALL: [Biome; Biome::COUNT] = [
        Biome::Rainforest,
        Biome::Savanna,
        Biome::Desert,
        Biome::Steppe,
        Biome::Mediterranean,
        Biome::Subtropical,
        Biome::Oceanic,
        Biome::HotSummer,
        Biome::WarmSummer,
        Biome::Boreal,
        Biome::Taiga,
        Biome::Tundra,
        Biome::IceSheet,
    ];

    /// Typ klimatu, do którego należy rodzaj.
    pub fn kind_of(self) -> BiomeType {
        match self {
            Biome::Rainforest | Biome::Savanna => BiomeType::Tropical,
            Biome::Desert | Biome::Steppe => BiomeType::Dry,
            Biome::Mediterranean | Biome::Subtropical | Biome::Oceanic => BiomeType::Temperate,
            Biome::HotSummer | Biome::WarmSummer | Biome::Boreal => BiomeType::Continental,
            Biome::Taiga | Biome::Tundra | Biome::IceSheet => BiomeType::Polar,
        }
    }
}

impl BiomeType {
    /// Rodzaje typu w kolejności `Biome` (bit j maski wariantu = j-ty rodzaj).
    pub fn kinds(self) -> &'static [Biome] {
        match self {
            BiomeType::Tropical => &[Biome::Rainforest, Biome::Savanna],
            BiomeType::Dry => &[Biome::Desert, Biome::Steppe],
            BiomeType::Temperate => &[Biome::Mediterranean, Biome::Subtropical, Biome::Oceanic],
            BiomeType::Continental => &[Biome::HotSummer, Biome::WarmSummer, Biome::Boreal],
            BiomeType::Polar => &[Biome::Taiga, Biome::Tundra, Biome::IceSheet],
        }
    }
}

/// Liczba wag wariantów w `MapGenParams::biome_variants`.
pub const VARIANT_COUNT: usize = 27;

/// Indeks wagi wariantu w `MapGenParams::biome_variants`: typy po kolei (`BiomeType::ALL`),
/// w typie maski 1..2^n − 1 (bit j = j-ty rodzaj z `BiomeType::kinds`).
/// Kolejność musi zgadzać się z `VARIANTS` w `web/src/app/render/terrain.ts`.
pub fn variant_index(t: BiomeType, mask: u8) -> usize {
    let offset: usize = BiomeType::ALL[..t as usize].iter().map(|b| (1usize << b.kinds().len()) - 1).sum();
    offset + mask as usize - 1
}

/// Domyślne wagi wariantów: wszystkie rodzaje 1.0, pozostałe warianty po 0.15.
pub fn default_variants() -> Vec<f32> {
    let mut v = Vec::with_capacity(VARIANT_COUNT);
    for t in BiomeType::ALL {
        let full = (1u8 << t.kinds().len()) - 1;
        for mask in 1..=full {
            v.push(if mask == full { 1.0 } else { 0.15 });
        }
    }
    v
}

/// Wszystkie pary typów. Indeks pary = numer bitu w `MapGenParams::biome_pairs`.
/// Kolejność musi zgadzać się z `BIOME_PAIRS` w `web/src/app/render/terrain.ts`.
pub const BIOME_PAIRS: [(BiomeType, BiomeType); 10] = [
    (BiomeType::Tropical, BiomeType::Dry),
    (BiomeType::Tropical, BiomeType::Temperate),
    (BiomeType::Tropical, BiomeType::Continental),
    (BiomeType::Tropical, BiomeType::Polar),
    (BiomeType::Dry, BiomeType::Temperate),
    (BiomeType::Dry, BiomeType::Continental),
    (BiomeType::Dry, BiomeType::Polar),
    (BiomeType::Temperate, BiomeType::Continental),
    (BiomeType::Temperate, BiomeType::Polar),
    (BiomeType::Continental, BiomeType::Polar),
];

/// Domyślne zasady łączenia typów – sąsiedzi w klimacie: tropikalny z suchym i umiarkowanym,
/// suchy z umiarkowanym i kontynentalnym, umiarkowany z kontynentalnym, kontynentalny z polarnym.
pub const DEFAULT_BIOME_PAIRS: u32 = {
    let allowed = [0, 1, 4, 5, 7, 9];
    let mut mask = 0;
    let mut i = 0;
    while i < allowed.len() {
        mask |= 1 << allowed[i];
        i += 1;
    }
    mask
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase", default)]
pub struct MapGenParams {
    pub seed: u32,
    pub width: u32,
    pub height: u32,
    /// Siatka makro-chunków: każdy chunk jest lądowy albo wodny.
    pub chunk_cols: u32,
    pub chunk_rows: u32,
    /// Liczba kontynentów (1 = jeden duży ląd).
    pub continents: u32,
    /// Udział chunków lądowych (reszta to chunki wodne, czyli duże przerwy z wodą).
    pub land_ratio: f32,
    /// 0 = kontynenty podobnej wielkości, 1 = duże różnice.
    pub size_variance: f32,
    /// Poszarpanie wybrzeża i odkształcenie kształtu kontynentów.
    pub coast_roughness: f32,
    /// Wyspy mniejsze niż tyle kafli są usuwane.
    pub min_island_area: u32,
    /// Ląd nie może dotykać krawędzi mapy ani granicy chunków wodnych.
    pub keep_off_edges: bool,
    /// Minimalny odstęp w kaflach przy `keep_off_edges`.
    pub edge_margin: u32,
    /// Udział gór i wyżyn w lądzie.
    pub mountain_share: f32,
    pub highland_share: f32,
    /// Skala łańcuchów górskich (większa = dłuższe, szersze pasma).
    pub range_scale: f32,
    pub rivers: bool,
    pub river_count: u32,
    pub lakes: bool,
    /// Ile skupisk jezior (0..1).
    pub lake_amount: f32,
    pub min_lake_area: u32,
    pub max_lake_area: u32,
    /// Wyłączone = cały ląd w jednym biomie (umiarkowany oceaniczny).
    pub biomes: bool,
    /// Szansa na typ klimatu (wagi względne, normalizowane do sumy; 0 = typ nie występuje).
    pub biome_tropical: f32,
    pub biome_dry: f32,
    pub biome_temperate: f32,
    pub biome_continental: f32,
    pub biome_polar: f32,
    /// 0 = typy losowe, 1 = typ wynika z położenia względem biegunów klimatu: biegun zimna
    /// przy górnej albo dolnej krawędzi, biegun ciepła naprzeciwko.
    pub biome_latitude: f32,
    /// Szansa, że kontynent ma dwa typy klimatu.
    pub biome_mix_chance: f32,
    /// Które pary typów mogą wystąpić razem na jednym kontynencie – maska bitowa, bit = indeks w `BIOME_PAIRS`.
    pub biome_pairs: u32,
    /// Udział drugiego typu w kontynencie (średnio; losowane ±25%).
    pub biome_secondary_share: f32,
    /// Szerokość strefy przejścia między typami w kaflach.
    pub biome_transition: u32,
    /// Pofalowanie granicy typów i przeplatanie się płatów w strefie przejścia.
    pub biome_roughness: f32,
    /// Udziały rodzajów w obszarze typu na kontynencie (percentyl, więc nie zależą od seeda).
    /// Tropikalny: las deszczowy (najwilgotniejsza część), reszta sawanna.
    pub biome_rainforest_share: f32,
    /// Suchy: pustynia (najsuchsza część, w głębi lądu), reszta step.
    pub biome_desert_share: f32,
    /// Umiarkowany: oceaniczny (najchłodniejsza część); z reszty śródziemnomorski
    /// (suchsza część, ten udział), a pozostałe subtropikalny.
    pub biome_oceanic_share: f32,
    pub biome_mediterranean_share: f32,
    /// Kontynentalny: gorące lato (najcieplejsza część) i borealny (najzimniejsza), środek – ciepłe lato.
    pub biome_hot_summer_share: f32,
    pub biome_boreal_share: f32,
    /// Polarny: tajga (najcieplejsza część) i lądolód (najzimniejsza), środek – tundra.
    pub biome_polar_taiga_share: f32,
    pub biome_ice_share: f32,
    /// Jak bardzo suchość zależy od odległości od morza (1 = tylko odległość: wybrzeża zawsze
    /// wilgotne; 0 = tylko wielkoskalowy szum: pustynia czy sawanna mogą sięgać morza,
    /// a cała wyspa może być jednym rodzajem).
    pub biome_coast_influence: f32,
    /// Szerokość przejścia między rodzajami w kaflach.
    pub biome_kind_transition: u32,
    /// Pofalowanie granic rodzajów.
    pub biome_kind_roughness: f32,
    /// Szanse (wagi) wariantów typów: wariant = zbiór rodzajów typu, które występują w jego
    /// obszarze na kontynencie (od jednego do wszystkich). Układ – patrz [`variant_index`];
    /// 27 wag: tropikalny 3, suchy 3, umiarkowany 7, kontynentalny 7, polarny 7.
    pub biome_variants: Vec<f32>,
    /// Lód morski: kafle oceanu do tylu kafli od lądolodu zamarzają (0 = brak).
    pub ice_shelf_width: u32,
    /// Średnia szerokość płytkiego szelfu przy brzegu (kafle).
    pub shelf_width: u32,
    /// Zmienność szerokości szelfu: 0 = równy pas wokół lądu, 1 = szerokie ławice obok urwisk.
    pub shelf_variation: f32,
    /// Stromość stoku kontynentalnego: 1 = ostre urwisko, 0 = łagodny spadek.
    pub slope_steepness: f32,
    /// Rzeźba dna: podwodne grzbiety, rowy i góry podwodne.
    pub seabed_relief: f32,
    /// Lasy (wyłączone = brak lasów).
    pub forest: bool,
    /// Docelowy udział lasu w lądzie danego rodzaju biomu (bez gór).
    pub forest_rainforest: f32,
    pub forest_savanna: f32,
    pub forest_desert: f32,
    pub forest_steppe: f32,
    pub forest_mediterranean: f32,
    pub forest_subtropical: f32,
    pub forest_oceanic: f32,
    pub forest_hot_summer: f32,
    pub forest_warm_summer: f32,
    pub forest_boreal: f32,
    pub forest_taiga: f32,
    pub forest_tundra: f32,
    pub forest_ice_sheet: f32,
    /// Zwartość lasów: 0 = drobne, rozproszone kępy, 1 = duże zwarte masywy.
    pub forest_clumping: f32,
    /// Jak mocno las ciągnie do wody (rzeki, jeziora, wybrzeże).
    pub forest_moisture: f32,
    /// Podział lądu na prowincje.
    pub provinces: bool,
    /// Docelowa (średnia) wielkość prowincji w kaflach (z rzekami; góry są niczyje).
    pub province_size: f32,
    /// Najmniejsza i największa prowincja w kaflach.
    pub province_min_size: u32,
    pub province_max_size: u32,
    /// Jak mocno granice trzymają się rzek i grzbietów górskich (0 = wcale).
    pub province_natural_borders: f32,
    /// Nieregularność granic (0 = gładkie, zaokrąglone prowincje).
    pub province_roughness: f32,
    /// Dokładność wyrównania wartości: liczba rund na siatce zgrubnej (więcej = równiejsze
    /// wartości, ale wolniej; 14 ≈ odchylenie 12%, 28 ≈ 11% i ok. 40% dłużej).
    pub province_rounds: u32,
}

impl Default for MapGenParams {
    fn default() -> Self {
        Self {
            seed: 1,
            width: 1400,
            height: 1400,
            chunk_cols: 10,
            chunk_rows: 10,
            continents: 3,
            land_ratio: 0.65,
            size_variance: 0.5,
            coast_roughness: 0.5,
            min_island_area: 50,
            keep_off_edges: false,
            edge_margin: 12,
            mountain_share: 0.12,
            highland_share: 0.22,
            range_scale: 2.0,
            rivers: true,
            river_count: 30,
            lakes: true,
            lake_amount: 0.5,
            min_lake_area: 40,
            max_lake_area: 2500,
            biomes: true,
            biome_tropical: 0.4,
            biome_dry: 0.45,
            biome_temperate: 0.6,
            biome_continental: 0.55,
            biome_polar: 0.35,
            biome_latitude: 0.6,
            biome_mix_chance: 0.5,
            biome_pairs: DEFAULT_BIOME_PAIRS,
            biome_secondary_share: 0.4,
            biome_transition: 60,
            biome_roughness: 0.5,
            biome_rainforest_share: 0.5,
            biome_desert_share: 0.5,
            biome_oceanic_share: 0.35,
            biome_mediterranean_share: 0.5,
            biome_hot_summer_share: 0.33,
            biome_boreal_share: 0.33,
            biome_polar_taiga_share: 0.3,
            biome_ice_share: 0.45,
            biome_coast_influence: 0.4,
            biome_kind_transition: 40,
            biome_kind_roughness: 0.5,
            biome_variants: default_variants(),
            ice_shelf_width: 8,
            shelf_width: 14,
            shelf_variation: 0.6,
            slope_steepness: 0.7,
            seabed_relief: 0.5,
            forest: true,
            forest_rainforest: 0.95,
            forest_savanna: 0.12,
            forest_desert: 0.03,
            forest_steppe: 0.08,
            forest_mediterranean: 0.25,
            forest_subtropical: 0.55,
            forest_oceanic: 0.4,
            forest_hot_summer: 0.35,
            forest_warm_summer: 0.5,
            forest_boreal: 0.75,
            forest_taiga: 0.75,
            forest_tundra: 0.04,
            forest_ice_sheet: 0.0,
            forest_clumping: 0.85,
            forest_moisture: 0.5,
            provinces: true,
            province_size: 600.0,
            province_min_size: 120,
            province_max_size: 4000,
            province_natural_borders: 0.6,
            province_roughness: 0.5,
            province_rounds: 14,
        }
    }
}

impl MapGenParams {
    /// Przycina parametry do bezpiecznych zakresów (UI może wysłać cokolwiek).
    pub fn sanitized(&self) -> Self {
        let mut p = self.clone();
        p.width = p.width.clamp(64, 4096);
        p.height = p.height.clamp(64, 4096);
        p.chunk_cols = p.chunk_cols.clamp(1, 32).min(p.width / 16);
        p.chunk_rows = p.chunk_rows.clamp(1, 32).min(p.height / 16);
        p.continents = p.continents.clamp(1, 16);
        p.land_ratio = p.land_ratio.clamp(0.05, 1.0);
        p.size_variance = p.size_variance.clamp(0.0, 1.0);
        p.coast_roughness = p.coast_roughness.clamp(0.0, 1.0);
        p.mountain_share = p.mountain_share.clamp(0.0, 0.5);
        p.highland_share = p.highland_share.clamp(0.0, 1.0 - p.mountain_share);
        p.range_scale = p.range_scale.clamp(0.2, 4.0);
        p.lake_amount = p.lake_amount.clamp(0.0, 1.0);
        p.max_lake_area = p.max_lake_area.max(p.min_lake_area);
        for w in [
            &mut p.biome_tropical,
            &mut p.biome_dry,
            &mut p.biome_temperate,
            &mut p.biome_continental,
            &mut p.biome_polar,
            &mut p.biome_rainforest_share,
            &mut p.biome_desert_share,
            &mut p.biome_oceanic_share,
            &mut p.biome_mediterranean_share,
            &mut p.biome_hot_summer_share,
            &mut p.biome_boreal_share,
            &mut p.biome_polar_taiga_share,
            &mut p.biome_ice_share,
            &mut p.biome_coast_influence,
            &mut p.biome_kind_roughness,
        ] {
            *w = w.clamp(0.0, 1.0);
        }
        p.biome_kind_transition = p.biome_kind_transition.clamp(2, 1000);
        p.biome_variants.resize(VARIANT_COUNT, 0.0);
        for w in p.biome_variants.iter_mut() {
            *w = w.clamp(0.0, 1.0);
        }
        p.ice_shelf_width = p.ice_shelf_width.min(60);
        p.biome_latitude = p.biome_latitude.clamp(0.0, 1.0);
        p.biome_mix_chance = p.biome_mix_chance.clamp(0.0, 1.0);
        p.biome_pairs &= (1 << BIOME_PAIRS.len()) - 1;
        p.biome_secondary_share = p.biome_secondary_share.clamp(0.05, 0.5);
        p.biome_transition = p.biome_transition.clamp(2, 1000);
        p.biome_roughness = p.biome_roughness.clamp(0.0, 1.0);
        p.shelf_width = p.shelf_width.clamp(1, 200);
        p.shelf_variation = p.shelf_variation.clamp(0.0, 1.0);
        p.slope_steepness = p.slope_steepness.clamp(0.0, 1.0);
        p.seabed_relief = p.seabed_relief.clamp(0.0, 1.0);
        for s in [
            &mut p.forest_rainforest,
            &mut p.forest_savanna,
            &mut p.forest_desert,
            &mut p.forest_steppe,
            &mut p.forest_mediterranean,
            &mut p.forest_subtropical,
            &mut p.forest_oceanic,
            &mut p.forest_hot_summer,
            &mut p.forest_warm_summer,
            &mut p.forest_boreal,
            &mut p.forest_taiga,
            &mut p.forest_tundra,
            &mut p.forest_ice_sheet,
            &mut p.forest_clumping,
            &mut p.forest_moisture,
        ] {
            *s = s.clamp(0.0, 1.0);
        }
        p.province_size = p.province_size.clamp(20.0, 100_000.0);
        p.province_min_size = p.province_min_size.clamp(1, 100_000);
        p.province_max_size = p.province_max_size.clamp(p.province_min_size, 1_000_000);
        p.province_natural_borders = p.province_natural_borders.clamp(0.0, 1.0);
        p.province_roughness = p.province_roughness.clamp(0.0, 1.0);
        p.province_rounds = p.province_rounds.clamp(2, 60);
        p
    }

    /// Docelowe udziały lasu w kolejności `Biome::ALL`.
    pub fn forest_shares(&self) -> [f32; Biome::COUNT] {
        [
            self.forest_rainforest,
            self.forest_savanna,
            self.forest_desert,
            self.forest_steppe,
            self.forest_mediterranean,
            self.forest_subtropical,
            self.forest_oceanic,
            self.forest_hot_summer,
            self.forest_warm_summer,
            self.forest_boreal,
            self.forest_taiga,
            self.forest_tundra,
            self.forest_ice_sheet,
        ]
    }

    /// Czy dwa różne typy klimatu mogą wystąpić na jednym kontynencie.
    pub fn biomes_can_mix(&self, a: BiomeType, b: BiomeType) -> bool {
        BiomeType::pair_bit(a, b).is_some_and(|bit| self.biome_pairs & (1 << bit) != 0)
    }

    /// Szanse (wagi) typów w kolejności `BiomeType::ALL`.
    pub fn biome_weights(&self) -> [f32; 5] {
        [self.biome_tropical, self.biome_dry, self.biome_temperate, self.biome_continental, self.biome_polar]
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct MapStats {
    pub land_share: f32,
    pub plains_share: f32,
    pub highlands_share: f32,
    pub mountains_share: f32,
    pub continents: u32,
    pub removed_islands: u32,
    pub lakes: u32,
    pub rivers: u32,
    /// Udział biomów w lądzie (biom dominujący kafla), w kolejności `Biome::ALL`.
    pub biome_shares: Vec<f32>,
    /// Ile kontynentów ma dwa typy klimatu.
    pub mixed_continents: u32,
    /// Udział lasu w lądzie (gęstość ≥ 128).
    pub forest_share: f32,
    /// Liczba prowincji.
    pub provinces: u32,
    /// Powierzchnia prowincji w kaflach: średnia, najmniejsza, największa, odchylenie standardowe.
    pub province_area_mean: f32,
    pub province_area_min: u32,
    pub province_area_max: u32,
    pub province_area_std: f32,
}

/// Wynik generatora. `terrain` i `shade` mają rozmiar `width * height`, wiersz po wierszu.
#[derive(Clone, Debug, Default)]
pub struct MapData {
    pub width: u32,
    pub height: u32,
    pub chunk_cols: u32,
    pub chunk_rows: u32,
    /// 1 = chunk wodny, 0 = lądowy (`chunk_cols * chunk_rows`).
    pub water_chunks: Vec<u8>,
    /// Wartości `Terrain as u8`.
    pub terrain: Vec<u8>,
    /// Ląd: wysokość 0..255. Ocean: głębokość 0..255. Pozostałe: 0.
    pub shade: Vec<u8>,
    /// Biom dominujący kafla (`Biome as u8`, rodzaj o największej wadze) – to on liczy się
    /// w rozgrywce. Woda dostaje biom najbliższego lądu.
    pub biome: Vec<u8>,
    /// Do płynnych przejść (wygląd, lasy): 6 bajtów na kafel – [typ, σ1, σ2] typu głównego
    /// kontynentu i [typ, σ1, σ2] drugiego; wagi rodzajów liczy [`kind_weights`].
    pub biome_layers: Vec<u8>,
    /// Udział drugiego typu w kaflu: 0..255.
    pub biome_mix: Vec<u8>,
    /// 1 = zamarznięty kafel oceanu (lód morski przy lądolodzie, do `ice_shelf_width` kafli od
    /// lądu). Nadal ocean – nieprzechodni dla jednostek lądowych; zmienia wygląd i fale.
    pub sea_ice: Vec<u8>,
    /// Gęstość lasu 0..255 (≥ 128 = las). Typ lasu wynika z biomu kafla.
    pub forest: Vec<u8>,
    /// Kafle rzek: odległość do ujścia wzdłuż nurtu (maleje z prądem), 0 = nie rzeka.
    /// Tylko do animacji nurtu – nie wchodzi do hashy.
    pub river_flow: Vec<u16>,
    /// Numer prowincji kafla (od 1), 0 = brak. Bez prowincji są woda (ocean, jeziora, rzeki)
    /// i góry – przechodni jest tylko kafel z prowincją.
    pub province: Vec<u16>,
    /// Prowincje w kolejności numerów (`provinces[id - 1]`).
    pub provinces: Vec<Province>,
    pub stats: MapStats,
}

/// Pierwszy kafel chunka `c` przy podziale `size` kafli na `count` chunków.
/// Kafel `x` należy do chunka `x * count / size`.
pub fn chunk_start(c: u32, count: u32, size: u32) -> u32 {
    (c * size).div_ceil(count)
}

/// Pełna mapa razem z prowincjami. W przeglądarce generowanie jest dwufazowe
/// (`generate_base`, potem `generate_provinces`), żeby teren pojawił się wcześniej –
/// wynik jest identyczny.
pub fn generate(params: &MapGenParams) -> MapData {
    let (mut map, input) = generate_base(params);
    generate_provinces(&input).apply(&mut map);
    map
}

/// Dane potrzebne do policzenia prowincji w drugiej fazie generowania.
pub struct ProvinceInput {
    params: MapGenParams,
    terrain: Vec<Terrain>,
    shade: Vec<u8>,
    biome: Vec<u8>,
    blocked: Vec<bool>,
}

/// Faza 1: wszystko poza prowincjami (`province` wypełnione zerami, `provinces` puste).
pub fn generate_base(params: &MapGenParams) -> (MapData, ProvinceInput) {
    let p = params.sanitized();
    let mut rng = util::Rng::new(p.seed as u64);

    let layout = layout::build(&p, &mut rng);
    let mut relief = relief::build(&p, &layout, &mut rng);
    // Biomy mają własny RNG, więc ich ustawienia nie zmieniają kształtu terenu, rzek ani jezior.
    let biomes = biome::build(&p, &layout, &relief);
    let (lakes, rivers) = hydro::build(&p, &layout, &mut relief, &mut rng);
    // Góry nieprzechodnie i niczyje (maleńkie kieszenie w górach stają się górami).
    let mountains = mountains::build(&p, &mut relief.terrain);
    // Dno oceanu – osobny RNG, więc nie zmienia terenu ani biomów.
    ocean::build(&p, &layout, &relief.terrain, &mut relief.shade);
    // Lód morski: ocean blisko lądu, którego biom (dziedziczony z najbliższego lądu) to lądolód.
    let sea_ice: Vec<u8> = if p.ice_shelf_width == 0 {
        vec![0; relief.terrain.len()]
    } else {
        let (w, h) = (p.width as usize, p.height as usize);
        let dist = util::distance_field(w, h, |i| relief.terrain[i] != Terrain::Ocean);
        (0..w * h)
            .map(|i| {
                (relief.terrain[i] == Terrain::Ocean
                    && dist[i] <= p.ice_shelf_width as f32
                    && biomes.dominant[i] == Biome::IceSheet as u8) as u8
            })
            .collect()
    };
    // Roślinność – osobny RNG, więc nie zmieniają terenu ani biomów.
    let veg = vegetation::build(&p, &layout, &relief.terrain, &relief.shade, &biomes.dominant, &biomes.layers, &biomes.mix);

    let mut stats = relief.stats(lakes, rivers);
    stats.biome_shares = biomes.shares(&relief.terrain);
    stats.mixed_continents = biomes.mixed_continents;
    stats.forest_share = veg.forest_share(&relief.terrain);
    let n = relief.terrain.len();
    let map = MapData {
        width: p.width,
        height: p.height,
        chunk_cols: p.chunk_cols,
        chunk_rows: p.chunk_rows,
        water_chunks: layout.owner.iter().map(|&o| (o < 0) as u8).collect(),
        terrain: relief.terrain.iter().map(|&t| t as u8).collect(),
        shade: relief.shade.clone(),
        biome: biomes.dominant,
        biome_layers: biomes.layers,
        biome_mix: biomes.mix,
        sea_ice,
        forest: veg.forest,
        river_flow: relief.river_flow,
        province: vec![0; n],
        provinces: Vec::new(),
        stats,
    };
    let input = ProvinceInput { params: p, terrain: relief.terrain, shade: relief.shade, biome: map.biome.clone(), blocked: mountains.blocked };
    (map, input)
}

/// Faza 2: prowincje (najdłuższy etap). Ma własny RNG, więc nie zależy od kolejności faz.
pub fn generate_provinces(input: &ProvinceInput) -> Provinces {
    provinces::build(&input.params, &input.terrain, &input.shade, &input.biome, &input.blocked)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_map() {
        let p = MapGenParams { width: 400, height: 240, ..Default::default() };
        let a = generate(&p);
        let b = generate(&p);
        assert_eq!(a.terrain, b.terrain);
        assert_eq!(a.shade, b.shade);
        assert_eq!(a.biome, b.biome);
        assert_eq!(a.biome_layers, b.biome_layers);
        assert_eq!(a.biome_mix, b.biome_mix);
    }

    #[test]
    fn different_seed_different_map() {
        let p = MapGenParams { width: 400, height: 240, ..Default::default() };
        let q = MapGenParams { seed: 2, ..p.clone() };
        assert_ne!(generate(&p).terrain, generate(&q).terrain);
    }

    fn small() -> MapGenParams {
        MapGenParams { width: 480, height: 280, ..Default::default() }
    }

    #[test]
    fn biome_settings_do_not_change_terrain() {
        let base = generate(&small());
        let tuned = generate(&MapGenParams {
            biome_dry: 1.0,
            biome_mix_chance: 1.0,
            biome_transition: 200,
            biome_roughness: 1.0,
            biome_ice_share: 1.0,
            biome_kind_transition: 200,
            biome_kind_roughness: 1.0,
            ..small()
        });
        let off = generate(&MapGenParams { biomes: false, ..small() });
        assert_eq!(base.terrain, tuned.terrain);
        assert_eq!(base.terrain, off.terrain);
    }

    #[test]
    fn biomes_off_means_temperate_everywhere() {
        let m = generate(&MapGenParams { biomes: false, ..small() });
        assert!(m.biome.iter().all(|&b| b == Biome::Oceanic as u8));
        assert!(m.biome_mix.iter().all(|&k| k == 0));
        assert!((0..m.biome.len()).all(|i| kind_weights(&m.biome_layers[6 * i..6 * i + 6], 0)[Biome::Oceanic as usize] == 1.0));
    }

    #[test]
    fn zero_weight_type_never_appears() {
        for seed in 1..6 {
            let m = generate(&MapGenParams { seed, biome_dry: 0.0, biome_mix_chance: 1.0, ..small() });
            for i in 0..m.biome.len() {
                let w = weights(&m, i);
                for b in [Biome::Desert, Biome::Steppe] {
                    assert_eq!(w[b as usize], 0.0, "seed {seed}: {b:?}");
                }
            }
        }
    }

    #[test]
    fn every_type_area_has_its_kinds() {
        // Każdy typ, który zajmuje spory obszar, ma wszystkie swoje rodzaje (udziały domyślne > 0).
        let mut checked = 0;
        for seed in 1..6 {
            let m = generate(&MapGenParams { seed, biome_variants: full_variants(), ..medium() });
            let shares = &m.stats.biome_shares;
            for t in BiomeType::ALL {
                let kinds: Vec<Biome> = Biome::ALL.into_iter().filter(|b| b.kind_of() == t).collect();
                let total: f32 = kinds.iter().map(|&b| shares[b as usize]).sum();
                if total < 0.05 {
                    continue;
                }
                checked += 1;
                for b in kinds {
                    assert!(shares[b as usize] > total * 0.08, "seed {seed}: {b:?} ma {} z {total}", shares[b as usize]);
                }
            }
        }
        assert!(checked >= 5, "za mało typów do sprawdzenia: {checked}");
    }

    #[test]
    fn kind_shares_follow_the_settings() {
        let tuned = MapGenParams {
            biome_desert_share: 0.0,
            biome_ice_share: 1.0,
            biome_boreal_share: 0.0,
            biome_hot_summer_share: 0.0,
            biome_variants: full_variants(),
            ..medium()
        };
        for seed in 1..6 {
            let m = generate(&MapGenParams { seed, ..tuned.clone() });
            let s = &m.stats.biome_shares;
            assert_eq!(s[Biome::Desert as usize], 0.0, "seed {seed}");
            assert_eq!(s[Biome::Tundra as usize], 0.0, "seed {seed}");
            assert_eq!(s[Biome::Taiga as usize], 0.0, "seed {seed}");
            assert_eq!(s[Biome::Boreal as usize], 0.0, "seed {seed}");
            assert_eq!(s[Biome::HotSummer as usize], 0.0, "seed {seed}");
        }
    }

    /// Wagi wariantów: zawsze wszystkie rodzaje typu.
    fn full_variants() -> Vec<f32> {
        default_variants().into_iter().map(|w| if w == 1.0 { 1.0 } else { 0.0 }).collect()
    }

    #[test]
    fn variants_limit_kinds() {
        // Tylko umiarkowany, zawsze w wariancie „sam oceaniczny”.
        let mut v = vec![0.0; VARIANT_COUNT];
        v[variant_index(BiomeType::Temperate, 0b100)] = 1.0;
        for seed in 1..4 {
            let m = generate(&MapGenParams {
                seed,
                biome_tropical: 0.0,
                biome_dry: 0.0,
                biome_continental: 0.0,
                biome_polar: 0.0,
                biome_variants: v.clone(),
                ..small()
            });
            let s = &m.stats.biome_shares;
            assert!((s[Biome::Oceanic as usize] - 1.0).abs() < 1e-6, "seed {seed}: {s:?}");
        }
        // Domyślnie część obszarów dostaje wariant bez któregoś rodzaju, ale większość – pełny.
        assert_eq!(default_variants().len(), VARIANT_COUNT);
        assert_eq!(variant_index(BiomeType::Polar, 0b111), VARIANT_COUNT - 1);
    }

    #[test]
    fn ice_sheet_is_passable_and_bare() {
        let mut ice = 0;
        for seed in 1..6 {
            let m = generate(&MapGenParams { seed, biome_polar: 1.0, ..medium() });
            for i in 0..m.terrain.len() {
                let t = m.terrain[i];
                if m.biome[i] == Biome::IceSheet as u8 && (t == Terrain::Plains as u8 || t == Terrain::Highlands as u8) {
                    ice += 1;
                    assert!(m.province[i] > 0, "seed {seed}: lądolód bez prowincji");
                    assert_eq!(m.forest[i], 0, "seed {seed}: las na lądolodzie");
                }
            }
        }
        assert!(ice > 1000, "prawie nie ma lądolodu: {ice}");
    }

    #[test]
    fn sea_ice_only_next_to_ice_sheet() {
        let polar = MapGenParams {
            biome_tropical: 0.0,
            biome_dry: 0.0,
            biome_temperate: 0.0,
            biome_continental: 0.0,
            biome_polar: 1.0,
            biome_variants: full_variants(),
            ..medium()
        };
        let m = generate(&polar);
        let mut frozen = 0;
        for i in 0..m.terrain.len() {
            if m.sea_ice[i] == 1 {
                frozen += 1;
                assert_eq!(m.terrain[i], Terrain::Ocean as u8);
                assert_eq!(m.biome[i], Biome::IceSheet as u8);
            }
        }
        assert!(frozen > 0, "brak lodu morskiego");
        let off = generate(&MapGenParams { ice_shelf_width: 0, ..polar });
        assert!(off.sea_ice.iter().all(|&v| v == 0));
        assert_eq!(off.terrain, m.terrain, "lód morski nie zmienia terenu");
    }

    #[test]
    fn dry_kinds_can_reach_the_sea() {
        // Bez wpływu odległości od morza pustynia i sawanna dochodzą do wybrzeża.
        let mut coastal = [0u32; 2];
        for seed in 1..6 {
            let m = generate(&MapGenParams { seed, biome_coast_influence: 0.0, biome_mix_chance: 1.0, ..medium() });
            let w = m.width as usize;
            for i in w..m.terrain.len() - w {
                let shore = [i - 1, i + 1, i - w, i + w].iter().any(|&j| m.terrain[j] == Terrain::Ocean as u8);
                if m.terrain[i] >= Terrain::Plains as u8 && shore {
                    if m.biome[i] == Biome::Desert as u8 {
                        coastal[0] += 1;
                    } else if m.biome[i] == Biome::Savanna as u8 {
                        coastal[1] += 1;
                    }
                }
            }
        }
        assert!(coastal.iter().all(|&c| c > 100), "pustynia / sawanna przy morzu: {coastal:?}");
    }

    /// Udziały biomów w kaflu jako wektor – do porównywania sąsiadów.
    fn weights(m: &MapData, i: usize) -> [f32; Biome::COUNT] {
        kind_weights(&m.biome_layers[6 * i..6 * i + 6], m.biome_mix[i])
    }

    #[test]
    fn two_biomes_blend_without_seams() {
        let mut mixed_tiles = 0;
        for seed in 1..6 {
            let m = generate(&MapGenParams { seed, biome_mix_chance: 1.0, ..small() });
            assert!(m.stats.mixed_continents > 0, "seed {seed}");
            let (w, h) = (m.width as usize, m.height as usize);
            let land = |i: usize| m.terrain[i] >= Terrain::Plains as u8;
            for y in 0..h {
                for x in 0..w {
                    let i = y * w + x;
                    if !land(i) {
                        continue;
                    }
                    if m.biome_mix[i] > 0 {
                        mixed_tiles += 1;
                    }
                    // Sąsiednie kafle lądu nigdy nie przeskakują z jednego biomu na drugi
                    // (twardy szew = różnica 2.0; strefa przejścia daje najwyżej ~0.35).
                    for j in [(x + 1 < w).then(|| i + 1), (y + 1 < h).then(|| i + w)].into_iter().flatten() {
                        if land(j) {
                            let (a, b) = (weights(&m, i), weights(&m, j));
                            let diff: f32 = (0..Biome::COUNT).map(|c| (a[c] - b[c]).abs()).sum();
                            assert!(diff < 0.5, "seed {seed}: skok {diff} między ({x},{y}) a sąsiadem");
                        }
                    }
                }
            }
        }
        assert!(mixed_tiles > 1000, "strefa przejścia prawie nie istnieje: {mixed_tiles}");
    }

    #[test]
    fn default_pair_rules() {
        use BiomeType::*;
        let p = MapGenParams::default();
        for (a, b) in [(Tropical, Dry), (Tropical, Temperate), (Dry, Temperate), (Dry, Continental), (Temperate, Continental), (Continental, Polar)] {
            assert!(p.biomes_can_mix(a, b), "{a:?} + {b:?}");
        }
        for (a, b) in [(Tropical, Continental), (Tropical, Polar), (Dry, Polar), (Temperate, Polar)] {
            assert!(!p.biomes_can_mix(a, b), "{a:?} + {b:?}");
        }
        // Kolejność w parze nie ma znaczenia.
        assert!(!p.biomes_can_mix(Polar, Tropical));
    }

    /// Zbiór typów klimatu (z biomu dominującego) na każdym spójnym lądzie.
    fn types_per_landmass(m: &MapData) -> Vec<u8> {
        let (w, h) = (m.width as usize, m.height as usize);
        let (lab, sizes) = util::components(w, h, |i| m.terrain[i] >= Terrain::Plains as u8);
        let mut seen = vec![0u8; sizes.len()]; // maska bitowa typów
        for i in 0..w * h {
            if lab[i] != u32::MAX {
                seen[lab[i] as usize] |= 1 << Biome::ALL[m.biome[i] as usize].kind_of() as u8;
            }
        }
        seen
    }

    #[test]
    fn forbidden_pairs_never_share_a_continent() {
        let mut pairs_seen = 0;
        for seed in 1..9 {
            let p = MapGenParams {
                seed,
                biome_tropical: 1.0,
                biome_dry: 1.0,
                biome_temperate: 1.0,
                biome_continental: 1.0,
                biome_polar: 1.0,
                biome_mix_chance: 1.0,
                ..small()
            };
            let m = generate(&p);
            for mask in types_per_landmass(&m) {
                let present: Vec<BiomeType> = BiomeType::ALL.into_iter().filter(|&b| mask & (1 << b as u8) != 0).collect();
                assert!(present.len() <= 2, "seed {seed}: więcej niż dwa typy na lądzie: {present:?}");
                if let [a, b] = present[..] {
                    assert!(p.biomes_can_mix(a, b), "seed {seed}: zabroniona para {a:?} + {b:?}");
                    pairs_seen += 1;
                }
            }
        }
        assert!(pairs_seen > 0, "żaden kontynent nie dostał dwóch biomów");
    }

    #[test]
    fn no_allowed_pairs_means_single_type_continents() {
        for seed in 1..5 {
            let m = generate(&MapGenParams { seed, biome_mix_chance: 1.0, biome_pairs: 0, ..small() });
            assert_eq!(m.stats.mixed_continents, 0);
            for mask in types_per_landmass(&m) {
                assert_eq!(mask.count_ones(), 1, "seed {seed}");
            }
        }
    }

    #[test]
    fn ocean_settings_do_not_change_terrain_or_biomes() {
        let base = generate(&small());
        let tuned = generate(&MapGenParams {
            shelf_width: 40,
            shelf_variation: 1.0,
            slope_steepness: 0.0,
            seabed_relief: 1.0,
            ..small()
        });
        assert_eq!(base.terrain, tuned.terrain);
        assert_eq!(base.biome, tuned.biome);
        assert_ne!(base.shade, tuned.shade);
    }

    #[test]
    fn coast_is_shallow_and_open_ocean_is_deep() {
        let m = generate(&MapGenParams { width: 800, height: 450, ..Default::default() });
        let (w, h) = (m.width as usize, m.height as usize);
        let ocean = |i: usize| m.terrain[i] == Terrain::Ocean as u8;
        let (mut coast, mut deepest) = (0u32, 0u8);
        for y in 1..h - 1 {
            for x in 1..w - 1 {
                let i = y * w + x;
                if !ocean(i) {
                    continue;
                }
                deepest = deepest.max(m.shade[i]);
                if [i - 1, i + 1, i - w, i + w].iter().any(|&j| !ocean(j)) {
                    coast += 1;
                    assert!(m.shade[i] < 40, "kafel przy brzegu ({x},{y}) za głęboki: {}", m.shade[i]);
                }
            }
        }
        assert!(coast > 0);
        assert!(deepest > 180, "brak głębi oceanu: {deepest}");
    }

    #[test]
    fn vegetation_settings_do_not_change_terrain_or_biomes() {
        let base = generate(&small());
        let tuned = generate(&MapGenParams { forest_oceanic: 1.0, forest_clumping: 0.0, forest_moisture: 1.0, ..small() });
        let off = generate(&MapGenParams { forest: false, ..small() });
        for m in [&tuned, &off] {
            assert_eq!(base.terrain, m.terrain);
            assert_eq!(base.biome, m.biome);
            assert_eq!(base.shade, m.shade);
        }
        assert!(off.forest.iter().all(|&f| f == 0));
    }

    #[test]
    fn no_forest_on_water_and_mountains() {
        let m = generate(&small());
        for i in 0..m.terrain.len() {
            let t = m.terrain[i];
            if t < Terrain::Plains as u8 || t == Terrain::Mountains as u8 {
                assert_eq!(m.forest[i], 0, "las na kaflu typu {t}");
            }
        }
    }

    #[test]
    fn forest_share_follows_the_setting() {
        // Jeden biom (biomy wyłączone = oceaniczny): udział lasu bliski ustawieniu.
        for share in [0.2, 0.5, 0.8] {
            let m = generate(&MapGenParams { biomes: false, forest_oceanic: share, ..small() });
            let got = m.stats.forest_share;
            assert!((got - share).abs() < 0.1, "ustawione {share}, wyszło {got}");
        }
    }

    #[test]
    fn rainforest_is_denser_than_savanna() {
        let m = generate(&MapGenParams { width: 800, height: 450, ..Default::default() });
        let (w, h) = (m.width as usize, m.height as usize);
        let share = |b: Biome| {
            let tiles: Vec<usize> = (0..w * h).filter(|&i| m.terrain[i] >= 3 && m.terrain[i] != 5 && m.biome[i] == b as u8).collect();
            tiles.iter().filter(|&&i| m.forest[i] >= 128).count() as f32 / tiles.len().max(1) as f32
        };
        let (rain, savanna) = (share(Biome::Rainforest), share(Biome::Savanna));
        if rain > 0.0 && savanna > 0.0 {
            assert!(rain > savanna, "las deszczowy {rain} vs sawanna {savanna}");
        }
    }

    #[test]
    fn province_settings_do_not_change_anything_else() {
        let base = generate(&small());
        let tuned = generate(&MapGenParams { province_size: 250.0, province_natural_borders: 1.0, province_roughness: 0.0, ..small() });
        let off = generate(&MapGenParams { provinces: false, ..small() });
        for m in [&tuned, &off] {
            assert_eq!(base.terrain, m.terrain);
            assert_eq!(base.shade, m.shade);
            assert_eq!(base.biome, m.biome);
            assert_eq!(base.forest, m.forest);
        }
        assert!(off.province.iter().all(|&p| p == 0));
        assert!(off.provinces.is_empty());
        assert_ne!(base.province, tuned.province);
    }

    fn medium() -> MapGenParams {
        MapGenParams { width: 800, height: 450, ..Default::default() }
    }

    #[test]
    fn every_land_tile_has_one_province_and_water_has_none() {
        let m = generate(&medium());
        let count = m.provinces.len();
        assert!(count > 20, "za mało prowincji: {count}");
        let mut area = vec![0u32; count];
        for (i, &p) in m.province.iter().enumerate() {
            let t = m.terrain[i];
            // Niziny i wyżyny zawsze mają prowincję, woda (także rzeki) i góry nigdy.
            if t == Terrain::Plains as u8 || t == Terrain::Highlands as u8 {
                assert!(p > 0, "kafel {i} typu {t} bez prowincji");
            } else {
                assert_eq!(p, 0, "kafel {i} typu {t} w prowincji {p}");
            }
            if p > 0 {
                area[p as usize - 1] += 1;
            }
        }
        for (k, pr) in m.provinces.iter().enumerate() {
            assert_eq!(pr.id as usize, k + 1);
            assert_eq!(pr.area, area[k]);
            let c = pr.center_y as usize * m.width as usize + pr.center_x as usize;
            assert_eq!(m.province[c], pr.id, "środek poza prowincją {}", pr.id);
        }
    }

    #[test]
    fn provinces_are_contiguous_on_each_landmass() {
        let m = generate(&medium());
        let (w, h) = (m.width as usize, m.height as usize);
        let (mass, _) = util::components(w, h, |i| m.province[i] > 0);
        // Kawałki każdej prowincji (spójne w sąsiedztwie 4) – najwyżej jeden na każdy ląd.
        for pr in &m.provinces {
            let (lab, sizes) = util::components(w, h, |i| m.province[i] == pr.id);
            let mut masses: Vec<u32> = (0..w * h).filter(|&i| lab[i] != u32::MAX).map(|i| mass[i]).collect();
            masses.sort_unstable();
            masses.dedup();
            assert_eq!(sizes.len(), masses.len(), "prowincja {} ma {} kawałków na {} lądach", pr.id, sizes.len(), masses.len());
        }
    }

    #[test]
    fn provinces_have_similar_size() {
        for (seed, size) in [(1, 600.0), (2, 600.0), (3, 300.0)] {
            let p = MapGenParams { seed, province_size: size, ..medium() };
            let m = generate(&p);
            // Prowincje na dużych lądach (bez samotnych wysp): liczba kafli blisko docelowej.
            let main: Vec<&Province> = m.provinces.iter().filter(|pr| pr.area >= p.province_min_size).collect();
            let close = main.iter().filter(|pr| (pr.area as f32 / size - 1.0).abs() < 0.25).count();
            let share = close as f32 / main.len() as f32;
            assert!(share > 0.85, "seed {seed}: tylko {:.0}% prowincji blisko docelowej wielkości", share * 100.0);
            let mean = main.iter().map(|pr| pr.area as f32).sum::<f32>() / main.len() as f32;
            assert!((mean / size - 1.0).abs() < 0.1, "seed {seed}: średnia {mean}");
            assert!(m.provinces.iter().all(|pr| pr.area <= p.province_max_size), "seed {seed}: za duża prowincja");
        }
    }

    #[test]
    fn mountains_are_blocked() {
        for seed in 1..4 {
            let m = generate(&MapGenParams { seed, ..medium() });
            for i in 0..m.terrain.len() {
                if m.terrain[i] == Terrain::Mountains as u8 {
                    assert_eq!(m.province[i], 0, "seed {seed}: góry w prowincji");
                }
            }
        }
    }

    #[test]
    fn province_properties_match_tiles() {
        let m = generate(&medium());
        let (w, h) = (m.width as usize, m.height as usize);
        let n = m.provinces.len();
        let mut near = vec![[false; 4]; n]; // ocean, rzeka, jezioro, góry
        let mut biomes = vec![[0u32; Biome::COUNT]; n];
        for i in 0..w * h {
            let p = m.province[i] as usize;
            if p == 0 {
                continue;
            }
            biomes[p - 1][m.biome[i] as usize] += 1;
            let (x, y) = (i % w, i / w);
            for j in [(x > 0).then(|| i - 1), (x + 1 < w).then(|| i + 1), (y > 0).then(|| i - w), (y + 1 < h).then(|| i + w)].into_iter().flatten() {
                let t = m.terrain[j];
                for (k, want) in [Terrain::Ocean, Terrain::River, Terrain::Lake, Terrain::Mountains].into_iter().enumerate() {
                    near[p - 1][k] |= t == want as u8;
                }
            }
        }
        for (k, pr) in m.provinces.iter().enumerate() {
            assert_eq!([pr.coastal, pr.river, pr.lake, pr.mountains], near[k], "prowincja {}", pr.id);
            let b = &biomes[k];
            let top = (0..Biome::COUNT).fold(0, |best, x| if b[x] > b[best] { x } else { best });
            assert_eq!(pr.biome as usize, top, "prowincja {}", pr.id);
        }
        // Na domyślnej mapie występują wszystkie rodzaje sąsiedztwa.
        for k in 0..4 {
            assert!(near.iter().any(|f| f[k]), "żadna prowincja nie ma sąsiedztwa {k}");
        }
    }

    #[test]
    fn two_phase_generation_matches_full() {
        let full = generate(&small());
        let (mut base, input) = generate_base(&small());
        assert!(base.province.iter().all(|&p| p == 0) && base.provinces.is_empty());
        assert_eq!(base.terrain, full.terrain);
        generate_provinces(&input).apply(&mut base);
        assert_eq!(base.province, full.province);
        assert_eq!(base.provinces.len(), full.provinces.len());
        assert_eq!(base.stats.provinces, full.stats.provinces);
    }

    #[test]
    fn river_flow_runs_downstream() {
        let m = generate(&MapGenParams { width: 800, height: 450, ..Default::default() });
        let (w, h) = (m.width as usize, m.height as usize);
        let river = Terrain::River as u8;
        let mut tiles = 0;
        for y in 1..h - 1 {
            for x in 1..w - 1 {
                let i = y * w + x;
                if m.terrain[i] != river {
                    assert_eq!(m.river_flow[i], 0);
                    continue;
                }
                tiles += 1;
                assert!(m.river_flow[i] > 0, "kafel rzeki ({x},{y}) bez nurtu");
                // Z każdego kafla rzeki da się zejść dalej z nurtem: w promieniu 2 kafli (boczne kafle
                // szerokiej rzeki dziedziczą wartość środka) jest kafel z mniejszą odległością
                // albo woda stojąca / morze (ujście).
                let down = (0..25).filter(|&k| k != 12).any(|k| {
                    let (xx, yy) = ((x + k % 5).wrapping_sub(2), (y + k / 5).wrapping_sub(2));
                    if xx >= w || yy >= h {
                        return false;
                    }
                    let j = yy * w + xx;
                    (m.terrain[j] == river && m.river_flow[j] < m.river_flow[i]) || m.terrain[j] < river
                });
                assert!(down || m.river_flow[i] <= 2, "nurt urywa się w ({x},{y})");
            }
        }
        assert!(tiles > 100);
    }
}
