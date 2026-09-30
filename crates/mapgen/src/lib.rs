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

pub use provinces::Province;

/// Zwiększaj przy każdej zmianie algorytmu – stare seedy dają wtedy inne mapy,
/// więc wersja musi trafić do konfiguracji gry i do replayów.
pub const GENERATOR_VERSION: u32 = 8;

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

/// Biomy (styl wizualny i klimat kontynentu). Wartości muszą zgadzać się z `web/src/app/render/terrain.ts`.
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Biome {
    Temperate = 0,
    Desert = 1,
    Cold = 2,
    Humid = 3,
    Steppe = 4,
}

impl Biome {
    pub const ALL: [Biome; 5] = [Biome::Temperate, Biome::Desert, Biome::Cold, Biome::Humid, Biome::Steppe];

    /// Numer bitu pary biomów w `MapGenParams::biome_pairs` (kolejność jak `BIOME_PAIRS`).
    /// `None` dla tego samego biomu.
    pub fn pair_bit(a: Biome, b: Biome) -> Option<u32> {
        let (a, b) = if (a as u8) < (b as u8) { (a, b) } else { (b, a) };
        BIOME_PAIRS.iter().position(|&pair| pair == (a, b)).map(|i| i as u32)
    }
}

/// Wszystkie pary biomów. Indeks pary = numer bitu w `MapGenParams::biome_pairs`.
/// Kolejność musi zgadzać się z `BIOME_PAIRS` w `web/src/app/render/terrain.ts`.
pub const BIOME_PAIRS: [(Biome, Biome); 10] = [
    (Biome::Temperate, Biome::Desert),
    (Biome::Temperate, Biome::Cold),
    (Biome::Temperate, Biome::Humid),
    (Biome::Temperate, Biome::Steppe),
    (Biome::Desert, Biome::Cold),
    (Biome::Desert, Biome::Humid),
    (Biome::Desert, Biome::Steppe),
    (Biome::Cold, Biome::Humid),
    (Biome::Cold, Biome::Steppe),
    (Biome::Humid, Biome::Steppe),
];

/// Domyślne zasady łączenia: wilgotny (dżungla) tylko ze stepem, zimny nie z pustynnym
/// ani ze stepem, umiarkowany z pozostałymi, pustynny ze stepem.
pub const DEFAULT_BIOME_PAIRS: u32 = {
    let allowed = [0, 1, 3, 6, 9];
    let mut mask = 0;
    let mut i = 0;
    while i < allowed.len() {
        mask |= 1 << allowed[i];
        i += 1;
    }
    mask
};

