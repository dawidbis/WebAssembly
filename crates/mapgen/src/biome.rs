//! Biomy kontynentów – dwa poziomy:
//!
//! 1. **Typ klimatu** (tropikalny, suchy, umiarkowany, kontynentalny, polarny) – na kontynent.
//!    Każdy kontynent dostaje typ główny, a z szansą `biome_mix_chance` także drugi – tylko z pary
//!    dozwolonej w `biome_pairs`. Granica między nimi to pofalowana szumem linia w poprzek
//!    kontynentu, a strefa przejścia (`biome_transition` kafli) miesza oba typy płynnie
//!    (smoothstep), z przeplatającymi się płatami zamiast prostego gradientu.
//! 2. **Rodzaj biomu** wewnątrz typu – z dwóch pól na kaflu: chłodu (położenie między biegunem
//!    ciepła a zimna) i suchości (odległość od morza), oba w kaflach i pofalowane szumem.
//!    Progi są dobierane percentylem w obszarze typu na kontynencie (udziały `biome_*_share`),
//!    więc każdy taki obszar ma wszystkie rodzaje swojego typu. Przejście między rodzajami ma
//!    szerokość `biome_kind_transition` kafli.
//!
//! Kafel zapisuje dwie **warstwy** (typ główny i drugi typ kontynentu), każdą jako typ i dwa
//! płynne parametry rodzaju (σ1, σ2 w 0..255), oraz udział drugiej warstwy `mix`. Z tego
//! [`kind_weights`] liczy wagi wszystkich rodzajów – dokładnie i bez szwów, także tam, gdzie
//! granica typów spotyka granice rodzajów (do sześciu rodzajów naraz). `dominant` = rodzaj
//! o największej wadze (to on liczy się w rozgrywce).
//!
//! Etap ma własny RNG i nie zmienia terenu, więc strojenie biomów nie rusza kształtu mapy.
//! Tylko operacje deterministyczne w IEEE 754 (bez sin/cos/exp), jak w reszcie generatora.

use std::collections::VecDeque;

use fastnoise_lite::FractalType;

use crate::{
    layout::Layout,
    relief::Relief,
    util::{components, distance_field, fractal, quantile_above, smoothstep, CoarseField, Rng},
    Biome, BiomeType, MapGenParams, Terrain,
};

/// Stała mieszana z seedem – osobny strumień losowości tylko dla biomów.
const BIOME_SALT: u64 = 0x5851_F42D_4C95_7F2D;

/// „Idealny” chłód typu: 0 = biegun ciepła, 1 = biegun zimna (kolejność `BiomeType::ALL`).
const IDEAL_COLDNESS: [f32; 5] = [0.0, 0.22, 0.45, 0.7, 1.0];
/// Jak szybko spada dopasowanie biomu z odległością od jego idealnego chłodu.
const COLDNESS_TOLERANCE: f32 = 0.35;

pub struct Biomes {
    pub dominant: Vec<u8>,
    /// 6 bajtów na kafel: [typ, σ1, σ2] warstwy głównej i [typ, σ1, σ2] drugiej.
    pub layers: Vec<u8>,
    /// Udział drugiej warstwy 0..255.
    pub mix: Vec<u8>,
    pub mixed_continents: u32,
}

/// Wagi rodzajów (suma 1) w kaflu: `layers` – 6 bajtów kafla, `mix` – udział drugiej warstwy.
/// Tak samo liczy je renderer (`kindWeights` w `web/src/app/render/terrain.ts`).
pub fn kind_weights(layers: &[u8], mix: u8) -> [f32; Biome::COUNT] {
    let mut w = [0f32; Biome::COUNT];
    let t = mix as f32 / 255.0;
    add_layer(&mut w, &layers[0..3], 1.0 - t);
    add_layer(&mut w, &layers[3..6], t);
    w
}

/// Rodzaj o największej wadze (przy remisie – niższy numer).
pub fn dominant_kind(w: &[f32; Biome::COUNT]) -> u8 {
    (0..Biome::COUNT).fold(0, |best, b| if w[b] > w[best] { b } else { best }) as u8
}

