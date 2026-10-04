//! Prowincje – podział administracyjny lądu.
//!
//! Każda prowincja ma podobną **wielkość** – liczbę kafli lądu. Woda (ocean, jeziora, rzeki)
//! i góry nie należą do żadnej prowincji i są nieprzechodnie, więc granice biegną rzekami.
//! Teren, biomy i lasy wpływają tylko na przebieg granic, nie na wielkość prowincji.
//!
//! Algorytm:
//! 1. Każdy spójny ląd (sąsiedztwo 4, rzeki go rozdzielają) dostaje `k ≈ powierzchnia lądu / docelowa wielkość`
//!    prowincji (z poprawką na minimalny i maksymalny rozmiar). Małe wyspy dołączają przez
//!    morze do najbliższej prowincji.
//! 2. Start: kafle lądu uporządkowane wzdłuż krzywej Hilberta i pocięte na `k` kawałków
//!    o równej liczbie kafli – środek kawałka to zalążek prowincji.
//! 3. Kilkadziesiąt rund: prowincje rosną od zalążków (Dijkstra z kolejką kubełkową, sąsiedztwo 8,
//!    każdy zalążek startuje z własnym „handicapem”), potem handicap rośnie prowincjom za dużym,
//!    maleje za małym, a zalążki przesuwają się do środka prowincji (Lloyd). W ostatnich rundach
//!    zalążki stoją i wyrównywana jest już tylko wielkość. Większość rund liczy się na siatce 2 × 2
//!    (4× szybciej), ostatnie w pełnej rozdzielczości.
//! 4. Koszt drogi przez teren robi granice naturalnymi: wspinaczka na grzbiet jest droga
//!    (granica wypada na grani), woda jest nieprzekraczalna (na siatce zgrubnej blok z rzeką jest
//!    drogi, więc już wstępny rozrost nie przeskakuje rzek), a szum robi granice nieregularne jak prawdziwe granice powiatów.
//! 5. Drobne meandry granic (przesunięcie szumem, bez przeskakiwania rzek), potem sprzątanie:
//!    każda prowincja jest spójna (sąsiedztwo 4); okruchy i za małe prowincje dołączają do sąsiadów.
//!
//! Etap ma własny RNG i nie zmienia terenu, biomów ani roślinności.

use fastnoise_lite::FractalType;
use serde::{Deserialize, Serialize};

use crate::{
    util::{components, fractal, CoarseField, Rng},
    Biome, MapGenParams, MapStats, Terrain,
};

const PROVINCE_SALT: u64 = 0x5851_F42D_4C95_7F2D;
/// Koszt kroku po płaskim terenie (koszty są całkowite – kolejka kubełkowa).
const STEP: u32 = 16;
/// Rundy wyrównywania z suwaka `province_rounds` (rundy na siatce zgrubnej): (zgrubne,
/// z przesuwaniem zalążków do środka prowincji, w pełnej rozdzielczości). 14 → (14, 5, 2).
fn rounds(p: &MapGenParams) -> (usize, usize, usize) {
    let coarse = p.province_rounds as usize;
    (coarse, (coarse * 5 + 7) / 14, if coarse >= 24 { 3 } else { 2 })
}
/// Kubełki kolejki (koszt jednego kroku musi być mniejszy).
const BUCKETS: usize = 4096;

/// Prowincja: stałe właściwości pod mechaniki i do wyświetlania (granice nie zmieniają się
/// po wygenerowaniu mapy).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct Province {
    /// Numer prowincji (taki jak w `MapData::province`), od 1.
    pub id: u16,
    /// Liczba kafli (tylko ląd – rzeki, jeziora i góry nie należą do prowincji).
    pub area: u32,
    /// Biom prowincji: rodzaj (`Biome as u8`), który dominuje na największej liczbie jej kafli.
    pub biome: u8,
    /// Środek prowincji (kafel należący do prowincji) – np. pod etykietę albo stolicę.
    pub center_x: u16,
    pub center_y: u16,
    /// Czy prowincja graniczy (sąsiedztwo 4) z oceanem.
    pub coastal: bool,
    /// Czy graniczy z rzeką.
    pub river: bool,
    /// Czy graniczy z jeziorem.
    pub lake: bool,
    /// Czy graniczy z górami.
    pub mountains: bool,
}

pub struct Provinces {
    /// Numer prowincji na kaflu, 0 = brak (woda).
    pub id: Vec<u16>,
    pub list: Vec<Province>,
}

impl Provinces {
    pub fn empty(n: usize) -> Self {
        Self { id: vec![0; n], list: Vec::new() }
    }

