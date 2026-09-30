//! Góry nieprzechodnie i przełęcze.
//!
//! Góry (i rzeki płynące przez góry) blokują ruch jednostek i nie należą do żadnej prowincji.
//! Żeby góry nie odcinały kawałków lądu, ten etap wycina przełęcze – ścieżki wyżyny przez pasmo:
//!
//! 1. **Spójność.** Każdy ląd (razem z górami) musi dać się przejść w całości. Fale Dijkstry
//!    ruszają ze wszystkich dostępnych kafli przez góry (koszt rośnie z wysokością, więc
//!    przełęcz wybiera najniższe siodło, chętnie doliną rzeki). Gdzie spotkają się fale dwóch
//!    odciętych obszarów, jest kandydat na przełęcz; minimalne drzewo rozpinające (Kruskal)
//!    wybiera najtańsze połączenia. Kieszenie mniejsze niż `POCKET_MIN` kafli stają się górami.
//! 2. **Gęstość.** Dodatkowa przełęcz tam, gdzie pasmo jest wąskie (do `passMaxLength` kafli),
//!    a obejście po dostępnym lądzie dłuższe niż `passDetour` kafli.
//!
//! Etap nie używa losowości. Zmienia teren (góry → wyżyna na przełęczach), więc działa przed
//! roślinnością i prowincjami.

use std::{
    cmp::Reverse,
    collections::{BTreeSet, BinaryHeap},
};

use crate::{util::components, MapGenParams, Terrain};

/// Kieszenie dostępnego lądu zamknięte w górach mniejsze niż tyle kafli stają się górami.
const POCKET_MIN: u32 = 40;
/// Oczko siatki do rozstawiania przełęczy (kafle).
const CELL: usize = 8;

