//! Góry nieprzechodnie.
//!
//! Góry (i rzeki płynące przez góry) blokują ruch jednostek i nie należą do żadnej prowincji.
//! Obszary odcięte górami są jak wyspy – osobne lądy dla prowincji. Przejścia przez góry
//! (np. tunele) będą mechaniką rozgrywki, nie generatora.
//!
//! Kieszenie dostępnego lądu zamknięte w górach mniejsze niż `POCKET_MIN` kafli stają się górami
//! (hale), więc etap zmienia teren i działa przed roślinnością i prowincjami. Bez losowości.

use crate::{util::components, MapGenParams, Terrain};

/// Kieszenie dostępnego lądu zamknięte w górach mniejsze niż tyle kafli stają się górami.
const POCKET_MIN: u32 = 40;

pub struct Mountains {
    /// Kafel lądu albo rzeki nieprzechodni i niczyj (góry, rzeki w górach).
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

pub fn build(p: &MapGenParams, terrain: &mut [Terrain]) -> Mountains {
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
    let (lab, sizes) = components(w, h, |i| open(&blocked, terrain, i));
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
    for i in 0..n {
        if lab[i] != u32::MAX && sizes[lab[i] as usize] < POCKET_MIN && touches[lab[i] as usize] {
            blocked[i] = true;
            if terrain[i] != Terrain::River {
                terrain[i] = Terrain::Mountains;
            }
        }
    }
    Mountains { blocked }
}
