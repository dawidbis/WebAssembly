//! Biomy kontynentów. Każdy kontynent dostaje biom główny, a z szansą `biome_mix_chance`
//! także drugi – tylko z pary dozwolonej w `biome_pairs`. Granica między nimi to pofalowana szumem linia w poprzek kontynentu,
//! a strefa przejścia (`biome_transition` kafli) miesza oba biomy płynnie (smoothstep),
//! z przeplatającymi się płatami zamiast prostego gradientu.
//!
//! Etap ma własny RNG i nie zmienia terenu, więc strojenie biomów nie rusza kształtu mapy.
//! Tylko operacje deterministyczne w IEEE 754 (bez sin/cos/exp), jak w reszcie generatora.

use std::collections::VecDeque;

use fastnoise_lite::FractalType;

use crate::{
    layout::Layout,
    relief::Relief,
    util::{components, fractal, quantile_above, smoothstep, CoarseField, Rng},
    Biome, MapGenParams, Terrain,
};

/// Stała mieszana z seedem – osobny strumień losowości tylko dla biomów.
const BIOME_SALT: u64 = 0x5851_F42D_4C95_7F2D;

/// „Idealny” chłód biomu: 0 = biegun ciepła, 1 = biegun zimna. Od najcieplejszego:
/// pustynia, dżungla, step (sawanna), umiarkowany, zimny.
const IDEAL_COLDNESS: [f32; 5] = [0.62, 0.0, 1.0, 0.18, 0.36];
/// Jak szybko spada dopasowanie biomu z odległością od jego idealnego chłodu.
const COLDNESS_TOLERANCE: f32 = 0.35;

pub struct Biomes {
    pub dominant: Vec<u8>,
    pub other: Vec<u8>,
    pub mix: Vec<u8>,
    pub mixed_continents: u32,
}

impl Biomes {
    /// Udział biomów wśród kafli lądu (według biomu dominującego).
    pub fn shares(&self, terrain: &[Terrain]) -> Vec<f32> {
        let mut count = [0u32; 5];
        let mut land = 0u32;
        for (i, t) in terrain.iter().enumerate() {
            if t.is_land() {
                count[self.dominant[i] as usize] += 1;
                land += 1;
            }
        }
        count.iter().map(|&c| c as f32 / land.max(1) as f32).collect()
    }
}

/// Plan biomów jednego kontynentu.
struct Plan {
    primary: Biome,
    /// `Some` = kontynent z dwoma biomami.
    secondary: Option<Biome>,
    /// Udział drugiego biomu w lądzie kontynentu.
    share: f32,
    /// Kierunek, w którym rośnie udział drugiego biomu (wektor jednostkowy).
    dir: (f32, f32),
    center: (f32, f32),
}