pub struct Passes {
    /// Kafel lądu albo rzeki nieprzechodni i niczyj (góry, rzeki w górach).
    pub blocked: Vec<bool>,
    /// Liczba wyciętych przełęczy.
    pub count: u32,
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

fn neighbors4(w: usize, h: usize, i: usize) -> impl Iterator<Item = usize> {
    let (x, y) = (i % w, i / w);
    [
        (x > 0).then(|| i - 1),
        (x + 1 < w).then(|| i + 1),
        (y > 0).then(|| i - w),
        (y + 1 < h).then(|| i + w),
    ]
    .into_iter()
    .flatten()
}

pub fn build(p: &MapGenParams, terrain: &mut [Terrain], shade: &mut [u8]) -> Passes {
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

    // Maleńkie kieszenie w górach – to po prostu góry (hale), nie osobne krainy.
    let (lab, sizes) = components(w, h, |i| open(&blocked, terrain, i));
    for i in 0..n {
        if lab[i] != u32::MAX && sizes[lab[i] as usize] < POCKET_MIN {
            blocked[i] = true;
            if terrain[i] != Terrain::River {
                terrain[i] = Terrain::Mountains;
            }
        }
    }
    let (comp, _) = components(w, h, |i| open(&blocked, terrain, i));

    // --- Fale przez góry od wszystkich dostępnych kafli -------------------------------------
    // dist = koszt, steps = liczba kafli gór, origin = dostępny kafel startowy, pred = poprzednik.
    const NONE: u32 = u32::MAX;
    let mut dist = vec![NONE; n];
    let mut steps = vec![0u16; n];
    let mut origin = vec![NONE; n];
    let mut pred = vec![NONE; n];
    let mut heap = BinaryHeap::new();
    let cost = |shade: &[u8], j: usize| 4 + shade[j] as u32 / 16;
    for i in 0..n {
        if !open(&blocked, terrain, i) {
            continue;
        }
        for j in neighbors4(w, h, i) {
            if blocked[j] {
                let d = cost(shade, j);
                if d < dist[j] || (d == dist[j] && (i as u32) < origin[j]) {
                    dist[j] = d;
                    steps[j] = 1;
                    origin[j] = i as u32;
                    pred[j] = i as u32;
                    heap.push(Reverse((d, j as u32)));
                }
            }
        }
    }
    while let Some(Reverse((d, i))) = heap.pop() {
        let i = i as usize;
        if d != dist[i] {
            continue;
        }
        for j in neighbors4(w, h, i) {
            if !blocked[j] {
                continue;
            }
            let nd = d + cost(shade, j);
            if nd < dist[j] {
                dist[j] = nd;
                steps[j] = steps[i] + 1;
                origin[j] = origin[i];
                pred[j] = i as u32;
                heap.push(Reverse((nd, j as u32)));
            }
        }
    }

    // Kandydaci: styk dwóch fal (albo fali z dostępnym kaflem innego obszaru).
    // (koszt, długość w kaflach, kafel gór, sąsiad, początek A, początek B)
    let mut candidates: Vec<(u32, u32, usize, usize, usize, usize)> = Vec::new();
    for i in 0..n {
        if !blocked[i] || origin[i] == NONE {
            continue;
        }
        let a = origin[i] as usize;
        for j in neighbors4(w, h, i) {
            if j < i && blocked[j] {
                continue; // para gór rozpatrzona od strony mniejszego indeksu
            }
            let (b, c, len) = if blocked[j] {
                if origin[j] == NONE {
                    continue;
                }
                (origin[j] as usize, dist[i] + dist[j], steps[i] as u32 + steps[j] as u32)
            } else if open(&blocked, terrain, j) {
                (j, dist[i], steps[i] as u32)
            } else {
                continue;
            };
            if b != a {
                candidates.push((c, len, i, j, a, b));
            }
        }
    }
    candidates.sort_unstable();

    let mut count = 0;
    let carve = |terrain: &mut [Terrain], shade: &mut [u8], blocked: &mut [bool], i: usize, j: usize| {
        for start in [i, j] {
            let mut k = start;
            while blocked[k] {
                blocked[k] = false;
                if terrain[k] == Terrain::Mountains {
                    terrain[k] = Terrain::Highlands;
                    // Przełęcz jest niżej niż otaczające szczyty.
                    shade[k] = (shade[k] as u32 * 3 / 4) as u8;
                }
                // Poszerzenie do dwóch kafli (w prawo), żeby przełęcz była czytelna.
                if k % w + 1 < w && terrain[k + 1] == Terrain::Mountains {
                    terrain[k + 1] = Terrain::Highlands;
                    shade[k + 1] = (shade[k + 1] as u32 * 3 / 4) as u8;
                    blocked[k + 1] = false;
                }
                k = pred[k] as usize;
            }
        }
    };

    // --- 1. Spójność: Kruskal po obszarach ------------------------------------------------------
    let comps = comp.iter().filter(|&&c| c != NONE).max().map_or(0, |&c| c as usize + 1);
    let mut parent: Vec<usize> = (0..comps).collect();
    fn find(parent: &mut [usize], mut x: usize) -> usize {
        while parent[x] != x {
            parent[x] = parent[parent[x]];
            x = parent[x];
        }
        x
    }
    let mut cells = vec![false; w.div_ceil(CELL) * h.div_ceil(CELL)];
    let cw = w.div_ceil(CELL);
    let mark = |cells: &mut [bool], i: usize, r: isize| {
        let (cx, cy) = ((i % w / CELL) as isize, (i / w / CELL) as isize);
        for dy in -r..=r {
            for dx in -r..=r {
                let (x, y) = (cx + dx, cy + dy);
                if x >= 0 && y >= 0 && (x as usize) < cw && (y as usize) < h.div_ceil(CELL) {
                    cells[y as usize * cw + x as usize] = true;
                }
            }
        }
    };
    for &(_, _, i, j, a, b) in &candidates {
        let (ca, cb) = (find(&mut parent, comp[a] as usize), find(&mut parent, comp[b] as usize));
        if ca != cb {
            parent[ca] = cb;
            carve(terrain, shade, &mut blocked, i, j);
            mark(&mut cells, i, (p.pass_detour as usize / CELL / 2) as isize);
            count += 1;
        }
    }

    // --- 2. Gęstość: przełęcz przez wąskie pasmo, gdy obejście jest za długie -----------------
    let mut seen = vec![0u32; n];
    let mut stamp = 0u32;
    let mut queue: Vec<(usize, u32)> = Vec::new();
    // Pary oczek (strona A, strona B), między którymi obejście okazało się krótkie.
    let mut near_pairs: BTreeSet<(usize, usize)> = BTreeSet::new();
    let cell_of = |i: usize| (i / w / CELL) * cw + i % w / CELL;
    for &(_, len, i, j, a, b) in &candidates {
        if len < 2 || len > p.pass_max_length || cells[cell_of(i)] {
            continue;
        }
        let pair = (cell_of(a).min(cell_of(b)), cell_of(a).max(cell_of(b)));
        if near_pairs.contains(&pair) {
            continue;
        }
        // BFS po dostępnym lądzie z A: czy B jest bliżej niż `pass_detour`?
        stamp += 1;
        queue.clear();
        queue.push((a, 0));
        seen[a] = stamp;
        let mut head = 0;
        let mut near = false;
        while head < queue.len() {
            let (k, d) = queue[head];
            head += 1;
            if k == b {
                near = true;
                break;
            }
            if d >= p.pass_detour {
                continue;
            }
            for m in neighbors4(w, h, k) {
                if seen[m] != stamp && open(&blocked, terrain, m) {
                    seen[m] = stamp;
                    queue.push((m, d + 1));
                }
            }
        }
        if near {
            near_pairs.insert(pair);
        } else {
            carve(terrain, shade, &mut blocked, i, j);
            mark(&mut cells, i, (p.pass_detour as usize / CELL / 2) as isize);
            count += 1;
        }
    }

    Passes { blocked, count }
}
