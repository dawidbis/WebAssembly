//! Stałe miejsca budynków w prowincjach (kafel na mapie dla każdego rodzaju poza infrastrukturą,
//! umocnieniami i centrum dowodzenia – to ostatnie sygnalizują gwiazdki nad miastem stolicy). Liczone z niezmiennej
//! mapy przy `Game::from_map`, deterministycznie – u wszystkich graczy te same; poza hashem stanu.
//!
//! - **Miasto:** w głębi prowincji, możliwie blisko jej środka.
//! - **Port:** kafel prowincji przy morzu, najbliżej miasta (prowincja bez morza – brak).
//! - **Wydobycie:** na terenie pasującym do surowca (kopalnia – wyżyny, tartak i myśliwi – las, pola,
//!   plantacje i rancza – niziny bez lasu), możliwie daleko od już postawionych.
//! - **Fabryka, uniwersytet:** rozstawione jak najdalej od pozostałych – w głębi prowincji (co najmniej
//!   `INNER_DEPTH_PCT`% największej głębokości), żeby nie lądowały na skraju.

use std::collections::VecDeque;

use crate::{
    buildings::Building,
    goods::{EXTRACTION, Extraction},
    mapgen::{MapData, Terrain},
};

/// Budynki z miejscem na mapie (kolejność wyniku `slots`).
pub const SLOT_BUILDINGS: [Building; SLOT_COUNT] =
    [Building::City, Building::Port, Building::Extraction, Building::Factory, Building::University];
/// Liczba budynków z miejscem na mapie.
pub const SLOT_COUNT: usize = 5;
/// Brak miejsca (np. port w prowincji bez morza).
pub const NO_SLOT: (u16, u16) = (u16::MAX, u16::MAX);
/// Kafle co najmniej tak głęboko w prowincji (od jej skraju) są kandydatami na miejsca.
const MIN_DEPTH: u32 = 2;
/// Wydobycie, fabryka i uniwersytet – na kaflach co najmniej tak głęboko (% największej głębokości prowincji).
const INNER_DEPTH_PCT: u32 = 50;

/// Miejsca budynków każdej prowincji: `slots[id - 1][k]` – kafel (x, y) budynku `SLOT_BUILDINGS[k]`.
pub fn slots(map: &MapData, bounds: &[[u32; 4]]) -> Vec<[(u16, u16); SLOT_COUNT]> {
    let w = map.width as usize;
    map.provinces
        .iter()
        .enumerate()
        .map(|(k, p)| {
            province_slots(map, w, k as u16 + 1, bounds[k], p.center_x as usize, p.center_y as usize, p.resource)
        })
        .collect()
}