#[derive(Clone, Debug, Serialize, Deserialize)]
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
    /// Wyłączone = cały ląd w biomie umiarkowanym (wygląd sprzed biomów).
    pub biomes: bool,
    /// Szansa na biom (wagi względne, normalizowane do sumy; 0 = biom nie występuje).
    pub biome_temperate: f32,
    pub biome_desert: f32,
    pub biome_cold: f32,
    pub biome_humid: f32,
    pub biome_steppe: f32,
    /// 0 = biomy losowe, 1 = biom wynika z położenia względem biegunów klimatu: biegun zimna
    /// przy górnej albo dolnej krawędzi, biegun ciepła (pustynia) naprzeciwko.
    pub biome_latitude: f32,
    /// Szansa, że kontynent ma dwa biomy.
    pub biome_mix_chance: f32,
    /// Które pary biomów mogą wystąpić razem na jednym kontynencie – maska bitowa, bit = indeks w `BIOME_PAIRS`.
    pub biome_pairs: u32,
    /// Udział drugiego biomu w kontynencie (średnio; losowane ±25%).
    pub biome_secondary_share: f32,
    /// Szerokość strefy przejścia między biomami w kaflach.
    pub biome_transition: u32,
    /// Pofalowanie granicy biomów i przeplatanie się płatów w strefie przejścia.
    pub biome_roughness: f32,
    /// Średnia szerokość płytkiego szelfu przy brzegu (kafle).
    pub shelf_width: u32,
    /// Zmienność szerokości szelfu: 0 = równy pas wokół lądu, 1 = szerokie ławice obok urwisk.
    pub shelf_variation: f32,
    /// Stromość stoku kontynentalnego: 1 = ostre urwisko, 0 = łagodny spadek.
    pub slope_steepness: f32,
    /// Rzeźba dna: podwodne grzbiety, rowy i góry podwodne.
    pub seabed_relief: f32,
    /// Lasy (wyłączone = brak lasów; żyzność liczona zawsze).
    pub forest: bool,
    /// Docelowy udział lasu w lądzie danego biomu (bez gór).
    pub forest_temperate: f32,
    pub forest_desert: f32,
    pub forest_cold: f32,
    pub forest_humid: f32,
    pub forest_steppe: f32,
    /// Zwartość lasów: 0 = drobne, rozproszone kępy, 1 = duże zwarte masywy.
    pub forest_clumping: f32,
    /// Jak mocno las ciągnie do wody (rzeki, jeziora, wybrzeże).
    pub forest_moisture: f32,
    /// Dodatek do żyzności na brzegach rzek i jezior (0..1) i szerokość tego pasa w kaflach.
    pub fertility_river_bonus: f32,
    pub fertility_river_reach: u32,
    /// Podział lądu na prowincje.
    pub provinces: bool,
    /// Docelowa (średnia) wartość prowincji – w „kaflach niziny” przy wartości niziny 1.
    pub province_value: f32,
    /// Wartość kafla rośnie z żyznością od tej wartości (jałowa ziemia, kafel rzeki) do 1
    /// (najżyźniejsza). Góry są niczyje.
    pub province_value_floor: f32,
    /// Najmniejsza i największa prowincja w kaflach.
    pub province_min_size: u32,
    pub province_max_size: u32,
    /// Jak mocno granice trzymają się rzek i grzbietów górskich (0 = wcale).
    pub province_natural_borders: f32,
    /// Nieregularność granic (0 = gładkie, zaokrąglone prowincje).
    pub province_roughness: f32,
}

impl Default for MapGenParams {
    fn default() -> Self {
        Self {
            seed: 1,
            width: 2000,
            height: 1000,
            chunk_cols: 12,
            chunk_rows: 6,
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
            biome_temperate: 0.9,
            biome_desert: 0.3,
            biome_cold: 0.3,
            biome_humid: 0.35,
            biome_steppe: 0.4,
            biome_latitude: 0.6,
            biome_mix_chance: 0.5,
            biome_pairs: DEFAULT_BIOME_PAIRS,
            biome_secondary_share: 0.4,
            biome_transition: 60,
            biome_roughness: 0.5,
            shelf_width: 14,
            shelf_variation: 0.6,
            slope_steepness: 0.7,
            seabed_relief: 0.5,
            forest: true,
            forest_temperate: 0.4,
            forest_desert: 0.03,
            forest_cold: 0.75,
            forest_humid: 0.95,
            forest_steppe: 0.08,
            forest_clumping: 0.85,
            forest_moisture: 0.5,
            fertility_river_bonus: 0.6,
            fertility_river_reach: 15,
            provinces: true,
            province_value: 400.0,
            province_value_floor: 0.25,
            province_min_size: 120,
            province_max_size: 4000,
            province_natural_borders: 0.6,
            province_roughness: 0.5,
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
            &mut p.biome_temperate,
            &mut p.biome_desert,
            &mut p.biome_cold,
            &mut p.biome_humid,
            &mut p.biome_steppe,
        ] {
            *w = w.clamp(0.0, 1.0);
        }
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
            &mut p.forest_temperate,
            &mut p.forest_desert,
            &mut p.forest_cold,
            &mut p.forest_humid,
            &mut p.forest_steppe,
            &mut p.forest_clumping,
            &mut p.forest_moisture,
        ] {
            *s = s.clamp(0.0, 1.0);
        }
        p.fertility_river_bonus = p.fertility_river_bonus.clamp(0.0, 1.0);
        p.fertility_river_reach = p.fertility_river_reach.clamp(1, 60);
        p.province_value = p.province_value.clamp(20.0, 100_000.0);
        p.province_value_floor = p.province_value_floor.clamp(0.02, 1.0);
        p.province_min_size = p.province_min_size.clamp(1, 100_000);
        p.province_max_size = p.province_max_size.clamp(p.province_min_size, 1_000_000);
        p.province_natural_borders = p.province_natural_borders.clamp(0.0, 1.0);
        p.province_roughness = p.province_roughness.clamp(0.0, 1.0);
        p
    }

    /// Docelowe udziały lasu w kolejności `Biome::ALL`.
    pub fn forest_shares(&self) -> [f32; 5] {
        [self.forest_temperate, self.forest_desert, self.forest_cold, self.forest_humid, self.forest_steppe]
    }

    /// Czy dwa różne biomy mogą wystąpić na jednym kontynencie.
    pub fn biomes_can_mix(&self, a: Biome, b: Biome) -> bool {
        Biome::pair_bit(a, b).is_some_and(|bit| self.biome_pairs & (1 << bit) != 0)
    }

    /// Szanse (wagi) biomów w kolejności `Biome::ALL`.
    pub fn biome_weights(&self) -> [f32; 5] {
        [self.biome_temperate, self.biome_desert, self.biome_cold, self.biome_humid, self.biome_steppe]
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
    /// Ile kontynentów ma dwa biomy.
    pub mixed_continents: u32,
    /// Udział lasu w lądzie (gęstość ≥ 128).
    pub forest_share: f32,
    /// Udział żyznego lądu (żyzność ≥ 128).
    pub fertile_share: f32,
    /// Liczba prowincji.
    pub provinces: u32,
    /// Wartość prowincji (bez samotnych wysp): średnia, najmniejsza, największa, odchylenie standardowe.
    pub province_value_mean: f32,
    pub province_value_min: f32,
    pub province_value_max: f32,
    pub province_value_std: f32,
    /// Powierzchnia prowincji w kaflach: średnia, najmniejsza, największa.
    pub province_area_mean: f32,
    pub province_area_min: u32,
    pub province_area_max: u32,
}