    /// Wpisuje prowincje i ich statystyki do mapy z fazy 1 (`generate_base`).
    pub fn apply(self, map: &mut crate::MapData) {
        self.fill_stats(&mut map.stats);
        map.province = self.id;
        map.provinces = self.list;
    }

    pub fn fill_stats(&self, s: &mut MapStats) {
        let l = &self.list;
        s.provinces = l.len() as u32;
        if l.is_empty() {
            return;
        }
        let k = l.len() as f64;
        let mean = l.iter().map(|p| p.area as f64).sum::<f64>() / k;
        let var = l.iter().map(|p| (p.area as f64 - mean).powi(2)).sum::<f64>() / k;
        s.province_area_mean = mean as f32;
        s.province_area_std = var.sqrt() as f32;
        s.province_area_min = l.iter().map(|p| p.area).min().unwrap_or(0);
        s.province_area_max = l.iter().map(|p| p.area).max().unwrap_or(0);
    }
}

/// Czy kafel może należeć do prowincji: dostępny ląd (bez gór). Woda – ocean, jeziora i rzeki –
/// nie należy do prowincji i jest nieprzechodnia.
fn owned(terrain: &[Terrain], blocked: &[bool], i: usize) -> bool {
    !blocked[i] && terrain[i].is_land()
}

/// Indeks kafla na krzywej Hilberta (siatka 4096 × 4096).
fn hilbert(mut x: u32, mut y: u32) -> u64 {
    let n: u32 = 4096;
    let mut d = 0u64;
    let mut s = n / 2;
    while s > 0 {
        let rx = ((x & s) > 0) as u32;
        let ry = ((y & s) > 0) as u32;
        d += s as u64 * s as u64 * ((3 * rx) ^ ry) as u64;
        if ry == 0 {
            if rx == 1 {
                x = n - 1 - x;
                y = n - 1 - y;
            }
            std::mem::swap(&mut x, &mut y);
        }
        s /= 2;
    }
    d
}

struct Region {
    seed: usize,
    bias: u32,
    /// Ląd (spójny obszar), do którego należy.
    mass: usize,
}