/// Dodaje wagi rodzajów jednej warstwy. σ1, σ2 – płynne udziały (0..1) po drugiej stronie progów
/// rodzajów, znaczenie zależy od typu (patrz `Th::sigmas`).
fn add_layer(w: &mut [f32; Biome::COUNT], l: &[u8], share: f32) {
    if share <= 0.0 {
        return;
    }
    let (a, b) = (l[1] as f32 / 255.0, l[2] as f32 / 255.0);
    let mut put = |k: Biome, v: f32| w[k as usize] += share * v;
    match BiomeType::ALL[l[0] as usize] {
        BiomeType::Tropical => {
            put(Biome::Rainforest, 1.0 - a);
            put(Biome::Savanna, a);
        }
        BiomeType::Dry => {
            put(Biome::Steppe, 1.0 - a);
            put(Biome::Desert, a);
        }
        BiomeType::Temperate => {
            put(Biome::Oceanic, a);
            put(Biome::Mediterranean, (1.0 - a) * b);
            put(Biome::Subtropical, (1.0 - a) * (1.0 - b));
        }
        BiomeType::Continental => {
            put(Biome::Boreal, a);
            put(Biome::WarmSummer, (b - a).max(0.0));
            put(Biome::HotSummer, 1.0 - b.max(a));
        }
        BiomeType::Polar => {
            put(Biome::Tundra, 1.0 - a);
            put(Biome::IceSheet, a);
        }
    }
}

