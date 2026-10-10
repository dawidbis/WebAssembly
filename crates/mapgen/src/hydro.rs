//! Hydrologia: skupiska jezior i rzeki spływające ze szczytów górskich.
//!
//! Rzeki: Priority-Flood (Barnes i in.) wypełnia bezodpływowe dołki, potem każdy kafel
//! dostaje kierunek najbardziej stromego spadku (D8). Powstaje drzewo odpływu,
//! a rzeka to ścieżka po tym drzewie od źródła w górach do morza.

use std::{cmp::Reverse, collections::BinaryHeap};

use fastnoise_lite::FractalType;

use crate::{
    MapGenParams, Terrain,
    layout::Layout,
    relief::Relief,
    util::{CoarseField, Rng, components, fractal, smoothstep},
};

const NONE: u32 = u32::MAX;

/// Zwraca (liczba jezior, liczba rzek).
pub fn build(p: &MapGenParams, l: &Layout, r: &mut Relief, rng: &mut Rng) -> (u32, u32) {
    if p.lakes {
        carve_lakes(p, l, r, rng);
    }
    let lakes = components(r.w, r.h, |i| r.terrain[i] == Terrain::Lake).1.len() as u32;
    let rivers = if p.rivers && p.river_count > 0 { carve_rivers(p, l, r, rng) } else { 0 };
    (lakes, rivers)
}

fn carve_lakes(p: &MapGenParams, l: &Layout, r: &mut Relief, rng: &mut Rng) {
    let (w, h) = (r.w, r.h);
    let cs = l.chunk_size();
    // Wolnozmienny szum wyznacza „pojezierza", szybkozmienny – pojedyncze jeziora.
    let cluster = fractal(rng.noise_seed(), FractalType::FBm, 0.8 / cs, 2);
    let blobs = fractal(rng.noise_seed(), FractalType::FBm, 7.0 / cs, 3);
    let cut = 1.0 - 0.5 * p.lake_amount;

    let lone_thr = 0.95 - 0.06 * p.lake_amount;
    // > 0 tam, gdzie powinno być jezioro (ciągła wartość → gładkie brzegi po interpolacji).
    let score = CoarseField::new(w, h, |fx, fy| {
        let c = smoothstep(cut, cut + 0.12, (cluster.get_noise_2d(fx, fy) + 1.0) * 0.5);
        let b = (blobs.get_noise_2d(fx, fy) + 1.0) * 0.5;
        let in_cluster = if c > 0.0 { b - (0.74 - 0.14 * c) } else { -1.0 };
        in_cluster.max(b - lone_thr)
    });
    let mut candidate = vec![false; w * h];
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            candidate[i] = matches!(r.terrain[i], Terrain::Plains | Terrain::Highlands)
                && r.coast_dist[i] >= 6.0
                && score.get(x, y) > 0.0;
        }
    }

    let (lab, sizes) = components(w, h, |i| candidate[i]);
    // Powierzchnia jeziora jest płaska: najniższa wysokość w jego obrysie.
    let mut surface = vec![f32::MAX; sizes.len()];
    for i in 0..w * h {
        if candidate[i] {
            let id = lab[i] as usize;
            surface[id] = surface[id].min(r.elevation[i]);
        }
    }
    for i in 0..w * h {
        if candidate[i] {
            let id = lab[i] as usize;
            if (p.min_lake_area..=p.max_lake_area).contains(&sizes[id]) {
                r.terrain[i] = Terrain::Lake;
                r.elevation[i] = surface[id];
                r.shade[i] = 0;
            }
        }
    }
}