pub fn build(p: &MapGenParams, terrain: &[Terrain], shade: &[u8], biome: &[u8], blocked: &[bool]) -> Provinces {
    let (w, h) = (p.width as usize, p.height as usize);
    let n = w * h;
    if !p.provinces {
        return Provinces::empty(n);
    }
    let mut rng = Rng::new(p.seed as u64 ^ PROVINCE_SALT);
    let target = p.province_size.max(1.0);
    let (min_area, max_area) = (p.province_min_size.max(1), p.province_max_size.max(p.province_min_size.max(1)));

    // --- Koszt wejścia na kafel ------------------------------------------------------------
    // Szum w skali prowincji (pofalowane granice) i drobny (postrzępione).
    // Typowy promień prowincji.
    let radius = target.sqrt().max(4.0);
    let wobble = fractal(rng.noise_seed(), FractalType::FBm, 1.6 / radius, 3);
    let jitter = fractal(rng.noise_seed(), FractalType::FBm, 0.1, 3);
    let rough = p.province_roughness;
    let broad = CoarseField::new(w, h, |x, y| (wobble.get_noise_2d(x, y) + 1.0) * 0.5);
    let warp = (fractal(rng.noise_seed(), FractalType::FBm, 0.06, 3), fractal(rng.noise_seed(), FractalType::FBm, 0.06, 3));
    let mut cost = vec![0u16; n];
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let t = terrain[i];
            if !owned(terrain, blocked, i) {
                continue;
            }
            let terrain_mult = match t {
                Terrain::Highlands => 1.0 + 0.2 * p.province_natural_borders,
                _ => 1.0,
            };
            let b = broad.get(x, y);
            let fine = (jitter.get_noise_2d(x as f32, y as f32) + 1.0) * 0.5;
            let noise = 1.0 + rough * (8.0 * b * b * b + 5.0 * fine * fine * fine);
            cost[i] = (STEP as f32 * terrain_mult * noise).round().clamp(1.0, 400.0) as u16;
        }
    }
    // Wspinaczka (różnica wysokości między kaflami lądu) i wejście na rzekę z lądu (przejście
    // na drugi brzeg) są drogie – granica chętnie biegnie rzeką i granią.
    let slope_q = (0.6 * p.province_natural_borders * 256.0) as u32;
    let river_cross = (STEP as f32 * 45.0 * p.province_natural_borders) as u32;

    // --- Lądy i liczba prowincji na każdym ----------------------------------------------------
    let (mass, mass_area) = components(w, h, |i| owned(terrain, blocked, i));
    let full = Level::full(p, terrain, shade, &cost, &mass, slope_q, river_cross, blocked);
    drop(cost);
    let masses = mass_area.len();
    let mut count = vec![0usize; masses];
    for m in 0..masses {
        let a = mass_area[m];
        if a < min_area {
            continue; // mała wyspa – dołączy przez morze albo będzie osobną prowincją
        }
        let by_size = (a as f64 / target as f64).round() as usize;
        let by_max = a.div_ceil(max_area) as usize;
        let by_min = (a / min_area) as usize;
        count[m] = by_size.max(by_max).min(by_min).max(1);
    }

    // --- Zalążki: cięcie krzywej Hilberta na kawałki o równej liczbie kafli --------------------
    let mut by_mass: Vec<Vec<usize>> = vec![Vec::new(); masses];
    for i in 0..n {
        if mass[i] != u32::MAX && count[mass[i] as usize] > 0 {
            by_mass[mass[i] as usize].push(i);
        }
    }
    let mut regions: Vec<Region> = Vec::new();
    for m in 0..masses {
        let k = count[m];
        if k == 0 {
            continue;
        }
        let tiles = &mut by_mass[m];
        tiles.sort_by_cached_key(|&i| hilbert((i % w) as u32, (i / w) as u32));
        let total = tiles.len() as f64;
        let mut acc = 0f64;
        let mut next = 0usize;
        for &i in tiles.iter() {
            acc += 1.0;
            // Zalążek w połowie kawałka: przy (next + 0.5) / k kafli lądu.
            while next < k && acc >= total * (next as f64 + 0.5) / k as f64 {
                regions.push(Region { seed: i, bias: 0, mass: m });
                next += 1;
            }
        }
        while next < k {
            regions.push(Region { seed: *tiles.last().unwrap(), bias: 0, mass: m });
            next += 1;
        }
    }
    drop(by_mass);

    let goal = Goal {
        area: regions.iter().map(|r| mass_area[r.mass] as f64 / count[r.mass] as f64).collect(),
        // Typowy promień prowincji w jednostkach kosztu – skala handicapu.
        scale: regions.iter().map(|r| (mass_area[r.mass] as f64 / count[r.mass] as f64).sqrt() * STEP as f64).collect(),
        min_area,
        max_area,
        masses,
    };
    let mut owner = vec![u32::MAX; n];
    let mut queue = BucketQueue::new();

    // Większość rund na siatce 2 × 2 (4× mniej kafli), ostatnie w pełnej rozdzielczości –
    // tam granice dokładnie siadają na rzekach.
    let coarse = full.coarsen();
    for r in regions.iter_mut() {
        r.seed = coarse.w * (r.seed / w / 2) + (r.seed % w) / 2;
    }
    let mut coarse_owner = vec![u32::MAX; coarse.w * coarse.h];
    let (coarse_rounds, move_rounds, fine_rounds) = rounds(p);
    balance(&coarse, &mut regions, coarse_rounds, move_rounds, &goal, &mut rng, &mut coarse_owner, &mut queue);
    for r in regions.iter_mut() {
        r.seed = full.refine(&coarse, r.seed, r.mass);
    }
    balance(&full, &mut regions, fine_rounds, 0, &goal, &mut rng, &mut owner, &mut queue);
    grow(&full.grid, &regions, &mut owner, &mut queue);

    // --- Sprzątanie -------------------------------------------------------------------------
    warp_borders(w, h, terrain, &warp, 4.0 * rough, &mut owner);
    make_contiguous(w, h, &regions, &mut owner);
    merge_small(w, h, min_area, &mut owner, regions.len());
    attach_islands(w, h, terrain, blocked, &mass, &count, &mut owner, regions.len(), min_area);

    finish(w, h, terrain, biome, &owner)
}

/// Siatka, na której rosną prowincje: pełna albo zgrubna (bloki 2 × 2).
struct Level {
    w: usize,
    h: usize,
    grid: CostGrid,
    /// Liczba kafli prowincji w kaflu siatki (w zgrubnej – suma bloku).
    area: Vec<u8>,
    /// Ląd kafla (`u32::MAX` = brak).
    mass: Vec<u32>,
}

impl Level {
    #[allow(clippy::too_many_arguments)]
    fn full(
        p: &MapGenParams,
        terrain: &[Terrain],
        shade: &[u8],
        cost: &[u16],
        mass: &[u32],
        slope_q: u32,
        river_cross: u32,
        blocked: &[bool],
    ) -> Self {
        let (w, h) = (p.width as usize, p.height as usize);
        let tile = (0..w * h)
            .map(|i| {
                // Rzeka jest zamknięta (koszt 0), ale zapamiętuje rodzaj: na siatce zgrubnej blok z rzeką
                // jest drogi do wejścia z lądu, więc już wstępny rozrost nie przeskakuje rzek.
                let kind = if terrain[i] == Terrain::River { RIVER } else if owned(terrain, blocked, i) { LAND } else { 0 };
                cost[i] as u32 | (kind as u32) << 16 | (shade[i] as u32) << 24
            })
            .collect();
        Level {
            w,
            h,
            grid: CostGrid { w, h, tile, slope_q, river_cross },
            area: (0..w * h).map(|i| owned(terrain, blocked, i) as u8).collect(),
            mass: mass.to_vec(),
        }
    }

