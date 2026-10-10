//! Ostatnie szlify (faza 3, po prowincjach): dostęp do każdej prowincji.
//!
//! Prowincje łączą się w grupy: kafle stykające się bokiem albo leżące naprzeciw siebie w poprzek
//! rzeki (do `RIVER_CROSS` kafli wody rzecznej – przyszły most albo bród), a prowincje nadmorskie
//! łączą się przez ocean. Grupa z oceanem (bez oceanu na mapie – największa) jest zdrowa.
//! Każda inna grupa – np. doliny zamknięte górami albo lodowcem – musi mieć **tunel**: najkrótszą
//! drogę przez kafle gór, lodowca i rzek (nie przez jeziora ani morze) do prowincji, do której już
//! da się dojść. Dłuższy niż `tunnel_max` – grupa przestaje być prowincjami i zostaje enklawą
//! (ląd niczyj; mieszka w niej yeti). Dzięki temu do każdej prowincji da się dojść.
//!
//! Bez losowości; nie zmienia terenu ani biomów. Enklawy zmieniają `province` (numeracja od nowa).

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use crate::provinces::{Enclave, fill_stats};
use crate::{MapData, MapGenParams, Terrain};

/// Najszersza rzeka, przez którą prowincje wciąż sąsiadują (kafle).
const RIVER_CROSS: usize = 4;

/// Wyznacza tunele (`Province::tunnel`) i enklawy (`MapData::enclaves`). Zwraca `true`, gdy
/// zmieniła się numeracja prowincji (powstały enklawy).
pub fn polish(map: &mut MapData, params: &MapGenParams) -> bool {
    let tunnel_max = params.sanitized().tunnel_max;
    let (w, h) = (map.width as usize, map.height as usize);
    let count = map.provinces.len();
    map.enclaves.clear();
    map.stats.tunnel_provinces = 0;
    map.stats.enclaves = 0;
    if count == 0 {
        return false;
    }
    let groups = group_provinces(map, w, h);

    // Zdrowa grupa: z oceanem, a bez oceanu – o największej powierzchni.
    let ocean = count;
    let healthy = if map.provinces.iter().any(|p| p.coastal) {
        groups[ocean]
    } else {
        let mut area = vec![0u64; count + 1];
        for (k, p) in map.provinces.iter().enumerate() {
            area[groups[k]] += p.area as u64;
        }
        (0..=count).max_by_key(|&g| (area[g], Reverse(g))).unwrap()
    };

    // --- Tunele: Dijkstra od prowincji osiągalnych przez kafle gór, lodowca i rzek ------------
    // Długość tunelu liczy się od ostatniej osiągalnej prowincji: grupa osiągnięta tunelem staje
    // się źródłem (odległość 0) dla kolejnych.
    let terrain = &map.terrain;
    let province = &map.province;
    let open =
        |j: usize| province[j] == 0 && (terrain[j] == Terrain::River as u8 || terrain[j] >= Terrain::Plains as u8);
    let mut reached: Vec<Option<u32>> = vec![None; count + 1];
    reached[healthy] = Some(0);
    let mut dist = vec![u32::MAX; w * h];
    let mut heap = BinaryHeap::new();
    let add_sources = |g: usize, dist: &mut Vec<u32>, heap: &mut BinaryHeap<Reverse<(u32, u32)>>| {
        for i in 0..w * h {
            let p = province[i];
            if p != 0 && groups[p as usize - 1] == g && neighbors4(w, h, i).any(open) {
                dist[i] = 0;
                heap.push(Reverse((0, i as u32)));
            }
        }
    };
    add_sources(healthy, &mut dist, &mut heap);
    while let Some(Reverse((d, i))) = heap.pop() {
        let i = i as usize;
        if d > dist[i] {
            continue;
        }
        for j in neighbors4(w, h, i) {
            let p = province[j];
            if p != 0 {
                let g = groups[p as usize - 1];
                if reached[g].is_none() {
                    reached[g] = Some(d.max(1));
                    add_sources(g, &mut dist, &mut heap);
                }
            } else if open(j) && d < tunnel_max && d + 1 < dist[j] {
                dist[j] = d + 1;
                heap.push(Reverse((d + 1, j as u32)));
            }
        }
    }

    // --- Grupy bez tunelu w zasięgu → enklawy ------------------------------------------------
    let dropped: Vec<bool> = (0..count).map(|k| reached[groups[k]].is_none()).collect();
    let mut enclave_of = vec![usize::MAX; count + 1];
    let mut sums: Vec<(f64, f64, u32)> = Vec::new();
    for k in 0..count {
        if dropped[k] && enclave_of[groups[k]] == usize::MAX {
            enclave_of[groups[k]] = sums.len();
            sums.push((0.0, 0.0, 0));
        }
    }
    let mut new_id = vec![0u16; count + 1];
    let mut next = 0u16;
    for k in 0..count {
        if !dropped[k] {
            next += 1;
            new_id[k + 1] = next;
        }
    }
    let changed = !sums.is_empty();
    let mut owner = vec![usize::MAX; if changed { w * h } else { 0 }];
    if changed {
        for i in 0..w * h {
            let p = map.province[i] as usize;
            if p == 0 {
                continue;
            }
            if dropped[p - 1] {
                let e = enclave_of[groups[p - 1]];
                owner[i] = e;
                let s = &mut sums[e];
                s.0 += (i % w) as f64;
                s.1 += (i / w) as f64;
                s.2 += 1;
            }
            map.province[i] = new_id[p];
        }
    }
    // Środek enklawy: jej kafel najbliższy środka ciężkości.
    let mut best = vec![(f64::MAX, 0usize); sums.len()];
    for (i, &e) in owner.iter().enumerate() {
        if e == usize::MAX {
            continue;
        }
        let (cx, cy) = (sums[e].0 / sums[e].2 as f64, sums[e].1 / sums[e].2 as f64);
        let d = ((i % w) as f64 - cx).powi(2) + ((i / w) as f64 - cy).powi(2);
        if d < best[e].0 {
            best[e] = (d, i);
        }
    }
    map.enclaves = sums
        .iter()
        .zip(&best)
        .map(|(s, &(_, i))| Enclave { area: s.2, center_x: (i % w) as u16, center_y: (i / w) as u16 })
        .collect();

    // --- Prowincje: tunele, nowa numeracja, statystyki ----------------------------------------
    let old = std::mem::take(&mut map.provinces);
    for (k, mut pr) in old.into_iter().enumerate() {
        if dropped[k] {
            continue;
        }
        pr.id = new_id[k + 1];
        pr.tunnel = reached[groups[k]].unwrap().min(u16::MAX as u32) as u16;
        map.stats.tunnel_provinces += (pr.tunnel > 0) as u32;
        map.provinces.push(pr);
    }
    fill_stats(&map.provinces, &mut map.stats);
    map.stats.enclaves = map.enclaves.len() as u32;
    changed
}