pub fn build(p: &MapGenParams, l: &Layout, r: &Relief) -> Biomes {
    let (w, h) = (r.w, r.h);
    let n = w * h;
    let uniform = |b: Biome| Biomes {
        dominant: vec![b as u8; n],
        other: vec![b as u8; n],
        mix: vec![0; n],
        mixed_continents: 0,
    };
    if !p.biomes {
        return uniform(Biome::Temperate);
    }

    let mut rng = Rng::new(p.seed as u64 ^ BIOME_SALT);
    let cs = l.chunk_size();
    // Stała liczba losowań na początku: zmiana jednego suwaka nie przetasowuje reszty.
    let border_noise = fractal(rng.noise_seed(), FractalType::FBm, 1.0 / cs, 3);
    let patch_noise_seed = rng.noise_seed();
    // Bieguny klimatu jak najdalej od siebie: zimna przy górnej albo dolnej krawędzi,
    // ciepła naprzeciwko (odbicie przez środek mapy).
    let cold_pole = (rng.f32() * w as f32, if rng.f32() < 0.5 { 0.0 } else { h as f32 });
    let hot_pole = (w as f32 - cold_pole.0, h as f32 - cold_pole.1);

    // --- 1. Który kafel lądu należy do którego kontynentu -------------------------------
    let continents = l.owner.iter().copied().max().unwrap_or(-1) + 1;
    if continents <= 0 {
        return uniform(Biome::Temperate);
    }
    let k = continents as usize;
    let chunk_owner = nearest_owner(l);
    let land: Vec<bool> = r.terrain.iter().map(|t| t.is_land()).collect();
    let (lab, sizes) = components(w, h, |i| land[i]);
    // Cały ląd (spójny obszar) należy do jednego kontynentu – głosowanie chunków.
    // Dzięki temu w obrębie jednego lądu nigdy nie ma twardego szwu między kontynentami.
    let mut votes = vec![0u32; sizes.len() * k];
    for i in 0..n {
        if land[i] {
            votes[lab[i] as usize * k + chunk_owner[l.chunk_of(i % w, i / w)]] += 1;
        }
    }
    let comp_owner: Vec<usize> = (0..sizes.len())
        .map(|c| {
            let v = &votes[c * k..(c + 1) * k];
            (0..k).fold(0, |best, j| if v[j] > v[best] { j } else { best })
        })
        .collect();
    let owner_of = |i: usize| comp_owner[lab[i] as usize];

    // --- 2. Środki ciężkości kontynentów ------------------------------------------------
    let (mut sx, mut sy, mut cnt) = (vec![0u64; k], vec![0u64; k], vec![0u64; k]);
    for i in 0..n {
        if land[i] {
            let o = owner_of(i);
            sx[o] += (i % w) as u64;
            sy[o] += (i / w) as u64;
            cnt[o] += 1;
        }
    }

    // --- 3. Wybór biomów ---------------------------------------------------------------
    let weights = p.biome_weights();
    let lat_pull = p.biome_latitude;
    // Chłód kontynentu: 0 przy biegunie ciepła, 1 przy biegunie zimna – rozciągnięty na pełny
    // zakres, więc najzimniejszy kontynent leży „na biegunie zimna”, a najcieplejszy – ciepła.
    let raw_cold: Vec<f32> = (0..k)
        .map(|o| {
            let c = cnt[o].max(1) as f32;
            let center = (sx[o] as f32 / c, sy[o] as f32 / c);
            let dist = |q: (f32, f32)| ((center.0 - q.0).powi(2) + (center.1 - q.1).powi(2)).sqrt();
            let (dc, dh) = (dist(cold_pole), dist(hot_pole));
            if dc + dh > 0.0 { dh / (dc + dh) } else { 0.5 }
        })
        .collect();
    let present = (0..k).filter(|&o| cnt[o] > 0);
    let lo = present.clone().map(|o| raw_cold[o]).fold(f32::MAX, f32::min);
    let hi = present.map(|o| raw_cold[o]).fold(f32::MIN, f32::max);
    let cold_of = |o: usize| if hi - lo > 1e-3 { (raw_cold[o] - lo) / (hi - lo) } else { raw_cold[o] };
    let plans: Vec<Plan> = (0..k)
        .map(|o| {
            let draws: [f32; 6] = std::array::from_fn(|_| rng.f32());
            let c = cnt[o].max(1) as f32;
            let center = (sx[o] as f32 / c, sy[o] as f32 / c);
            let coldness = cold_of(o);
            let fit = |b: Biome| {
                let d = (coldness - IDEAL_COLDNESS[b as usize]).abs();
                0.02 + 0.98 * (1.0 - smoothstep(0.0, COLDNESS_TOLERANCE, d))
            };
            // Biegun zimna i ciepła jak najdalej od siebie: zimny biom tylko po zimnej połowie,
            // ciepłe (pustynia, dżungla, step) tylko po ciepłej; umiarkowany wszędzie.
            let allowed = |b: Biome| {
                lat_pull <= 0.0
                    || match b {
                        Biome::Cold => coldness >= 0.5,
                        Biome::Desert | Biome::Humid | Biome::Steppe => coldness <= 0.5,
                        Biome::Temperate => true,
                    }
            };
            let score = |b: Biome| {
                if allowed(b) { weights[b as usize] * ((1.0 - lat_pull) + lat_pull * fit(b)) } else { 0.0 }
            };
            let primary = pick(&Biome::ALL.map(|b| (b, score(b))), draws[0]).unwrap_or(Biome::Temperate);

            // Drugi biom tylko spośród par dozwolonych w `biome_pairs` (zabronione mają wagę 0).
            let secondary = (draws[1] < p.biome_mix_chance)
                .then(|| {
                    let options = Biome::ALL
                        .map(|b| (b, if b != primary && p.biomes_can_mix(primary, b) { score(b) } else { 0.0 }));
                    pick(&options, draws[2])
                })
                .flatten();
            let share = (p.biome_secondary_share * (0.75 + 0.5 * draws[3])).clamp(0.05, 0.5);

            // Kierunek granicy: losowy, a przy wpływie biegunów ciągnięty tak, żeby chłodniejszy
            // z dwóch biomów leżał bliżej bieguna zimna, a cieplejszy – bieguna ciepła.
            let toward_cold = {
                let (vx, vy) = (cold_pole.0 - hot_pole.0, cold_pole.1 - hot_pole.1);
                let len = (vx * vx + vy * vy).sqrt().max(1e-3);
                (vx / len, vy / len)
            };
            let sign = match secondary {
                Some(b) if IDEAL_COLDNESS[b as usize] < IDEAL_COLDNESS[primary as usize] => -1.0,
                _ => 1.0,
            };
            let (rx, ry) = (draws[4] * 2.0 - 1.0, draws[5] * 2.0 - 1.0);
            let (dx, dy) = (
                rx * (1.0 - lat_pull) + sign * toward_cold.0 * lat_pull,
                ry * (1.0 - lat_pull) + sign * toward_cold.1 * lat_pull,
            );
            let len = (dx * dx + dy * dy).sqrt();
            let dir = if len > 1e-3 { (dx / len, dy / len) } else { (sign * toward_cold.0, sign * toward_cold.1) };
            Plan { primary, secondary, share, dir, center }
        })
        .collect();
    let mixed_continents = (0..k).filter(|&o| plans[o].secondary.is_some() && cnt[o] > 0).count() as u32;

    // --- 4. Mieszanie biomów -----------------------------------------------------------
    let mut dominant = vec![0u8; n];
    let mut other = vec![0u8; n];
    let mut mix = vec![0u8; n];
    let tw = p.biome_transition as f32;
    let rough = p.biome_roughness;

    if plans.iter().any(|pl| pl.secondary.is_some()) {
        // Duża skala: pofalowanie całej granicy. Mała (zależna od szerokości przejścia):
        // płaty jednego biomu wchodzące w drugi – przejście wygląda naturalnie, nie jak gradient.
        let patch_noise = fractal(patch_noise_seed, FractalType::FBm, 1.0 / (tw * 0.6).max(3.0), 3);
        let (border, patch) = CoarseField::pair(w, h, |fx, fy| {
            (border_noise.get_noise_2d(fx, fy), patch_noise.get_noise_2d(fx, fy))
        });
        let border_amp = cs * 0.9 * rough;
        let patch_amp = tw * (0.15 + 0.5 * rough);

        // Pozycja kafla wzdłuż kierunku granicy (w kaflach), z szumem.
        let mut pos = vec![0.0f32; n];
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                if !land[i] {
                    continue;
                }
                let pl = &plans[owner_of(i)];
                if pl.secondary.is_none() {
                    continue;
                }
                pos[i] = (x as f32 - pl.center.0) * pl.dir.0
                    + (y as f32 - pl.center.1) * pl.dir.1
                    + border.get(x, y) * border_amp
                    + patch.get(x, y) * patch_amp;
            }
        }
        // Próg dobrany percentylem: drugi biom zajmuje zadany udział lądu kontynentu.
        let mut per_owner: Vec<Vec<f32>> = vec![Vec::new(); k];
        for i in 0..n {
            if land[i] && plans[owner_of(i)].secondary.is_some() {
                per_owner[owner_of(i)].push(pos[i]);
            }
        }
        let split: Vec<f32> = (0..k)
            .map(|o| if per_owner[o].is_empty() { 0.0 } else { quantile_above(per_owner[o].iter().copied(), plans[o].share) })
            .collect();

        for i in 0..n {
            if !land[i] {
                continue;
            }
            let o = owner_of(i);
            let pl = &plans[o];
            let Some(sec) = pl.secondary else {
                dominant[i] = pl.primary as u8;
                other[i] = pl.primary as u8;
                continue;
            };
            let t = smoothstep(split[o] - tw * 0.5, split[o] + tw * 0.5, pos[i]);
            let (dom, oth) = if t < 0.5 { (pl.primary, sec) } else { (sec, pl.primary) };
            dominant[i] = dom as u8;
            other[i] = oth as u8;
            mix[i] = (t.min(1.0 - t) * 256.0 + 0.5).min(128.0) as u8;
        }
    } else {
        for i in 0..n {
            if land[i] {
                let b = plans[owner_of(i)].primary as u8;
                dominant[i] = b;
                other[i] = b;
            }
        }
    }

    // --- 5. Woda przejmuje biom najbliższego lądu (BFS wielu źródeł) ---------------------
    // Rzeki i jeziora wycinane później przez hydrologię leżą na dawnym lądzie, więc już mają biom.
    let mut seen = land.clone();
    let mut queue: VecDeque<usize> = (0..n).filter(|&i| land[i]).collect();
    while let Some(i) = queue.pop_front() {
        let (x, y) = (i % w, i / w);
        let mut visit = |j: usize| {
            if !seen[j] {
                seen[j] = true;
                dominant[j] = dominant[i];
                other[j] = other[i];
                mix[j] = mix[i];
                queue.push_back(j);
            }
        };
        if x > 0 { visit(i - 1); }
        if x + 1 < w { visit(i + 1); }
        if y > 0 { visit(i - w); }
        if y + 1 < h { visit(i + w); }
    }

    Biomes { dominant, other, mix, mixed_continents }
}