    /// Bloki 2 × 2: koszt kroku ×2 (krok = dwa kafle), więc handicapy przenoszą się bez zmian.
    /// Blok z choćby jednym kaflem rzeki jest rzeką; wysokość = najwyższy kafel lądu.
    fn coarsen(&self) -> Self {
        let (cw, ch) = (self.w.div_ceil(2), self.h.div_ceil(2));
        let mut tile = vec![0u32; cw * ch];
        let mut area = vec![0u8; cw * ch];
        let mut mass = vec![u32::MAX; cw * ch];
        for cy in 0..ch {
            for cx in 0..cw {
                let c = cy * cw + cx;
                let (mut sum, mut n, mut kind, mut top) = (0u32, 0u32, 0u8, 0u32);
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let (x, y) = (cx * 2 + dx, cy * 2 + dy);
                    if x >= self.w || y >= self.h {
                        continue;
                    }
                    let i = y * self.w + x;
                    let t = self.grid.tile[i];
                    let k = ((t >> 16) & 0xFF) as u8;
                    if k == RIVER {
                        kind = RIVER;
                    }
                    if t & 0xFFFF == 0 {
                        continue;
                    }
                    sum += t & 0xFFFF;
                    n += 1;
                    if kind != RIVER {
                        kind = LAND;
                    }
                    top = top.max(t >> 24);
                    area[c] += 1;
                    if mass[c] == u32::MAX {
                        mass[c] = self.mass[i];
                    }
                }
                if n > 0 {
                    let cost = (sum * 2 / n).clamp(1, 0xFFFF);
                    tile[c] = cost | (kind as u32) << 16 | top << 24;
                }
            }
        }
        let g = &self.grid;
        Level { w: cw, h: ch, grid: CostGrid { w: cw, h: ch, tile, slope_q: g.slope_q, river_cross: g.river_cross }, area, mass }
    }

    /// Kafel tej siatki leżący w bloku `c` zgrubnej siatki (najlepiej ląd z tego samego lądu).
    fn refine(&self, coarse: &Level, c: usize, mass: usize) -> usize {
        let (cx, cy) = (c % coarse.w, c / coarse.w);
        let mut best = None;
        for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            let (x, y) = (cx * 2 + dx, cy * 2 + dy);
            if x >= self.w || y >= self.h {
                continue;
            }
            let i = y * self.w + x;
            if self.mass[i] != mass as u32 {
                continue;
            }
            let land = (self.grid.tile[i] >> 16) & 0xFF == LAND as u32;
            if best.is_none() || land {
                best = Some(i);
                if land {
                    break;
                }
            }
        }
        best.unwrap_or(cy * 2 * self.w + cx * 2)
    }
}

/// Cel wyrównywania dla każdej prowincji.
struct Goal {
    /// Docelowa liczba kafli.
    area: Vec<f64>,
    scale: Vec<f64>,
    min_area: u32,
    max_area: u32,
    masses: usize,
}