impl Biomes {
    /// Udział rodzajów biomów wśród kafli lądu (według biomu dominującego).
    pub fn shares(&self, terrain: &[Terrain]) -> Vec<f32> {
        let mut count = [0u32; Biome::COUNT];
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

/// Plan typów klimatu jednego kontynentu.
struct Plan {
    primary: BiomeType,
    /// `Some` = kontynent z dwoma typami.
    secondary: Option<BiomeType>,
    /// Udział drugiego typu w lądzie kontynentu.
    share: f32,
    /// Kierunek, w którym rośnie udział drugiego typu (wektor jednostkowy).
    dir: (f32, f32),
    center: (f32, f32),
}

pub fn build(p: &MapGenParams, l: &Layout, r: &Relief) -> Biomes {
    let (w, h) = (r.w, r.h);
    let n = w * h;
    // Cały ląd w umiarkowanym oceanicznym (σ1 = 255 → sam oceaniczny).
    let uniform = || Biomes {
        dominant: vec![Biome::Oceanic as u8; n],
        layers: [BiomeType::Temperate as u8, 255, 0].repeat(2 * n),
        mix: vec![0; n],
        mixed_continents: 0,
    };
    if !p.biomes {
        return uniform();
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
    // Pofalowanie granic rodzajów: osobny szum dla chłodu i suchości.
    let kind_noise = (
        fractal(rng.noise_seed(), FractalType::FBm, 1.5 / cs, 3),
        fractal(rng.noise_seed(), FractalType::FBm, 1.5 / cs, 3),
    );

    // --- 1. Który kafel lądu należy do którego kontynentu -------------------------------
    let continents = l.owner.iter().copied().max().unwrap_or(-1) + 1;
    if continents <= 0 {
        return uniform();
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

    // --- 3. Wybór typów klimatu ---------------------------------------------------------
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
            let fit = |b: BiomeType| {
                let d = (coldness - IDEAL_COLDNESS[b as usize]).abs();
                0.02 + 0.98 * (1.0 - smoothstep(0.0, COLDNESS_TOLERANCE, d))
            };
            // Biegun zimna i ciepła jak najdalej od siebie: polarny tylko po zimnej połowie,
            // tropikalny i suchy tylko po ciepłej; umiarkowany i kontynentalny wszędzie.
            let allowed = |b: BiomeType| {
                lat_pull <= 0.0
                    || match b {
                        BiomeType::Polar => coldness >= 0.5,
                        BiomeType::Tropical | BiomeType::Dry => coldness <= 0.5,
                        BiomeType::Temperate | BiomeType::Continental => true,
                    }
            };
            let score = |b: BiomeType| {
                if allowed(b) { weights[b as usize] * ((1.0 - lat_pull) + lat_pull * fit(b)) } else { 0.0 }
            };
            let primary = pick(&BiomeType::ALL.map(|b| (b, score(b))), draws[0]).unwrap_or(BiomeType::Temperate);

            // Drugi typ tylko spośród par dozwolonych w `biome_pairs` (zabronione mają wagę 0).
            let secondary = (draws[1] < p.biome_mix_chance)
                .then(|| {
                    let options = BiomeType::ALL
                        .map(|b| (b, if b != primary && p.biomes_can_mix(primary, b) { score(b) } else { 0.0 }));
                    pick(&options, draws[2])
                })
                .flatten();
            let share = (p.biome_secondary_share * (0.75 + 0.5 * draws[3])).clamp(0.05, 0.5);

            // Kierunek granicy: losowy, a przy wpływie biegunów ciągnięty tak, żeby chłodniejszy
            // z dwóch typów leżał bliżej bieguna zimna, a cieplejszy – bieguna ciepła.
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

    // --- 4. Udział drugiego typu w kaflu ------------------------------------------------
    // 0 = sam typ główny, 1 = sam drugi typ; w strefie przejścia płynnie.
    let mut second = vec![0f32; n];
    let tw = p.biome_transition as f32;
    let rough = p.biome_roughness;

    if plans.iter().any(|pl| pl.secondary.is_some()) {
        // Duża skala: pofalowanie całej granicy. Mała (zależna od szerokości przejścia):
        // płaty jednego typu wchodzące w drugi – przejście wygląda naturalnie, nie jak gradient.
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
        // Próg dobrany percentylem: drugi typ zajmuje zadany udział lądu kontynentu.
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
            if land[i] && plans[owner_of(i)].secondary.is_some() {
                let o = owner_of(i);
                second[i] = smoothstep(split[o] - tw * 0.5, split[o] + tw * 0.5, pos[i]);
            }
        }
    }
    let type_at = |i: usize| {
        let pl = &plans[owner_of(i)];
        match pl.secondary {
            Some(s) if second[i] >= 0.5 => s,
            _ => pl.primary,
        }
    };

    // --- 5. Pola chłodu i suchości (w kaflach) -------------------------------------------
    // Chłód: o ile kafel jest bliżej bieguna zimna niż ciepła. Suchość: odległość od morza.
    let coast = distance_field(w, h, |i| !land[i]);
    let (noise_c, noise_d) = CoarseField::pair(w, h, |fx, fy| {
        (kind_noise.0.get_noise_2d(fx, fy), kind_noise.1.get_noise_2d(fx, fy))
    });
    let amp = cs * 0.6 * p.biome_kind_roughness;
    let mut cold = vec![0f32; n];
    let mut dry = vec![0f32; n];
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if !land[i] {
                continue;
            }
            let dist = |q: (f32, f32)| ((x as f32 - q.0).powi(2) + (y as f32 - q.1).powi(2)).sqrt();
            cold[i] = (dist(hot_pole) - dist(cold_pole)) * 0.5 + noise_c.get(x, y) * amp;
            dry[i] = coast[i] + noise_d.get(x, y) * amp;
        }
    }

    // --- 6. Progi rodzajów: percentyl w obszarze typu na kontynencie ---------------------
    let mut region: Vec<Vec<u32>> = vec![Vec::new(); k * 5];
    let mut whole: Vec<Vec<u32>> = vec![Vec::new(); k];
    for i in 0..n {
        if land[i] {
            region[owner_of(i) * 5 + type_at(i) as usize].push(i as u32);
            whole[owner_of(i)].push(i as u32);
        }
    }
    let mut thresholds = vec![Th::NONE; k * 5];
    for o in 0..k {
        let pl = &plans[o];
        for t in [Some(pl.primary), pl.secondary].into_iter().flatten() {
            // Typ tylko w strefie przejścia (bez własnych kafli) – progi z całego kontynentu.
            let tiles = if region[o * 5 + t as usize].is_empty() { &whole[o] } else { &region[o * 5 + t as usize] };
            thresholds[o * 5 + t as usize] = Th::build(p, t, tiles, &cold, &dry);
        }
    }

    // --- 7. Warstwy typów z parametrami rodzajów -----------------------------------------
    let mut dominant = vec![0u8; n];
    let mut layers = vec![0u8; 6 * n];
    let mut mix = vec![0u8; n];
    let half = p.biome_kind_transition as f32 * 0.5;
    for i in 0..n {
        if !land[i] {
            continue;
        }
        let o = owner_of(i);
        let pl = &plans[o];
        let sec = pl.secondary.unwrap_or(pl.primary);
        for (slot, t) in [pl.primary, sec].into_iter().enumerate() {
            let (s1, s2) = thresholds[o * 5 + t as usize].sigmas(t, cold[i], dry[i], half);
            layers[6 * i + 3 * slot..6 * i + 3 * slot + 3].copy_from_slice(&[t as u8, s1, s2]);
        }
        mix[i] = (second[i] * 255.0 + 0.5) as u8;
        dominant[i] = dominant_kind(&kind_weights(&layers[6 * i..6 * i + 6], mix[i]));
    }

    // --- 8. Woda przejmuje biom najbliższego lądu (BFS wielu źródeł) ---------------------
    // Rzeki i jeziora wycinane później przez hydrologię leżą na dawnym lądzie, więc już mają biom.
    let mut seen = land.clone();
    let mut queue: VecDeque<usize> = (0..n).filter(|&i| land[i]).collect();
    while let Some(i) = queue.pop_front() {
        let (x, y) = (i % w, i / w);
        let mut visit = |j: usize| {
            if !seen[j] {
                seen[j] = true;
                dominant[j] = dominant[i];
                layers.copy_within(6 * i..6 * i + 6, 6 * j);
                mix[j] = mix[i];
                queue.push_back(j);
            }
        };
        if x > 0 { visit(i - 1); }
        if x + 1 < w { visit(i + 1); }
        if y > 0 { visit(i - w); }
        if y + 1 < h { visit(i + w); }
    }

    Biomes { dominant, layers, mix, mixed_continents }
}

/// Losowanie ważone. `u` z przedziału [0, 1). `None`, gdy wszystkie wagi są zerowe.
fn pick(options: &[(BiomeType, f32)], u: f32) -> Option<BiomeType> {
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

/// Progi rodzajów w obszarze jednego typu na kontynencie (znaczenie `a`, `b` zależy od typu).
/// Nieskończoność = granica nie występuje.
#[derive(Clone, Copy)]
struct Th {
    a: f32,
    b: f32,
}

impl Th {
    const NONE: Th = Th { a: f32::INFINITY, b: f32::INFINITY };

    fn build(p: &MapGenParams, t: BiomeType, tiles: &[u32], cold: &[f32], dry: &[f32]) -> Th {
        let c = || tiles.iter().map(|&i| cold[i as usize]);
        let d = || tiles.iter().map(|&i| dry[i as usize]);
        match t {
            BiomeType::Tropical => Th { a: above(d(), 1.0 - p.biome_rainforest_share), b: f32::INFINITY },
            BiomeType::Dry => Th { a: above(d(), p.biome_desert_share), b: f32::INFINITY },
            BiomeType::Temperate => {
                let a = above(c(), p.biome_oceanic_share);
                // Śródziemnomorski: suchsza część tego, co nie jest oceaniczne.
                let rest = d().zip(c()).filter(|&(_, cv)| cv <= a).map(|(dv, _)| dv);
                Th { a, b: above(rest, p.biome_mediterranean_share) }
            }
            BiomeType::Continental => {
                let a = above(c(), p.biome_boreal_share);
                Th { a, b: above(c(), 1.0 - p.biome_hot_summer_share).min(a) }
            }
            BiomeType::Polar => Th { a: above(c(), p.biome_ice_share), b: f32::INFINITY },
        }
    }

    /// Płynne parametry rodzaju (0..255) w punkcie (chłód `c`, suchość `d`): udział strony
    /// „powyżej” progu, przejście o połowie szerokości `half` kafli. Tropikalny: σ1 = sawanna;
    /// suchy: σ1 = pustynia; umiarkowany: σ1 = oceaniczny, σ2 = śródziemnomorski (w reszcie);
    /// kontynentalny: σ1 = borealny, σ2 = nie-gorące lato; polarny: σ1 = lądolód.
    fn sigmas(&self, t: BiomeType, c: f32, d: f32, half: f32) -> (u8, u8) {
        let side = |th: f32, key: f32| {
            let v = if th.is_finite() {
                smoothstep(th - half, th + half, key)
            } else if th > 0.0 {
                0.0
            } else {
                1.0
            };
            (v * 255.0 + 0.5) as u8
        };
        match t {
            BiomeType::Tropical | BiomeType::Dry => (side(self.a, d), 0),
            BiomeType::Temperate => (side(self.a, c), side(self.b, d)),
            BiomeType::Continental => (side(self.a, c), side(self.b, c)),
            BiomeType::Polar => (side(self.a, c), 0),
        }
    }
}

/// Próg, powyżej którego leży `share` wartości (0 = nic, 1 = wszystko).
fn above(values: impl Iterator<Item = f32> + Clone, share: f32) -> f32 {
    if share <= 0.0 || values.clone().next().is_none() {
        f32::INFINITY
    } else if share >= 1.0 {
        f32::NEG_INFINITY
    } else {
        quantile_above(values, share)
    }
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
