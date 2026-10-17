//! Stałe cechy prowincji pod budynki (faza 3 `polish`, po zasobach): **miejsca budynków** na mapie
//! (`Province::slots`) i **startowy poziom infrastruktury** (`Province::infrastructure`). Liczone
//! z mapy, więc u wszystkich graczy te same; należą do wersji generatora.
//!
//! Miejsca (kafel każdego rodzaju z miejscem; infrastruktura i umocnienia działają na całą prowincję):
//! - **Miasto** (`SLOT_CITY`; tu też uniwersytet i centrum dowodzenia): w głębi prowincji, blisko środka.
//! - **Port** (`SLOT_PORT`): kafel prowincji przy morzu, najbliżej miasta (prowincja bez morza – brak).
//! - **Wydobycie** (`SLOT_EXTRACTION`): na terenie pasującym do surowca (`FIT`), w głębi prowincji.
//! - **Fabryka** (`SLOT_FACTORY`): jak najdalej od pozostałych, w głębi prowincji.

use std::collections::VecDeque;

use crate::{Biome, MapData, Terrain, resources::RESOURCE_COUNT};

/// Liczba miejsc i ich kolejność w `Province::slots`.
pub const SLOT_COUNT: usize = 4;
pub const SLOT_CITY: usize = 0;
pub const SLOT_PORT: usize = 1;
pub const SLOT_EXTRACTION: usize = 2;
pub const SLOT_FACTORY: usize = 3;
/// Brak miejsca (np. port w prowincji bez morza).
pub const NO_SLOT: [u16; 2] = [u16::MAX, u16::MAX];
/// Kafle co najmniej tak głęboko w prowincji (od jej skraju) są kandydatami na miasto.
const MIN_DEPTH: u32 = 2;
/// Wydobycie i fabryka – na kaflach co najmniej tak głęboko (% największej głębokości prowincji).
const INNER_DEPTH_PCT: u32 = 50;
/// Startowy poziom infrastruktury: bazowo 2, korekty z cech prowincji, przycięty do 1–3.
pub const INFRA_START_BASE: i32 = 2;
pub const INFRA_START_MAX: u8 = 3;
/// Kafel leśny w rozgrywce.
const FOREST_TILE: u8 = 128;

/// Teren pod budynek wydobycia.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fit {
    /// Wyżyny (kopalnie).
    Highlands,
    /// Las (tartak, myśliwi).
    Forest,
    /// Niziny bez lasu (pola, plantacje, rancza).
    Open,
    /// Dowolny (szyb naftowy, kamieniołom).
    Any,
    /// Brzeg morza albo jeziora (łowisko).
    Shore,
}

/// Teren wydobycia każdego surowca (kolejność `RESOURCE_NAMES`) – zgodny z `goods::EXTRACTION` w rdzeniu
/// (pilnuje tego test rdzenia).
#[rustfmt::skip]
pub const FIT: [Fit; RESOURCE_COUNT] = {
    use Fit::*;
    [
        Open, Open, Open, Open,                              // zboża, ryż, kukurydza, ziemniaki (pola)
        Open, Open, Open, Open, Open, Open,                  // bawełna … herbata (pola)
        Open,                                                // kauczuk (plantacja)
        Open, Open, Open,                                    // bydło, trzoda chlewna, drób (rancza)
        Shore,                                               // ryby (łowisko)
        Open, Open,                                          // konie, wełna (rancza)
        Forest,                                              // futra (myśliwi)
        Forest,                                              // drewno (tartak)
        Highlands, Highlands,                                // węgiel kamienny, brunatny
        Any,                                                 // ropa naftowa
        Highlands, Highlands, Highlands, Highlands, Highlands, Highlands, Highlands, Highlands, Highlands,
        Highlands,                                           // rudy żelaza … złoto
        Highlands,                                           // surowce nawozowe
        Any,                                                 // cement (kamieniołom)
        Highlands, Highlands, Highlands,                     // saletra, siarka, sól
    ]
};

/// Startowy poziom infrastruktury z cech prowincji: bazowo 2; −1 za trudny biom (pustynia, lądolód,
/// tundra, las deszczowy); −1 za przewagę wyżyn albo sąsiedztwo gór; +1 za rzekę albo morze; 1–3.
pub fn start_infrastructure(biome: u8, area: u32, mountains: bool, water: bool, highland_tiles: u32) -> u8 {
    let hard = [Biome::Desert, Biome::IceSheet, Biome::Tundra, Biome::Rainforest].iter().any(|&b| biome == b as u8);
    let rough = mountains || highland_tiles * 2 > area;
    let level = INFRA_START_BASE - hard as i32 - rough as i32 + water as i32;
    level.clamp(1, INFRA_START_MAX as i32) as u8
}

