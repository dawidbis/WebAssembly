//! Ostatnie szlify (faza 3, po prowincjach): wykrywanie prowincji zablokowanych.
//!
//! Prowincja jest **zablokowana**, gdy nie ma żadnej sąsiedniej prowincji i nie ma dostępu do
//! oceanu – np. dolina zamknięta górami albo lodowcem ze wszystkich stron. Gracz nie może jej
//! wybrać. Sąsiedztwo: kafle prowincji stykające się bokiem albo leżące naprzeciw siebie
//! w poprzek rzeki (do `RIVER_CROSS` kafli wody rzecznej – rzeka jest granicą, nie przepaścią).
//!
//! Bez losowości; nie zmienia terenu, biomów ani przebiegu granic.

use crate::{MapData, Terrain};

/// Najszersza rzeka, przez którą prowincje wciąż sąsiadują (kafle).
const RIVER_CROSS: usize = 4;

/// Oznacza zablokowane prowincje (`Province::blocked`) i wpisuje ich liczbę do statystyk.
pub fn polish(map: &mut MapData) {
    let (w, h) = (map.width as usize, map.height as usize);
    let n = map.provinces.len();
    let mut has_neighbour = vec![false; n];
    let terrain = &map.terrain;
    let province = &map.province;
    let river = Terrain::River as u8;
    for i in 0..w * h {
        let p = province[i];
        if p == 0 || has_neighbour[p as usize - 1] {
            continue;
        }
        let (x, y) = ((i % w) as isize, (i / w) as isize);
        for (dx, dy) in [(1isize, 0isize), (-1, 0), (0, 1), (0, -1)] {
            // Pierwszy kafel w tym kierunku, który nie jest rzeką (najwyżej `RIVER_CROSS` kafli rzeki).
            let mut k = 1;
            let found = loop {
                let (nx, ny) = (x + dx * k as isize, y + dy * k as isize);
                if nx < 0 || ny < 0 || nx >= w as isize || ny >= h as isize {
                    break None;
                }
                let j = ny as usize * w + nx as usize;
                if terrain[j] != river {
                    break Some(j);
                }
                if k > RIVER_CROSS {
                    break None;
                }
                k += 1;
            };
            if let Some(j) = found {
                if province[j] != 0 && province[j] != p {
                    has_neighbour[p as usize - 1] = true;
                    has_neighbour[province[j] as usize - 1] = true;
                    break;
                }
            }
        }
    }
    let mut blocked = 0;
    for (k, pr) in map.provinces.iter_mut().enumerate() {
        pr.blocked = !pr.coastal && !has_neighbour[k];
        blocked += pr.blocked as u32;
    }
    map.stats.blocked_provinces = blocked;
}