fn province_slots(
    map: &MapData,
    w: usize,
    id: u16,
    [x0, y0, x1, y1]: [u32; 4],
    cx: usize,
    cy: usize,
    resource: u8,
) -> [(u16, u16); SLOT_COUNT] {
    let h = map.height as usize;
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
    if tiles.is_empty() {
        return [NO_SLOT; SLOT_COUNT];
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
    let candidates: Vec<usize> = (0..tiles.len()).filter(|&k| depth[k] >= need).collect();
    let xy = |i: usize| ((i % w) as i64, (i / w) as i64);
    let dist2 = |a: usize, b: usize| {
        let ((ax, ay), (bx, by)) = (xy(a), xy(b));
        (ax - bx).pow(2) + (ay - by).pow(2)
    };
    let center = cy * w + cx;
    let mut placed: Vec<usize> = Vec::new();
    let mut out = [NO_SLOT; SLOT_COUNT];
    let to_slot = |i: usize| ((i % w) as u16, (i / w) as u16);

    // Miasto: blisko środka, w głębi.
    let city = candidates.iter().map(|&k| tiles[k]).min_by_key(|&t| (dist2(t, center), t)).unwrap();
    out[0] = to_slot(city);
    placed.push(city);

    // Port: kafel prowincji przy oceanie, najbliżej miasta.
    let ocean = |i: usize| neighbours(i).into_iter().flatten().any(|n| map.terrain[n] == Terrain::Ocean as u8);
    if let Some(port) = tiles.iter().copied().filter(|&t| ocean(t)).min_by_key(|&t| (dist2(t, city), t)) {
        out[1] = to_slot(port);
        placed.push(port);
    }

    // Najdalej od postawionych (największa najmniejsza odległość), spośród `pool`; remis – niższy kafel.
    let farthest = |pool: &[usize], placed: &[usize]| {
        pool.iter()
            .copied()
            .max_by_key(|&t| (placed.iter().map(|&p| dist2(t, p)).min().unwrap_or(0), std::cmp::Reverse(t)))
    };
    // Wydobycie: teren pasujący do surowca.
    let forest = |i: usize| map.forest[i] >= 128;
    let highlands = |i: usize| map.terrain[i] == Terrain::Highlands as u8;
    let fits = |i: usize| match EXTRACTION[resource as usize] {
        Extraction::Mine => highlands(i),
        Extraction::Sawmill | Extraction::Hunters => forest(i),
        Extraction::Farm | Extraction::Plantation | Extraction::Ranch => !highlands(i) && !forest(i),
        Extraction::OilWell | Extraction::Quarry => true,
    };
    // Rozstawiane budynki tylko w głębi prowincji (co najmniej połowa największej głębokości) – inaczej
    // „najdalej od pozostałych” spycha je na skraj.
    let inner_depth = need.max(deepest * INNER_DEPTH_PCT / 100);
    let pool: Vec<usize> = (0..tiles.len()).filter(|&k| depth[k] >= inner_depth).map(|k| tiles[k]).collect();
    let fitting: Vec<usize> = pool.iter().copied().filter(|&t| fits(t)).collect();
    // Pasujący teren w głębi, a gdy go tam nie ma – pasujący bliżej skraju (byle nie na samym skraju).
    let fitting_any: Vec<usize> = candidates.iter().map(|&k| tiles[k]).filter(|&t| fits(t)).collect();
    let from = if !fitting.is_empty() {
        &fitting
    } else if !fitting_any.is_empty() {
        &fitting_any
    } else {
        &pool
    };
    let extraction = farthest(from, &placed).unwrap();
    out[2] = to_slot(extraction);
    placed.push(extraction);
    // Fabryka, uniwersytet: jak najdalej od pozostałych.
    for slot in out.iter_mut().skip(3) {
        let t = farthest(&pool, &placed).unwrap();
        *slot = to_slot(t);
        placed.push(t);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mapgen::{self, MapGenParams};

    #[test]
    fn every_province_gets_distinct_slots_inside_it() {
        let map = mapgen::generate(&MapGenParams { width: 320, height: 200, ..Default::default() });
        let w = map.width as usize;
        let mut bounds = vec![[u32::MAX, u32::MAX, 0, 0]; map.provinces.len()];
        for (i, &p) in map.province.iter().enumerate() {
            if p != 0 {
                let (x, y) = ((i % w) as u32, (i / w) as u32);
                let b = &mut bounds[p as usize - 1];
                *b = [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)];
            }
        }
        let all = slots(&map, &bounds);
        assert_eq!(all.len(), map.provinces.len());
        for (k, s) in all.iter().enumerate() {
            let id = k as u16 + 1;
            let set: Vec<(u16, u16)> = s.iter().copied().filter(|&p| p != NO_SLOT).collect();
            for &(x, y) in &set {
                assert_eq!(map.province[y as usize * w + x as usize], id, "miejsce w prowincji {id}");
            }
            let mut uniq = set.clone();
            uniq.sort_unstable();
            uniq.dedup();
            assert_eq!(uniq.len(), set.len(), "różne miejsca w prowincji {id}");
            assert_eq!(s[1] != NO_SLOT, map.provinces[k].coastal, "port tylko nad morzem ({id})");
        }
        assert_eq!(slots(&map, &bounds), all, "deterministycznie");
    }
}