/// Wpisuje miejsca budynków i startową infrastrukturę do prowincji mapy.
pub fn assign(map: &mut MapData) {
    let w = map.width as usize;
    let n = map.provinces.len();
    let mut bounds = vec![[u32::MAX, u32::MAX, 0, 0]; n];
    let mut highlands = vec![0u32; n];
    for (i, &p) in map.province.iter().enumerate() {
        if p == 0 {
            continue;
        }
        let (x, y) = ((i % w) as u32, (i / w) as u32);
        let b = &mut bounds[p as usize - 1];
        *b = [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)];
        highlands[p as usize - 1] += (map.terrain[i] == Terrain::Highlands as u8) as u32;
    }
    for k in 0..n {
        let p = &map.provinces[k];
        let slots = if bounds[k][0] == u32::MAX {
            [NO_SLOT; SLOT_COUNT]
        } else {
            province_slots(map, k as u16 + 1, bounds[k], p.center_x as usize, p.center_y as usize, p.resource)
        };
        let infrastructure = start_infrastructure(p.biome, p.area, p.mountains, p.river || p.coastal, highlands[k]);
        let p = &mut map.provinces[k];
        p.slots = slots;
        p.infrastructure = infrastructure;
    }
}

fn province_slots(
    map: &MapData,
    id: u16,
    [x0, y0, x1, y1]: [u32; 4],
    cx: usize,
    cy: usize,
    resource: u8,
) -> [[u16; 2]; SLOT_COUNT] {
    let (w, h) = (map.width as usize, map.height as usize);
    let inside = |i: usize| map.province[i] == id;
    // Kafle prowincji i głębokość od skraju (BFS od kafli przy granicy prowincji).
    let mut tiles = Vec::new();
    for y in y0 as usize..=y1 as usize {
        for x in x0 as usize..=x1 as usize {
            if inside(y * w + x) {
                tiles.push(y * w + x);
            }
        }
    }
    let neighbours = |i: usize| {
        let (x, y) = (i % w, i / w);
        [(x > 0).then(|| i - 1), (x + 1 < w).then(|| i + 1), (y > 0).then(|| i - w), (y + 1 < h).then(|| i + w)]
    };
    let mut depth = vec![u32::MAX; tiles.len()];
    let index = |i: usize| tiles.binary_search(&i).ok();
    let mut queue = VecDeque::new();
    for (k, &t) in tiles.iter().enumerate() {
        if neighbours(t).iter().any(|n| n.is_none_or(|n| !inside(n))) {
            depth[k] = 0;
            queue.push_back(k);
        }
    }
    while let Some(k) = queue.pop_front() {
        for n in neighbours(tiles[k]).into_iter().flatten() {
            if let Some(j) = index(n)
                && depth[j] == u32::MAX
            {
                depth[j] = depth[k] + 1;
                queue.push_back(j);
            }
        }
    }
    // Kandydaci: w głębi (a w małej prowincji – najgłębsze, jakie są).
    let deepest = depth.iter().copied().max().unwrap_or(0);
    let need = MIN_DEPTH.min(deepest);
    let candidates: Vec<usize> = (0..tiles.len()).filter(|&k| depth[k] >= need).map(|k| tiles[k]).collect();
    let xy = |i: usize| ((i % w) as i64, (i / w) as i64);
    let dist2 = |a: usize, b: usize| {
        let ((ax, ay), (bx, by)) = (xy(a), xy(b));
        (ax - bx).pow(2) + (ay - by).pow(2)
    };
    let center = cy * w + cx;
    let mut placed: Vec<usize> = Vec::new();
    let mut out = [NO_SLOT; SLOT_COUNT];
    let to_slot = |i: usize| [(i % w) as u16, (i / w) as u16];

    // Miasto: blisko środka, w głębi.
    let city = candidates.iter().copied().min_by_key(|&t| (dist2(t, center), t)).unwrap();
    out[SLOT_CITY] = to_slot(city);
    placed.push(city);

    // Port: kafel prowincji przy oceanie, najbliżej miasta.
    let ocean = |i: usize| neighbours(i).into_iter().flatten().any(|n| map.terrain[n] == Terrain::Ocean as u8);
    if let Some(port) = tiles.iter().copied().filter(|&t| ocean(t)).min_by_key(|&t| (dist2(t, city), t)) {
        out[SLOT_PORT] = to_slot(port);
        placed.push(port);
    }

    // Najdalej od postawionych (największa najmniejsza odległość), spośród `pool`; remis – niższy kafel.
    let farthest = |pool: &[usize], placed: &[usize]| {
        pool.iter()
            .copied()
            .max_by_key(|&t| (placed.iter().map(|&p| dist2(t, p)).min().unwrap_or(0), std::cmp::Reverse(t)))
    };
    let forest = |i: usize| map.forest[i] >= FOREST_TILE;
    let highlands = |i: usize| map.terrain[i] == Terrain::Highlands as u8;
    let fits = |i: usize| match FIT.get(resource as usize).copied().unwrap_or(Fit::Any) {
        Fit::Highlands => highlands(i),
        Fit::Forest => forest(i),
        Fit::Open => !highlands(i) && !forest(i),
        Fit::Any => true,
        Fit::Shore => neighbours(i)
            .into_iter()
            .flatten()
            .any(|n| map.terrain[n] == Terrain::Ocean as u8 || map.terrain[n] == Terrain::Lake as u8),
    };
    // Wydobycie i fabryka tylko w głębi prowincji (co najmniej połowa największej głębokości) – inaczej
    // „najdalej od pozostałych” spycha je na skraj.
    let inner_depth = need.max(deepest * INNER_DEPTH_PCT / 100);
    let pool: Vec<usize> = (0..tiles.len()).filter(|&k| depth[k] >= inner_depth).map(|k| tiles[k]).collect();
    let fitting: Vec<usize> = pool.iter().copied().filter(|&t| fits(t)).collect();
    // Pasujący teren w głębi, a gdy go tam nie ma – pasujący bliżej skraju (byle nie na samym skraju).
    let fitting_any: Vec<usize> = candidates.iter().copied().filter(|&t| fits(t)).collect();
    let from = if !fitting.is_empty() {
        &fitting
    } else if !fitting_any.is_empty() {
        &fitting_any
    } else {
        &pool
    };
    let extraction = farthest(from, &placed).unwrap();
    out[SLOT_EXTRACTION] = to_slot(extraction);
    placed.push(extraction);
    let factory = farthest(&pool, &placed).unwrap_or(extraction);
    out[SLOT_FACTORY] = to_slot(factory);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MapGenParams, generate};

    #[test]
    fn every_province_gets_distinct_slots_inside_it() {
        let map = generate(&MapGenParams { width: 320, height: 200, ..Default::default() });
        let w = map.width as usize;
        for p in &map.provinces {
            let set: Vec<[u16; 2]> = p.slots.iter().copied().filter(|&s| s != NO_SLOT).collect();
            for &[x, y] in &set {
                assert_eq!(map.province[y as usize * w + x as usize], p.id, "miejsce w prowincji {}", p.id);
            }
            let mut uniq = set.clone();
            uniq.sort_unstable();
            uniq.dedup();
            assert!(uniq.len() == set.len() || p.area < 20, "różne miejsca w prowincji {}", p.id);
            assert_eq!(p.slots[SLOT_PORT] != NO_SLOT, p.coastal, "port tylko nad morzem ({})", p.id);
            assert!((1..=INFRA_START_MAX).contains(&p.infrastructure));
        }
        let again = generate(&MapGenParams { width: 320, height: 200, ..Default::default() });
        assert!(again.provinces.iter().zip(&map.provinces).all(|(a, b)| a.slots == b.slots), "deterministycznie");
    }

    #[test]
    fn start_infrastructure_follows_province_features() {
        let steppe = Biome::Steppe as u8;
        assert_eq!(start_infrastructure(steppe, 100, false, false, 0), 2);
        assert_eq!(start_infrastructure(steppe, 100, false, true, 0), 3);
        assert_eq!(start_infrastructure(Biome::Desert as u8, 100, true, false, 0), 1, "przycięte do 1");
        assert_eq!(start_infrastructure(steppe, 100, false, false, 60), 1, "przewaga wyżyn");
        assert_eq!(start_infrastructure(Biome::Tundra as u8, 100, false, true, 0), 2);
    }
}