/// Rundy: wzrost prowincji, korekta handicapów (za duża prowincja startuje później,
/// za mała – wcześniej) i przez pierwsze `move_rounds` – przesunięcie zalążków do środka (Lloyd).
#[allow(clippy::too_many_arguments)]
fn balance(
    level: &Level,
    regions: &mut [Region],
    rounds: usize,
    move_rounds: usize,
    goal: &Goal,
    rng: &mut Rng,
    owner: &mut [u32],
    queue: &mut BucketQueue,
) {
    let (w, n, k) = (level.w, level.w * level.h, regions.len());
    for round in 0..rounds {
        grow(&level.grid, regions, owner, queue);
        let mut area = vec![0u32; k];
        let (mut sx, mut sy) = (vec![0f64; k], vec![0f64; k]);
        for (i, &r) in owner.iter().enumerate() {
            if r == u32::MAX {
                continue;
            }
            let r = r as usize;
            let a = level.area[i] as u32;
            area[r] += a;
            sx[r] += (i % w) as f64 * a as f64;
            sy[r] += (i / w) as f64 * a as f64;
        }

        let gain = if round < move_rounds { 0.35 } else { 0.5 };
        let mut min_bias = vec![u32::MAX; goal.masses];
        for (r, reg) in regions.iter_mut().enumerate() {
            let a = area[r];
            let mut err = if a > 0 { (a as f64 / goal.area[r]).ln() } else { -1.0 };
            if a > goal.max_area {
                err = err.max(0.0) + (a as f64 / goal.max_area as f64).ln() * 1.5;
            } else if a < goal.min_area {
                err = err.min(0.0) - (goal.min_area as f64 / a.max(1) as f64).ln() * 1.5;
            }
            let sc = goal.scale[r];
            let delta = (err * gain * sc).clamp(-0.4 * sc, 0.4 * sc);
            reg.bias = (reg.bias as f64 + delta).max(0.0) as u32;
            min_bias[reg.mass] = min_bias[reg.mass].min(reg.bias);
        }
        for reg in regions.iter_mut() {
            reg.bias -= min_bias[reg.mass];
        }

        if round < move_rounds {
            // Lloyd: zalążek przechodzi na kafel prowincji najbliższy jej środka ciężkości.
            let mut best = vec![(f64::MAX, usize::MAX); k];
            for i in 0..n {
                let r = owner[i];
                if r == u32::MAX {
                    continue;
                }
                let r = r as usize;
                let (cx, cy) = (sx[r] / area[r] as f64, sy[r] / area[r] as f64);
                let d = ((i % w) as f64 - cx).powi(2) + ((i / w) as f64 - cy).powi(2);
                if d < best[r].0 {
                    best[r] = (d, i);
                }
            }
            for r in 0..k {
                if best[r].1 != usize::MAX {
                    regions[r].seed = best[r].1;
                    continue;
                }
                // Prowincja zniknęła (wchłonięta): nowy zalążek w losowym kaflu największej
                // prowincji tego samego lądu.
                let m = regions[r].mass;
                let big = (0..k)
                    .filter(|&j| regions[j].mass == m && j != r)
                    .max_by(|&a, &b| area[a].cmp(&area[b]).then(b.cmp(&a)));
                if let Some(big) = big {
                    let tiles: Vec<usize> = (0..n).filter(|&i| owner[i] == big as u32).collect();
                    if !tiles.is_empty() {
                        regions[r].seed = tiles[rng.below(tiles.len())];
                        regions[r].bias = regions[big].bias;
                    }
                }
            }
        }
    }
}

/// Kolejka kubełkowa (algorytm Diala) – koszty całkowite, krok < `BUCKETS`.
/// Bufory są używane ponownie między rundami.
struct BucketQueue {
    buckets: Vec<Vec<(u32, u32)>>,
    state: Vec<(u32, u32)>,
}

impl BucketQueue {
    fn new() -> Self {
        Self { buckets: (0..BUCKETS).map(|_| Vec::new()).collect(), state: Vec::new() }
    }
}

const LAND: u8 = 1;
const RIVER: u8 = 2;

/// Koszt drogi po mapie dla `grow`.
struct CostGrid {
    w: usize,
    h: usize,
    /// Dane kafla spakowane w jedno słowo (mniej chybień w pamięci podręcznej):
    /// bity 0..16 koszt wejścia (0 = kafel bez prowincji), 16..24 `LAND`/`RIVER`/0, 24..32 wysokość.
    tile: Vec<u32>,
    /// Koszt wspinaczki na jednostkę wysokości (× 256).
    slope_q: u32,
    river_cross: u32,
}

impl CostGrid {
    #[inline]
    fn open(&self, i: usize) -> bool {
        self.tile[i] & 0xFFFF != 0
    }

    #[inline]
    fn edge(&self, ti: u32, tj: u32) -> u32 {
        let (ki, kj) = (((ti >> 16) & 0xFF) as u8, ((tj >> 16) & 0xFF) as u8);
        let mut c = tj & 0xFFFF;
        if kj == RIVER && ki == LAND {
            c += self.river_cross;
        } else if ki == LAND && kj == LAND {
            c += (((ti >> 24) as i32 - (tj >> 24) as i32).unsigned_abs() * self.slope_q) >> 8;
        }
        // Po przekątnej koszt × √2 nadal musi zmieścić się w kolejce.
        c.min(BUCKETS as u32 * 2 / 3)
    }
}

