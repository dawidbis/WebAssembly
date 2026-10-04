//! Góry i lądolód nieprzechodnie.
//!
//! Góry (i rzeki płynące przez góry) oraz lądolód (kafle, na których dominuje biom lądolodu,
//! razem z rzekami) blokują ruch jednostek i nie należą do żadnej prowincji.
//! Obszary odcięte górami są jak wyspy – osobne lądy dla prowincji. Przejścia przez góry
//! (np. tunele) będą mechaniką rozgrywki, nie generatora.
//!
//! Kieszenie dostępnego lądu zamknięte w górach mniejsze niż `POCKET_MIN` kafli stają się górami
//! (hale), więc etap zmienia teren i działa przed roślinnością i prowincjami. Teren zależy tylko
//! od gór – kieszenie zamknięte w lodzie są potem tylko nieprzechodnie (bez zmiany terenu), więc
//! ustawienia biomów nie zmieniają terenu. Bez losowości.

use crate::{util::components, MapGenParams, Terrain};

/// Kieszenie dostępnego lądu zamknięte w górach mniejsze niż tyle kafli stają się górami.
const POCKET_MIN: u32 = 40;

pub struct Mountains {
    /// Kafel lądu albo rzeki nieprzechodni i niczyj (góry, rzeki w górach, lądolód).
    pub blocked: Vec<bool>,
}

/// Rzeka płynąca przez góry: w otoczeniu 5 × 5 więcej gór niż dostępnego lądu.
fn mountain_river(terrain: &[Terrain], w: usize, h: usize, i: usize) -> bool {
    let (x, y) = ((i % w) as isize, (i / w) as isize);
    let (mut rock, mut open) = (0, 0);
    for dy in -2..=2 {
        for dx in -2..=2 {
            let (nx, ny) = (x + dx, y + dy);
            if nx < 0 || ny < 0 || nx >= w as isize || ny >= h as isize {
                continue;
            }
            match terrain[ny as usize * w + nx as usize] {
                Terrain::Mountains => rock += 1,
                Terrain::Plains | Terrain::Highlands => open += 1,
                _ => {}
            }
        }
    }
    rock > open
}

/// `ice[i]` – na kaflu dominuje lądolód.
pub fn build(p: &MapGenParams, terrain: &mut [Terrain], ice: &[bool]) -> Mountains {
    let (w, h) = (p.width as usize, p.height as usize);
    let n = w * h;
    let mut blocked: Vec<bool> = (0..n)
        .map(|i| match terrain[i] {
            Terrain::Mountains => true,
            Terrain::River => mountain_river(terrain, w, h, i),
            _ => false,
        })
        .collect();
    let open = |blocked: &[bool], terrain: &[Terrain], i: usize| {
        !blocked[i] && (terrain[i].is_land() || terrain[i] == Terrain::River)
    };

    // Maleńkie kieszenie przy górach – to po prostu góry (hale), nie osobne krainy.
    // Kieszeń musi stykać się z górami, więc małe wyspy zostają lądem.
    for i in pockets(w, h, terrain, &blocked) {
        blocked[i] = true;
        if terrain[i] != Terrain::River {
            terrain[i] = Terrain::Mountains;
        }
    }
    // Lądolód (z rzekami) i kieszenie zamknięte w lodzie – nieprzechodnie, teren bez zmian.
    for i in 0..n {
        if ice[i] && open(&blocked, terrain, i) {
            blocked[i] = true;
        }
    }
    for i in pockets(w, h, terrain, &blocked) {
        blocked[i] = true;
    }
    Mountains { blocked }
}

/// Kafle dostępnego lądu w spójnych kawałkach mniejszych niż `POCKET_MIN`, które stykają się
/// z kaflem nieprzechodnim.
fn pockets(w: usize, h: usize, terrain: &[Terrain], blocked: &[bool]) -> Vec<usize> {
    let n = w * h;
    let open = |i: usize| !blocked[i] && (terrain[i].is_land() || terrain[i] == Terrain::River);
    let (lab, sizes) = components(w, h, open);
    let mut touches = vec![false; sizes.len()];
    for i in 0..n {
        if lab[i] == u32::MAX {
            continue;
        }
        let (x, y) = (i % w, i / w);
        let near = [(x > 0).then(|| i - 1), (x + 1 < w).then(|| i + 1), (y > 0).then(|| i - w), (y + 1 < h).then(|| i + w)];
        if near.into_iter().flatten().any(|j| blocked[j]) {
            touches[lab[i] as usize] = true;
        }
    }
    (0..n).filter(|&i| lab[i] != u32::MAX && sizes[lab[i] as usize] < POCKET_MIN && touches[lab[i] as usize]).collect()
}