/// Grupy połączonych prowincji: dla prowincji `k` (od 0) – numer grupy; ostatni element
/// (`groups[liczba prowincji]`) to grupa oceanu.
fn group_provinces(map: &MapData, w: usize, h: usize) -> Vec<usize> {
    let count = map.provinces.len();
    let ocean = count;
    let mut parent: Vec<usize> = (0..=count).collect();
    fn find(parent: &mut [usize], mut a: usize) -> usize {
        while parent[a] != a {
            parent[a] = parent[parent[a]];
            a = parent[a];
        }
        a
    }
    let union = |parent: &mut Vec<usize>, a: usize, b: usize| {
        let (ra, rb) = (find(parent, a), find(parent, b));
        // Mniejszy numer zostaje korzeniem – deterministycznie.
        if ra != rb {
            parent[ra.max(rb)] = ra.min(rb);
        }
    };
    for (k, p) in map.provinces.iter().enumerate() {
        if p.coastal {
            union(&mut parent, k, ocean);
        }
    }
    let river = Terrain::River as u8;
    for i in 0..w * h {
        let p = map.province[i];
        if p == 0 {
            continue;
        }
        let (x, y) = ((i % w) as isize, (i / w) as isize);
        for (dx, dy) in [(1isize, 0isize), (0, 1), (-1, 0), (0, -1)] {
            // Pierwszy kafel w tym kierunku, który nie jest rzeką (najwyżej `RIVER_CROSS` kafli rzeki).
            for k in 1..=RIVER_CROSS as isize + 1 {
                let (nx, ny) = (x + dx * k, y + dy * k);
                if nx < 0 || ny < 0 || nx >= w as isize || ny >= h as isize {
                    break;
                }
                let j = ny as usize * w + nx as usize;
                if map.terrain[j] == river {
                    continue;
                }
                let q = map.province[j];
                if q != 0 && q != p {
                    union(&mut parent, p as usize - 1, q as usize - 1);
                }
                break;
            }
        }
    }
    (0..=count).map(|k| find(&mut parent, k)).collect()
}

fn neighbors4(w: usize, h: usize, i: usize) -> impl Iterator<Item = usize> {
    let (x, y) = (i % w, i / w);
    [(x > 0).then(|| i - 1), (x + 1 < w).then(|| i + 1), (y > 0).then(|| i - w), (y + 1 < h).then(|| i + w)]
        .into_iter()
        .flatten()
}