/// Rozrost prowincji od zalążków: każdy kafel lądu trafia do zalążka o najmniejszym
/// `handicap + koszt drogi`. Zalążki o większym handicapie wchodzą do kolejki później.
fn grow(g: &CostGrid, regions: &[Region], owner: &mut [u32], q: &mut BucketQueue) {
    let (w, h) = (g.w, g.h);
    // (odległość, prowincja) razem – jedno chybienie w pamięci zamiast dwóch.
    let st = &mut q.state;
    st.clear();
    st.resize(w * h, (u32::MAX, u32::MAX));
    let mut order: Vec<usize> = (0..regions.len()).collect();
    order.sort_by_key(|&r| (regions[r].bias, r));
    let mut pending = order.into_iter().peekable();
    let mut cur: u32 = 0;
    for b in q.buckets.iter_mut() {
        b.clear();
    }
    let mut len = 0usize;
    loop {
        while let Some(&r) = pending.peek() {
            if regions[r].bias > cur {
                break;
            }
            pending.next();
            let s = regions[r].seed;
            if regions[r].bias < st[s].0 {
                st[s].0 = regions[r].bias;
                q.buckets[regions[r].bias as usize % BUCKETS].push((s as u32, r as u32));
                len += 1;
            }
        }
        if len == 0 {
            match pending.peek() {
                Some(&r) => {
                    cur = regions[r].bias;
                    continue;
                }
                None => break,
            }
        }
        let slot = cur as usize % BUCKETS;
        let mut bucket = std::mem::take(&mut q.buckets[slot]);
        let mut k = 0;
        while k < bucket.len() {
            let (i, r) = bucket[k];
            k += 1;
            len -= 1;
            let i = i as usize;
            if st[i].0 != cur || st[i].1 != u32::MAX {
                continue;
            }
            st[i].1 = r;
            let ti = g.tile[i];
            let (x, y) = (i % w, i / w);
            let (left, right, up, down) = (x > 0, x + 1 < w, y > 0, y + 1 < h);
            let mut relax = |j: usize, diagonal: bool| {
                let tj = g.tile[j];
                if tj & 0xFFFF == 0 || st[j].1 != u32::MAX {
                    return;
                }
                let c = g.edge(ti, tj);
                let d = cur + if diagonal { (c * 181) >> 7 } else { c };
                if d < st[j].0 {
                    st[j].0 = d;
                    if d == cur {
                        bucket.push((j as u32, r));
                    } else {
                        q.buckets[d as usize % BUCKETS].push((j as u32, r));
                    }
                    len += 1;
                }
            };
            if left { relax(i - 1, false); }
            if right { relax(i + 1, false); }
            if up { relax(i - w, false); }
            if down { relax(i + w, false); }
            // Po przekątnej (koszt × √2) – tylko gdy obaj wspólni sąsiedzi mają prowincje
            // (bez przeciekania przez narożnik wody), a z lądu nie przez narożnik rzeki.
            let from_land = (ti >> 16) & 0xFF == LAND as u32;
            let open = |j: usize| g.open(j) && !(from_land && (g.tile[j] >> 16) & 0xFF == RIVER as u32);
            if up && left && open(i - w) && open(i - 1) { relax(i - w - 1, true); }
            if up && right && open(i - w) && open(i + 1) { relax(i - w + 1, true); }
            if down && left && open(i + w) && open(i - 1) { relax(i + w - 1, true); }
            if down && right && open(i + w) && open(i + 1) { relax(i + w + 1, true); }
        }
        bucket.clear();
        q.buckets[slot] = bucket;
        cur += 1;
    }
    for (o, s) in owner.iter_mut().zip(st.iter()) {
        *o = s.1;
    }
}

/// Sąsiedzi w sąsiedztwie 4.
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

/// Drobne meandry granic: kafel lądu przejmuje prowincję kafla przesuniętego o szum (do `amp`
/// kafli). Przesunięcie nie przeskakuje przez rzekę, wodę ani góry, więc granice na rzekach zostają.
fn warp_borders(
    w: usize,
    h: usize,
    terrain: &[Terrain],
    (wx, wy): &(fastnoise_lite::FastNoiseLite, fastnoise_lite::FastNoiseLite),
    amp: f32,
    owner: &mut [u32],
) {
    if amp < 0.5 {
        return;
    }
    let (fx, fy) = CoarseField::pair(w, h, |x, y| (wx.get_noise_2d(x, y), wy.get_noise_2d(x, y)));
    let src = owner.to_vec();
    let reach = amp.ceil() as usize + 1;
    for y in reach..h.saturating_sub(reach) {
        for x in reach..w - reach {
            let i = y * w + x;
            // Tylko dostępny ląd (bez rzek, gór i wody) zmienia prowincję.
            if !terrain[i].is_land() || src[i] == u32::MAX {
                continue;
            }
            // Tylko w pobliżu granicy (poza nią przesunięcie i tak nic nie zmienia).
            let r = src[i];
            if src[i - reach] == r && src[i + reach] == r && src[i - reach * w] == r && src[i + reach * w] == r {
                continue;
            }
            let (dx, dy) = (fx.get(x, y) * amp, fy.get(x, y) * amp);
            let at = |t: f32| ((y as f32 + dy * t).round() as usize) * w + (x as f32 + dx * t).round() as usize;
            let j = at(1.0);
            if src[j] == r || src[j] == u32::MAX || !terrain[j].is_land() {
                continue;
            }
            if [0.25, 0.5, 0.75].iter().all(|&t| terrain[at(t)].is_land() && src[at(t)] != u32::MAX) {
                owner[i] = src[j];
            }
        }
    }
}