fn carve_rivers(p: &MapGenParams, l: &Layout, r: &mut Relief, rng: &mut Rng) -> u32 {
    let (w, h) = (r.w, r.h);
    let n = w * h;
    let cs = l.chunk_size();
    let height = |i: usize| (r.elevation[i].max(0.0) * 50_000.0) as u64;
    // Koszt przejścia przez płaskie (wypełnione) obszary. Gładki szum sprawia, że rzeka
    // omija „droższe" miejsca i meandruje, zamiast iść prostą linią po siatce.
    let meander = fractal(rng.noise_seed(), FractalType::FBm, 5.0 / cs, 2);
    let flat_cost = CoarseField::new(w, h, |fx, fy| {
        let m = (meander.get_noise_2d(fx, fy) + 1.0) * 0.5;
        1.0 + 60.0 * m * m
    });

    // --- Priority-Flood: zalewanie od oceanu (i krawędzi mapy) w górę -----------------
    // `level` to wysokość po wypełnieniu dołków: każdy kafel ląduje wyżej niż kafel,
    // z którego został zalany, więc z każdego miejsca istnieje spadek do morza.
    let mut level = vec![0u64; n];
    let mut done = vec![false; n];
    let mut order = Vec::with_capacity(n);
    let mut heap = BinaryHeap::new();
    let push = |heap: &mut BinaryHeap<Reverse<u64>>, level: u64, i: usize| heap.push(Reverse((level << 32) | i as u64));
    const NEIGHBORS: [(isize, isize); 8] = [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)];
    let neighbor = |i: usize, (dx, dy): (isize, isize)| {
        let (nx, ny) = ((i % w) as isize + dx, (i / w) as isize + dy);
        (nx >= 0 && ny >= 0 && nx < w as isize && ny < h as isize).then(|| ny as usize * w + nx as usize)
    };

    for i in 0..n {
        let (x, y) = (i % w, i / w);
        if r.terrain[i] == Terrain::Ocean {
            done[i] = true;
            // Do kolejki trafia tylko linia brzegowa – głęboki ocean nic nie wnosi.
            if NEIGHBORS.iter().any(|&d| neighbor(i, d).is_some_and(|j| r.terrain[j] != Terrain::Ocean)) {
                push(&mut heap, 0, i);
            }
        } else if x == 0 || y == 0 || x == w - 1 || y == h - 1 {
            done[i] = true; // ląd przy krawędzi odpływa poza mapę
            level[i] = height(i);
            push(&mut heap, level[i], i);
        }
    }
    while let Some(Reverse(key)) = heap.pop() {
        let (lvl, i) = (key >> 32, (key & 0xFFFF_FFFF) as usize);
        if r.terrain[i] != Terrain::Ocean {
            order.push(i);
        }
        for d in NEIGHBORS {
            if let Some(j) = neighbor(i, d)
                && !done[j]
            {
                done[j] = true;
                let step = flat_cost.get(j % w, j / w) as u64;
                level[j] = height(j).max(lvl + step);
                push(&mut heap, level[j], j);
            }
        }
    }

    // --- Kierunek spływu: najbardziej stromy spadek (D8) po wypełnionej powierzchni ----
    let mut receiver = vec![NONE; n];
    for &i in &order {
        let mut best = (0.0f32, NONE);
        for (k, d) in NEIGHBORS.into_iter().enumerate() {
            if let Some(j) = neighbor(i, d)
                && level[j] < level[i]
            {
                let slope = (level[i] - level[j]) as f32 / if k < 4 { 1.0 } else { 1.414 };
                if slope > best.0 {
                    best = (slope, j as u32);
                }
            }
        }
        receiver[i] = best.1;
    }

    // --- Akumulacja przepływu: ile kafli spływa przez dany kafel ----------------------
    // `order` jest posortowany rosnąco po `level`, a odbiorca ma zawsze niższy poziom,
    // więc przejście od końca liczy dopływy przed kaflem, do którego spływają.
    let mut acc = vec![1u32; n];
    for &i in order.iter().rev() {
        let to = receiver[i];
        if to != NONE && r.terrain[to as usize] != Terrain::Ocean {
            acc[to as usize] += acc[i];
        }
    }

    // --- Źródła: wysokie kafle górskie, rozstawione co najmniej `spacing` od siebie --
    let src_kind = if r.terrain.contains(&Terrain::Mountains) { Terrain::Mountains } else { Terrain::Highlands };
    let mut sources: Vec<(f32, usize)> =
        (0..n).filter(|&i| r.terrain[i] == src_kind).map(|i| (r.elevation[i] + rng.f32() * 0.15, i)).collect();
    sources.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    let spacing2 = (cs * 0.35).powi(2);
    let mut picked: Vec<usize> = Vec::new();
    for &(_, i) in &sources {
        if picked.len() >= p.river_count as usize {
            break;
        }
        let far = picked.iter().all(|&j| {
            let (dx, dy) = ((i % w) as f32 - (j % w) as f32, (i / w) as f32 - (j / w) as f32);
            dx * dx + dy * dy >= spacing2
        });
        if far {
            picked.push(i);
        }
    }

    // --- Śledzenie rzek w dół drzewa odpływu -------------------------------------------
    let min_len = (cs * 0.25) as usize;
    let (wide, widest) = (cs * cs * 0.08, cs * cs * 0.35);
    let mut is_river = vec![false; n];
    let mut rivers = 0;
    for &src in &picked {
        let mut path = Vec::new();
        let mut cur = src;
        while r.terrain[cur] != Terrain::Ocean && !is_river[cur] && path.len() < n {
            path.push(cur);
            match receiver[cur] {
                NONE => break,
                next => cur = next as usize,
            }
        }
        if path.len() < min_len {
            continue;
        }
        rivers += 1;
        // Odległość do ujścia: dopływ dziedziczy odległość rzeki, do której wpada
        // (ścieżka kończy się na kaflu istniejącej rzeki, który nie należy do ścieżki).
        let base = if is_river[cur] && path.last() != Some(&cur) { r.river_flow[cur] as usize } else { 0 };
        let len = path.len();
        for (k, &i) in path.iter().enumerate() {
            let width = if acc[i] as f32 > widest {
                3
            } else if acc[i] as f32 > wide {
                2
            } else {
                1
            };
            let flow = (base + len - k).min(u16::MAX as usize) as u16;
            paint(r, &mut is_river, i, width, flow);
            // Krok po przekątnej: dopełnij narożnik, żeby rzeka była spójna w sąsiedztwie 4.
            if let Some(&j) = path.get(k + 1)
                && i % w != j % w
                && i / w != j / w
            {
                paint(r, &mut is_river, (i / w) * w + j % w, 1, flow);
            }
        }
    }
    rivers
}

fn paint(r: &mut Relief, is_river: &mut [bool], i: usize, width: u8, flow: u16) {
    let (w, h) = (r.w, r.h);
    let (x, y) = (i % w, i / w);
    let mut set = |x: usize, y: usize| {
        let j = y * w + x;
        if r.terrain[j].is_land() {
            r.terrain[j] = Terrain::River;
            r.shade[j] = 0;
        }
        if r.terrain[j] == Terrain::River {
            is_river[j] = true;
            if r.river_flow[j] == 0 {
                r.river_flow[j] = flow;
            }
        }
    };
    set(x, y);
    match width {
        2 => {
            if x + 1 < w {
                set(x + 1, y);
            }
            if y + 1 < h {
                set(x, y + 1);
            }
            if x + 1 < w && y + 1 < h {
                set(x + 1, y + 1);
            }
        }
        3 => {
            if x > 0 {
                set(x - 1, y);
            }
            if x + 1 < w {
                set(x + 1, y);
            }
            if y > 0 {
                set(x, y - 1);
            }
            if y + 1 < h {
                set(x, y + 1);
            }
        }
        _ => {}
    }
}
