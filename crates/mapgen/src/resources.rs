//! Zasoby prowincji (koniec fazy 3, `polish`): każda prowincja dostaje jeden z `RESOURCE_COUNT` zasobów.
//!
//! Szanse to tabela `[biom][ukształtowanie][zasób]` (`resource_table.rs`, generowana przez
//! `tools/resources/chances.mjs`), każda kolumna sumuje się do 100%. Prowincja kwalifikuje się do
//! kilku ukształtowań – płaski albo wyżyna (przewaga kafli), góry (`mountains`), rzeki i jeziora
//! (`river` lub `lake`), brzeg (`coastal`) – a jej szanse to średnia tych kolumn dla jej biomu.
//! Złoto jest bardzo rzadkie (tylko góry i rzeki), ale na każdej mapie jest co najmniej jedna prowincja ze złotem.
//! Losowanie ma własny strumień losowości, więc nie zmienia niczego innego na mapie.

use crate::{MapData, Terrain, util::Rng};

pub use crate::resource_table::{GOLD, RESOURCE_CHANCES, RESOURCE_COUNT, RESOURCE_NAMES};

const RESOURCE_SALT: u64 = 0x1405_7B7E_F767_814F;

/// Ukształtowania prowincji – kolumny tabeli szans.
const FLAT: usize = 0;
const UPLAND: usize = 1;
const MOUNTAINS: usize = 2;
const WATER: usize = 3;
const COAST: usize = 4;

/// Przydziela zasób każdej prowincji (`Province::resource`). Wymaga ostatecznej numeracji prowincji.
pub fn assign(map: &mut MapData, seed: u32) {
    let mut highlands = vec![0u32; map.provinces.len() + 1];
    for (i, &p) in map.province.iter().enumerate() {
        if p != 0 && map.terrain[i] == Terrain::Highlands as u8 {
            highlands[p as usize] += 1;
        }
    }
    let mut rng = Rng::new(seed as u64 ^ RESOURCE_SALT);
    for pr in &mut map.provinces {
        let upland = highlands[pr.id as usize] * 2 > pr.area;
        let mut columns = [false; 5];
        columns[if upland { UPLAND } else { FLAT }] = true;
        columns[MOUNTAINS] = pr.mountains;
        columns[WATER] = pr.river || pr.lake;
        columns[COAST] = pr.coastal;
        let chances = &RESOURCE_CHANCES[(pr.biome as usize).min(RESOURCE_CHANCES.len() - 1)];
        // Suma kolumn zamiast średniej – te same proporcje, bez dzielenia (liczby całkowite).
        let mut weights = [0u32; RESOURCE_COUNT];
        for (c, _) in columns.iter().enumerate().filter(|&(_, &on)| on) {
            for (w, &p) in weights.iter_mut().zip(&chances[c]) {
                *w += p as u32;
            }
        }
        let mut roll = rng.below(weights.iter().sum::<u32>() as usize) as u32;
        let picked = weights.iter().position(|&w| {
            if roll < w {
                return true;
            }
            roll -= w;
            false
        });
        pr.resource = picked.expect("kolumny sumują się do 1000") as u8;
    }
    // Złoto ma być na każdej mapie (walka o nie od startu gry): gdy losowanie nie dało żadnego,
    // dostaje je losowa prowincja przy górach (a na mapie bez gór – dowolna).
    if !map.provinces.is_empty() && !map.provinces.iter().any(|p| p.resource == GOLD) {
        let near_mountains: Vec<usize> = (0..map.provinces.len()).filter(|&k| map.provinces[k].mountains).collect();
        let pool = if near_mountains.is_empty() { (0..map.provinces.len()).collect() } else { near_mountains };
        map.provinces[pool[rng.below(pool.len())]].resource = GOLD;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MapGenParams, generate};

    #[test]
    fn every_column_sums_to_100_percent() {
        for biome in &RESOURCE_CHANCES {
            for column in biome {
                assert_eq!(column.iter().map(|&p| p as u32).sum::<u32>(), 1000);
            }
        }
    }

    /// Zasób zawsze ma niezerową szansę w którejś kolumnie prowincji (wyjątki, np. ryż w tundrze,
    /// nigdy nie wypadają), a przydział jest powtarzalny i zależy od seeda.
    #[test]
    fn resources_follow_the_table() {
        let p = MapGenParams { width: 480, height: 280, ..Default::default() };
        let map = generate(&p);
        let mut highlands = vec![0u32; map.provinces.len() + 1];
        for (i, &id) in map.province.iter().enumerate() {
            if id != 0 && map.terrain[i] == Terrain::Highlands as u8 {
                highlands[id as usize] += 1;
            }
        }
        for pr in &map.provinces {
            let r = pr.resource as usize;
            assert!(r < RESOURCE_COUNT);
            let chances = &RESOURCE_CHANCES[pr.biome as usize];
            let upland = highlands[pr.id as usize] * 2 > pr.area;
            let allowed = chances[if upland { UPLAND } else { FLAT }][r] > 0
                || (pr.mountains && chances[MOUNTAINS][r] > 0)
                || ((pr.river || pr.lake) && chances[WATER][r] > 0)
                || (pr.coastal && chances[COAST][r] > 0);
            assert!(allowed, "prowincja {} ma zasób {} spoza tabeli", pr.id, RESOURCE_NAMES[r]);
        }
        let resources = |m: &MapData| m.provinces.iter().map(|p| p.resource).collect::<Vec<_>>();
        assert_eq!(resources(&map), resources(&generate(&p)));
        let distinct = resources(&map).into_iter().collect::<std::collections::BTreeSet<_>>().len();
        assert!(distinct >= 10, "za mało różnych zasobów: {distinct}");
        assert!(map.provinces.iter().any(|p| p.resource == GOLD), "brak złota na mapie");
    }
}