/// Wynik generatora. `terrain` i `shade` mają rozmiar `width * height`, wiersz po wierszu.
#[derive(Clone, Debug)]
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
    /// Biom dominujący kafla (`Biome as u8`) – to on liczy się w rozgrywce.
    /// Woda dostaje biom najbliższego lądu.
    pub biome: Vec<u8>,
    /// Drugi biom w strefie przejścia (poza nią równy `biome`).
    pub biome_other: Vec<u8>,
    /// Udział `biome_other` w kaflu: 0..=128 (128 = dokładnie pół na pół, granica biomów).
    pub biome_mix: Vec<u8>,
    /// Gęstość lasu 0..255 (≥ 128 = las). Typ lasu wynika z biomu kafla.
    pub forest: Vec<u8>,
    /// Kafle rzek: odległość do ujścia wzdłuż nurtu (maleje z prądem), 0 = nie rzeka.
    /// Tylko do animacji nurtu – nie wchodzi do hashy.
    pub river_flow: Vec<u16>,
    /// Żyzność gleby 0..255 – pod przyszłe pola uprawne wokół miast.
    pub fertility: Vec<u8>,
    /// Numer prowincji kafla (od 1), 0 = brak. Bez prowincji są woda i kafle nieprzechodnie:
    /// góry oraz rzeki płynące przez góry (kafel lądu/rzeki z prowincją 0 blokuje ruch).
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

