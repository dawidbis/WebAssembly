//! Góry nieprzechodnie.
//!
//! Góry blokują ruch jednostek i nie należą do żadnej prowincji (tak jak woda – ocean, jeziora
//! i rzeki).
//! Obszary odcięte górami są jak wyspy – osobne lądy dla prowincji. Przejścia przez góry
//! (np. tunele) będą mechaniką rozgrywki, nie generatora.
//!
//! Kieszenie dostępnego lądu zamknięte w górach mniejsze niż `POCKET_MIN` kafli stają się górami
//! (hale), więc etap zmienia teren i działa przed roślinnością i prowincjami. Bez losowości.

use crate::{util::components, MapGenParams, Terrain};

/// Kieszenie dostępnego lądu zamknięte w górach mniejsze niż tyle kafli stają się górami.
const POCKET_MIN: u32 = 40;

pub struct Mountains {
    /// Kafel lądu nieprzechodni i niczyj (góry).
    pub blocked: Vec<bool>,
}

pub fn build(p: &MapGenParams, terrain: &mut [Terrain]) -> Mountains {
    let (w, h) = (p.width as usize, p.height as usize);
    let mut blocked: Vec<bool> = terrain.iter().map(|&t| t == Terrain::Mountains).collect();
    // Maleńkie kieszenie przy górach – to po prostu góry (hale), nie osobne krainy.
    // Kieszeń musi stykać się z górami, więc małe wyspy zostają lądem.
    for i in pockets(w, h, terrain, &blocked) {
        blocked[i] = true;
        terrain[i] = Terrain::Mountains;
    }
    Mountains { blocked }
}

/// Lodowiec (`glacier`): lądolód (`ice[i]`) jest nieprzechodni i niczyj jak góry, razem
/// z maleńkimi kieszeniami zamkniętymi w lodzie. Teren się nie zmienia, więc ustawienia
/// biomów nadal nie zmieniają terenu.
pub fn block_glacier(w: usize, h: usize, terrain: &[Terrain], ice: &[bool], blocked: &mut [bool]) {
    for i in 0..w * h {
        if ice[i] && terrain[i].is_land() {
            blocked[i] = true;
        }
    }
    for i in pockets(w, h, terrain, blocked) {
        blocked[i] = true;
    }
}

/// Kafle dostępnego lądu w spójnych kawałkach mniejszych niż `POCKET_MIN`, które stykają się
/// z kaflem nieprzechodnim.
fn pockets(w: usize, h: usize, terrain: &[Terrain], blocked: &[bool]) -> Vec<usize> {
    let n = w * h;
    let (lab, sizes) = components(w, h, |i| !blocked[i] && terrain[i].is_land());
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