/// Okruchy prowincji odcięte od jej zalążka (np. przez przewężenie) dołączają do sąsiada,
/// z którym mają najdłuższą wspólną granicę.
fn make_contiguous(w: usize, h: usize, regions: &[Region], owner: &mut [u32]) {
    let n = w * h;
    for _ in 0..4 {
        // Spójne kawałki każdej prowincji osobno: flood fill po kaflach tej samej prowincji.
        let mut seen = vec![false; n];
        let mut keep = vec![false; n];
        let mut stack = Vec::new();
        let mut changed = false;
        for (r, reg) in regions.iter().enumerate() {
            // Kawałek z zalążkiem zostaje.
            let s = reg.seed;
            if owner[s] != r as u32 || keep[s] {
                continue;
            }
            keep[s] = true;
            stack.push(s);
            while let Some(i) = stack.pop() {
                for j in neighbors4(w, h, i) {
                    if !keep[j] && owner[j] == r as u32 {
                        keep[j] = true;
                        stack.push(j);
                    }
                }
            }
        }
        for start in 0..n {
            if owner[start] == u32::MAX || keep[start] || seen[start] {
                continue;
            }
            // Okruch: zbierz go i policz granice z sąsiadami.
            let r = owner[start];
            let mut tiles = Vec::new();
            seen[start] = true;
            stack.push(start);
            while let Some(i) = stack.pop() {
                tiles.push(i);
                for j in neighbors4(w, h, i) {
                    if owner[j] == r && !seen[j] && !keep[j] {
                        seen[j] = true;
                        stack.push(j);
                    }
                }
            }
            let mut border: Vec<(u32, u32)> = Vec::new();
            for &i in &tiles {
                for j in neighbors4(w, h, i) {
                    let o = owner[j];
                    if o != u32::MAX && o != r {
                        match border.iter_mut().find(|(k, _)| *k == o) {
                            Some(e) => e.1 += 1,
                            None => border.push((o, 1)),
                        }
                    }
                }
            }
            if let Some(&(to, _)) = border.iter().max_by_key(|&&(k, c)| (c, std::cmp::Reverse(k))) {
                for &i in &tiles {
                    owner[i] = to;
                }
                changed = true;
            } else {
                // Okruch bez sąsiadów (osobna wysepka) – zostaje.
                for &i in &tiles {
                    keep[i] = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
}

/// Prowincje mniejsze niż połowa `min_area` (np. zamknięte w zakolu rzeki) dołączają
/// do sąsiada o najmniejszej wartości.
fn merge_small(w: usize, h: usize, min_area: u32, owner: &mut [u32], count: usize) {
    let n = w * h;
    let mut tiles: Vec<Vec<u32>> = vec![Vec::new(); count];
    for i in 0..n {
        if owner[i] != u32::MAX {
            tiles[owner[i] as usize].push(i as u32);
        }
    }
    let limit = (min_area / 2) as usize;
    let mut small: Vec<usize> = (0..count).filter(|&r| !tiles[r].is_empty() && tiles[r].len() < limit).collect();
    // Od najmniejszych, deterministycznie.
    small.sort_by_key(|&r| (tiles[r].len(), r));
    for r in small {
        if tiles[r].is_empty() || tiles[r].len() >= limit {
            continue;
        }
        let mut best: Option<(usize, u32)> = None;
        for &i in &tiles[r] {
            for j in neighbors4(w, h, i as usize) {
                let o = owner[j];
                if o != u32::MAX && o != r as u32 && best.is_none_or(|(a, k)| (tiles[o as usize].len(), o) < (a, k)) {
                    best = Some((tiles[o as usize].len(), o));
                }
            }
        }
        if let Some((_, o)) = best {
            let moved = std::mem::take(&mut tiles[r]);
            for &i in &moved {
                owner[i as usize] = o;
            }
            tiles[o as usize].extend(moved);
        }
    }
}

/// Małe wyspy (bez własnych prowincji) dołączają przez morze do najbliższej prowincji;
/// za daleko od innych – dostają własną.
#[allow(clippy::too_many_arguments)]
fn attach_islands(
    w: usize,
    h: usize,
    terrain: &[Terrain],
    blocked: &[bool],
    mass: &[u32],
    count: &[usize],
    owner: &mut [u32],
    regions: usize,
    min_area: u32,
) {
    let n = w * h;
    let orphan = |i: usize| mass[i] != u32::MAX && count[mass[i] as usize] == 0;
    if !(0..n).any(orphan) {
        return;
    }
    // BFS przez wodę od wszystkich kafli prowincji (zasięg rośnie z rozmiarem prowincji).
    let reach = ((min_area as f32).sqrt() * 1.5).clamp(8.0, 60.0) as u32;
    let mut dist = vec![u32::MAX; n];
    let mut from = vec![u32::MAX; n];
    let mut frontier: Vec<usize> = (0..n).filter(|&i| owner[i] != u32::MAX).collect();
    for &i in &frontier {
        dist[i] = 0;
        from[i] = owner[i];
    }
    let mut d = 0;
    while !frontier.is_empty() && d < reach {
        d += 1;
        let mut next = Vec::new();
        for &i in &frontier {
            for j in neighbors4(w, h, i) {
                if dist[j] == u32::MAX && (!owned(terrain, blocked, j) || orphan(j)) {
                    dist[j] = d;
                    from[j] = from[i];
                    if !orphan(j) {
                        next.push(j);
                    }
                }
            }
        }
        frontier = next;
    }
    // Wyspa bierze prowincję, która dotarła do niej najwcześniej (pierwszy kafel w kolejności skanowania).
    let mut island_owner: Vec<u32> = vec![u32::MAX; count.len()];
    let mut island_dist: Vec<u32> = vec![u32::MAX; count.len()];
    for i in 0..n {
        if orphan(i) && from[i] != u32::MAX {
            let m = mass[i] as usize;
            if dist[i] < island_dist[m] {
                island_dist[m] = dist[i];
                island_owner[m] = from[i];
            }
        }
    }
    let mut next_id = regions as u32;
    for i in 0..n {
        if !orphan(i) {
            continue;
        }
        let m = mass[i] as usize;
        if island_owner[m] == u32::MAX {
            // Samotna wyspa – własna prowincja.
            island_owner[m] = next_id;
            next_id += 1;
        }
        owner[i] = island_owner[m];
    }
}

/// Numeracja od 1 w kolejności skanowania i podsumowanie prowincji.
fn finish(w: usize, h: usize, terrain: &[Terrain], biome: &[u8], owner: &[u32]) -> Provinces {
    let n = w * h;
    let mut ids: Vec<u16> = vec![0; n];
    let slots = owner.iter().filter(|&&o| o != u32::MAX).max().map_or(0, |&o| o as usize + 1);
    let mut map: Vec<u16> = vec![0; slots];
    let mut list: Vec<Province> = Vec::new();
    let mut sums: Vec<(f64, f64)> = Vec::new(); // (x, y)
    let mut biomes: Vec<[u32; Biome::COUNT]> = Vec::new();
    for i in 0..n {
        let r = owner[i];
        if r == u32::MAX {
            continue;
        }
        if map[r as usize] == 0 {
            list.push(Province { id: list.len() as u16 + 1, ..Default::default() });
            sums.push((0.0, 0.0));
            biomes.push([0; Biome::COUNT]);
            map[r as usize] = list.len() as u16;
        }
        let id = map[r as usize];
        ids[i] = id;
        let (pr, s) = (&mut list[id as usize - 1], &mut sums[id as usize - 1]);
        pr.area += 1;
        s.0 += (i % w) as f64;
        s.1 += (i / w) as f64;
        biomes[id as usize - 1][(biome[i] as usize).min(Biome::COUNT - 1)] += 1;
        for j in neighbors4(w, h, i) {
            match terrain[j] {
                Terrain::Ocean => pr.coastal = true,
                Terrain::River => pr.river = true,
                Terrain::Lake => pr.lake = true,
                Terrain::Mountains => pr.mountains = true,
                _ => {}
            }
        }
    }
    // Środek: kafel prowincji najbliższy jej środka ciężkości.
    let mut best = vec![(f64::MAX, 0usize); list.len()];
    for i in 0..n {
        if ids[i] == 0 {
            continue;
        }
        let k = ids[i] as usize - 1;
        let (cx, cy) = (sums[k].0 / list[k].area as f64, sums[k].1 / list[k].area as f64);
        let d = ((i % w) as f64 - cx).powi(2) + ((i / w) as f64 - cy).powi(2);
        if d < best[k].0 {
            best[k] = (d, i);
        }
    }
    for (k, pr) in list.iter_mut().enumerate() {
        let b = &biomes[k];
        pr.biome = (0..Biome::COUNT).fold(0, |best, x| if b[x] > b[best] { x } else { best }) as u8;
        pr.center_x = (best[k].1 % w) as u16;
        pr.center_y = (best[k].1 / w) as u16;
    }
    Provinces { id: ids, list }
}