pub fn generate(params: &MapGenParams) -> MapData {
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
    // Roślinność i żyzność – osobny RNG, więc nie zmieniają terenu ani biomów.
    let veg = vegetation::build(&p, &layout, &relief.terrain, &relief.shade, &biomes.dominant, &biomes.other, &biomes.mix);

    // Prowincje – osobny RNG, więc nie zmieniają niczego wyżej.
    let prov = provinces::build(&p, &relief.terrain, &relief.shade, &veg.fertility, &mountains.blocked);

    let mut stats = relief.stats(lakes, rivers);
    prov.fill_stats(&mut stats);
    stats.biome_shares = biomes.shares(&relief.terrain);
    stats.mixed_continents = biomes.mixed_continents;
    (stats.forest_share, stats.fertile_share) = veg.shares(&relief.terrain);
    MapData {
        width: p.width,
        height: p.height,
        chunk_cols: p.chunk_cols,
        chunk_rows: p.chunk_rows,
        water_chunks: layout.owner.iter().map(|&o| (o < 0) as u8).collect(),
        terrain: relief.terrain.iter().map(|&t| t as u8).collect(),
        shade: relief.shade,
        biome: biomes.dominant,
        biome_other: biomes.other,
        biome_mix: biomes.mix,
        forest: veg.forest,
        river_flow: relief.river_flow,
        fertility: veg.fertility,
        province: prov.id,
        provinces: prov.list,
        stats,
    }
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
        assert_eq!(a.biome_other, b.biome_other);
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
            biome_desert: 1.0,
            biome_mix_chance: 1.0,
            biome_transition: 200,
            biome_roughness: 1.0,
            ..small()
        });
        let off = generate(&MapGenParams { biomes: false, ..small() });
        assert_eq!(base.terrain, tuned.terrain);
        assert_eq!(base.terrain, off.terrain);
    }

    #[test]
    fn biomes_off_means_temperate_everywhere() {
        let m = generate(&MapGenParams { biomes: false, ..small() });
        assert!(m.biome.iter().all(|&b| b == Biome::Temperate as u8));
        assert!(m.biome_mix.iter().all(|&k| k == 0));
    }

    #[test]
    fn zero_weight_biome_never_appears() {
        for seed in 1..6 {
            let m = generate(&MapGenParams { seed, biome_desert: 0.0, biome_mix_chance: 1.0, ..small() });
            assert!(!m.biome.contains(&(Biome::Desert as u8)), "seed {seed}");
        }
    }

    /// Udziały biomów w kaflu jako wektor – do porównywania sąsiadów.
    fn weights(m: &MapData, i: usize) -> [f32; 5] {
        let mut w = [0.0; 5];
        let k = m.biome_mix[i] as f32 / 256.0;
        w[m.biome[i] as usize] += 1.0 - k;
        w[m.biome_other[i] as usize] += k;
        w
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
                            let diff: f32 = (0..5).map(|c| (a[c] - b[c]).abs()).sum();
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
        use Biome::*;
        let p = MapGenParams::default();
        for b in [Desert, Cold, Steppe] {
            assert!(p.biomes_can_mix(Temperate, b));
        }
        assert!(!p.biomes_can_mix(Temperate, Humid), "dżungla tylko ze stepem");
        assert!(!p.biomes_can_mix(Cold, Desert));
        assert!(!p.biomes_can_mix(Cold, Steppe));
        assert!(!p.biomes_can_mix(Cold, Humid));
        assert!(!p.biomes_can_mix(Humid, Desert));
        assert!(p.biomes_can_mix(Humid, Steppe));
        assert!(p.biomes_can_mix(Desert, Steppe));
        // Kolejność w parze nie ma znaczenia.
        assert!(!p.biomes_can_mix(Steppe, Cold));
    }

    /// Zbiór biomów dominujących na każdym spójnym lądzie.
    fn biomes_per_landmass(m: &MapData) -> Vec<u8> {
        let (w, h) = (m.width as usize, m.height as usize);
        let (lab, sizes) = util::components(w, h, |i| m.terrain[i] >= Terrain::Plains as u8);
        let mut seen = vec![0u8; sizes.len()]; // maska bitowa biomów
        for i in 0..w * h {
            if lab[i] != u32::MAX {
                seen[lab[i] as usize] |= 1 << m.biome[i];
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
                biome_temperate: 1.0,
                biome_desert: 1.0,
                biome_cold: 1.0,
                biome_humid: 1.0,
                biome_steppe: 1.0,
                biome_mix_chance: 1.0,
                ..small()
            };
            let m = generate(&p);
            for mask in biomes_per_landmass(&m) {
                let present: Vec<Biome> = Biome::ALL.into_iter().filter(|&b| mask & (1 << b as u8) != 0).collect();
                assert!(present.len() <= 2, "seed {seed}: więcej niż dwa biomy na lądzie: {present:?}");
                if let [a, b] = present[..] {
                    assert!(p.biomes_can_mix(a, b), "seed {seed}: zabroniona para {a:?} + {b:?}");
                    pairs_seen += 1;
                }
            }
        }
        assert!(pairs_seen > 0, "żaden kontynent nie dostał dwóch biomów");
    }

    #[test]
    fn no_allowed_pairs_means_single_biome_continents() {
        for seed in 1..5 {
            let m = generate(&MapGenParams { seed, biome_mix_chance: 1.0, biome_pairs: 0, ..small() });
            assert_eq!(m.stats.mixed_continents, 0);
            assert!(m.biome_mix.iter().all(|&k| k == 0));
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
        let tuned = generate(&MapGenParams { forest_temperate: 1.0, forest_clumping: 0.0, forest_moisture: 1.0, ..small() });
        let off = generate(&MapGenParams { forest: false, ..small() });
        for m in [&tuned, &off] {
            assert_eq!(base.terrain, m.terrain);
            assert_eq!(base.biome, m.biome);
            assert_eq!(base.shade, m.shade);
        }
        assert!(off.forest.iter().all(|&f| f == 0));
        assert_eq!(base.fertility, off.fertility, "żyzność nie zależy od lasów");
    }

    #[test]
    fn no_forest_or_fertility_on_water_and_mountains() {
        let m = generate(&small());
        for i in 0..m.terrain.len() {
            let t = m.terrain[i];
            if t < Terrain::Plains as u8 || t == Terrain::Mountains as u8 {
                assert_eq!(m.forest[i], 0, "las na kaflu typu {t}");
            }
            if t < Terrain::Plains as u8 {
                assert_eq!(m.fertility[i], 0, "żyzność na wodzie");
            }
        }
    }

    #[test]
    fn forest_share_follows_the_setting() {
        // Jeden biom (biomy wyłączone = umiarkowany): udział lasu bliski ustawieniu.
        for share in [0.2, 0.5, 0.8] {
            let m = generate(&MapGenParams { biomes: false, forest_temperate: share, ..small() });
            let got = m.stats.forest_share;
            assert!((got - share).abs() < 0.1, "ustawione {share}, wyszło {got}");
        }
    }

    #[test]
    fn humid_is_denser_than_steppe_and_rivers_are_fertile() {
        let m = generate(&MapGenParams { width: 800, height: 450, ..Default::default() });
        let (w, h) = (m.width as usize, m.height as usize);
        let share = |b: Biome| {
            let tiles: Vec<usize> = (0..w * h).filter(|&i| m.terrain[i] >= 3 && m.terrain[i] != 5 && m.biome[i] == b as u8).collect();
            tiles.iter().filter(|&&i| m.forest[i] >= 128).count() as f32 / tiles.len().max(1) as f32
        };
        let (humid, steppe) = (share(Biome::Humid), share(Biome::Steppe));
        if humid > 0.0 && steppe > 0.0 {
            assert!(humid > steppe, "wilgotny {humid} vs step {steppe}");
        }
        // Kafle lądu tuż przy rzece są średnio żyźniejsze niż ląd w ogóle.
        let land: Vec<usize> = (w + 1..w * h - w - 1).filter(|&i| m.terrain[i] >= 3).collect();
        let near_river: Vec<usize> = land
            .iter()
            .copied()
            .filter(|&i| [i - 1, i + 1, i - w, i + w].iter().any(|&j| m.terrain[j] == Terrain::River as u8))
            .collect();
        let mean = |v: &[usize]| v.iter().map(|&i| m.fertility[i] as f32).sum::<f32>() / v.len().max(1) as f32;
        assert!(!near_river.is_empty());
        assert!(mean(&near_river) > mean(&land) * 1.1, "przy rzece {} vs ogółem {}", mean(&near_river), mean(&land));
    }

    #[test]
    fn province_settings_do_not_change_anything_else() {
        let base = generate(&small());
        let tuned = generate(&MapGenParams { province_value: 250.0, province_natural_borders: 1.0, province_roughness: 0.0, ..small() });
        let off = generate(&MapGenParams { provinces: false, ..small() });
        for m in [&tuned, &off] {
            assert_eq!(base.terrain, m.terrain);
            assert_eq!(base.shade, m.shade);
            assert_eq!(base.biome, m.biome);
            assert_eq!(base.forest, m.forest);
            assert_eq!(base.fertility, m.fertility);
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
            // Niziny i wyżyny zawsze mają prowincję, woda i góry nigdy; rzeki – poza górami.
            if t == Terrain::Plains as u8 || t == Terrain::Highlands as u8 {
                assert!(p > 0, "kafel {i} typu {t} bez prowincji");
            } else if t != Terrain::River as u8 {
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
    fn provinces_have_similar_value() {
        for seed in 1..4 {
            let p = MapGenParams { seed, ..medium() };
            let m = generate(&p);
            // Prowincje na dużych lądach (bez samotnych wysp): wartość blisko docelowej.
            let main: Vec<&Province> = m.provinces.iter().filter(|pr| pr.area >= p.province_min_size).collect();
            let close = main.iter().filter(|pr| (pr.value / p.province_value - 1.0).abs() < 0.25).count();
            let share = close as f32 / main.len() as f32;
            assert!(share > 0.85, "seed {seed}: tylko {:.0}% prowincji blisko docelowej wartości", share * 100.0);
            let mean = main.iter().map(|pr| pr.value).sum::<f32>() / main.len() as f32;
            assert!((mean / p.province_value - 1.0).abs() < 0.1, "seed {seed}: średnia {mean}");
            assert!(m.provinces.iter().all(|pr| pr.area <= p.province_max_size), "seed {seed}: za duża prowincja");
        }
    }

    #[test]
    fn barren_provinces_are_larger_than_fertile_ones() {
        let m = generate(&medium());
        // Ćwiartka najmniej żyznych prowincji vs ćwiartka najżyźniejszych (bez małych wysp).
        let mut main: Vec<&Province> = m.provinces.iter().filter(|pr| pr.area >= 120).collect();
        main.sort_by(|a, b| a.fertility.total_cmp(&b.fertility));
        let q = main.len() / 4;
        assert!(q > 3, "za mało prowincji: {}", main.len());
        let mean_area = |v: &[&Province]| v.iter().map(|pr| pr.area as f32).sum::<f32>() / v.len() as f32;
        let (barren, fertile) = (mean_area(&main[..q]), mean_area(&main[main.len() - q..]));
        assert!(barren > fertile * 1.2, "jałowe {barren} vs żyzne {fertile}");
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
    fn province_fertility_is_mean_of_land_tiles() {
        let m = generate(&small());
        let n = m.provinces.len();
        let (mut sum, mut cnt) = (vec![0f64; n], vec![0u32; n]);
        for (i, &p) in m.province.iter().enumerate() {
            if p > 0 && m.terrain[i] >= Terrain::Plains as u8 {
                sum[p as usize - 1] += m.fertility[i] as f64;
                cnt[p as usize - 1] += 1;
            }
        }
        for (k, pr) in m.provinces.iter().enumerate() {
            let want = if cnt[k] > 0 { sum[k] / cnt[k] as f64 } else { 0.0 };
            assert!((pr.fertility as f64 - want).abs() < 0.01, "prowincja {}: {} vs {want}", pr.id, pr.fertility);
        }
    }

    #[test]
    fn natural_borders_follow_rivers() {
        // Udział kafli rzek leżących na granicy prowincji: z naturalnymi granicami wyraźnie większy.
        let on_border = |natural: f32| {
            let m = generate(&MapGenParams { province_natural_borders: natural, ..medium() });
            let w = m.width as usize;
            let river: Vec<usize> = (w..m.province.len() - w)
                .filter(|&i| m.terrain[i] == Terrain::River as u8)
                .collect();
            let border = river
                .iter()
                .filter(|&&i| [i - 1, i + 1, i - w, i + w].iter().any(|&j| m.province[j] > 0 && m.province[j] != m.province[i]))
                .count();
            border as f32 / river.len().max(1) as f32
        };
        let (none, full) = (on_border(0.0), on_border(1.0));
        assert!(full > none * 1.5, "z granicami naturalnymi {full}, bez {none}");
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