/// Losowanie ważone. `u` z przedziału [0, 1). `None`, gdy wszystkie wagi są zerowe.
fn pick(options: &[(Biome, f32)], u: f32) -> Option<Biome> {
    let total: f32 = options.iter().map(|o| o.1).sum();
    if total <= 0.0 {
        return None;
    }
    let mut acc = u * total;
    for &(b, s) in options {
        if s > 0.0 && acc < s {
            return Some(b);
        }
        acc -= s;
    }
    options.iter().rev().find(|o| o.1 > 0.0).map(|o| o.0)
}

/// Dla każdego chunka: kontynent, do którego należy, a dla wodnego – najbliższy lądowy.
fn nearest_owner(l: &Layout) -> Vec<usize> {
    let total = l.cols * l.rows;
    (0..total)
        .map(|c| {
            if l.owner[c] >= 0 {
                return l.owner[c] as usize;
            }
            let (cx, cy) = ((c % l.cols) as i64, (c / l.cols) as i64);
            let mut best = (i64::MAX, 0usize);
            for o in 0..total {
                if l.owner[o] >= 0 {
                    let (ox, oy) = ((o % l.cols) as i64, (o / l.cols) as i64);
                    let d = (ox - cx).pow(2) + (oy - cy).pow(2);
                    if d < best.0 {
                        best = (d, l.owner[o] as usize);
                    }
                }
            }
            best.1
        })
        .collect()
}
