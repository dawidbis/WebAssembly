//! Generator map z seeda. Czysty Rust, zero zależności od webu.
//! Ten sam seed + te same parametry + ta sama wersja generatora = identyczna mapa
//! (natywnie i w wasm), bo używamy własnego RNG i `libm` w fastnoise-lite.

mod hydro;
mod layout;
mod relief;
mod util;

use serde::{Deserialize, Serialize};

/// Zwiększaj przy każdej zmianie algorytmu – stare seedy dają wtedy inne mapy,
/// więc wersja musi trafić do konfiguracji gry i do replayów.
pub const GENERATOR_VERSION: u32 = 1;

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
}

impl Default for MapGenParams {
    fn default() -> Self {
        Self {
            seed: 1,
            width: 1600,
            height: 900,
            chunk_cols: 8,
            chunk_rows: 4,
            continents: 3,
            land_ratio: 0.65,
            size_variance: 0.5,
            coast_roughness: 0.5,
            min_island_area: 300,
            keep_off_edges: true,
            edge_margin: 12,
            mountain_share: 0.08,
            highland_share: 0.20,
            range_scale: 1.0,
            rivers: true,
            river_count: 30,
            lakes: true,
            lake_amount: 0.5,
            min_lake_area: 40,
            max_lake_area: 2500,
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
        p
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
    let (lakes, rivers) = hydro::build(&p, &layout, &mut relief, &mut rng);

    let stats = relief.stats(lakes, rivers);
    MapData {
        width: p.width,
        height: p.height,
        chunk_cols: p.chunk_cols,
        chunk_rows: p.chunk_rows,
        water_chunks: layout.owner.iter().map(|&o| (o < 0) as u8).collect(),
        terrain: relief.terrain.iter().map(|&t| t as u8).collect(),
        shade: relief.shade,
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
    }

    #[test]
    fn different_seed_different_map() {
        let p = MapGenParams { width: 400, height: 240, ..Default::default() };
        let q = MapGenParams { seed: 2, ..p.clone() };
        assert_ne!(generate(&p).terrain, generate(&q).terrain);
    }
}
