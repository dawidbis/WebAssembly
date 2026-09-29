use fastnoise_lite::{DomainWarpType, FastNoiseLite, FractalType, NoiseType};

/// SplitMix64 – mały, szybki i w pełni deterministyczny RNG (bez zależności).
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed ^ 0x9E37_79B9_7F4A_7C15)
    }
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Liczba z przedziału [0, 1).
    pub fn f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }
    /// Liczba całkowita z przedziału [0, n).
    pub fn below(&mut self, n: usize) -> usize {
        (((self.next_u64() >> 32) * n as u64) >> 32) as usize
    }
    /// Seed dla kolejnej warstwy szumu.
    pub fn noise_seed(&mut self) -> i32 {
        self.next_u64() as i32
    }
}

pub fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Szum fraktalny (fBm lub ridged) z częstotliwością w jednostkach „na kafel".
pub fn fractal(seed: i32, kind: FractalType, freq: f32, octaves: i32) -> FastNoiseLite {
    let mut n = FastNoiseLite::with_seed(seed);
    n.set_noise_type(Some(NoiseType::OpenSimplex2));
    n.set_frequency(Some(freq));
    n.set_fractal_type(Some(kind));
    n.set_fractal_octaves(Some(octaves));
    n.set_fractal_lacunarity(Some(2.0));
    n.set_fractal_gain(Some(0.5));
    n
}

/// Odkształcenie współrzędnych (domain warp) – daje organiczne, nieprostokątne kształty.
pub fn warp(seed: i32, freq: f32, amp: f32, octaves: i32) -> FastNoiseLite {
    let mut n = FastNoiseLite::with_seed(seed);
    n.set_domain_warp_type(Some(DomainWarpType::OpenSimplex2));
    n.set_domain_warp_amp(Some(amp));
    n.set_frequency(Some(freq));
    n.set_fractal_type(Some(FractalType::DomainWarpProgressive));
    n.set_fractal_octaves(Some(octaves));
    n.set_fractal_lacunarity(Some(2.0));
    n.set_fractal_gain(Some(0.5));
    n
}

/// Transformata odległości (chamfer 3-4, ~euklidesowa) od kafli, dla których `is_source` = true.
/// Zwraca odległość w kaflach; `f32::MAX / 2` gdy nie ma żadnego źródła.
pub fn distance_field(w: usize, h: usize, is_source: impl Fn(usize) -> bool) -> Vec<f32> {
    const INF: u32 = u32::MAX / 4;
    let mut d = vec![INF; w * h];
    for i in 0..w * h {
        if is_source(i) {
            d[i] = 0;
        }
    }
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let mut v = d[i];
            if x > 0 { v = v.min(d[i - 1] + 3); }
            if y > 0 {
                v = v.min(d[i - w] + 3);
                if x > 0 { v = v.min(d[i - w - 1] + 4); }
                if x + 1 < w { v = v.min(d[i - w + 1] + 4); }
            }
            d[i] = v;
        }
    }
    for y in (0..h).rev() {
        for x in (0..w).rev() {
            let i = y * w + x;
            let mut v = d[i];
            if x + 1 < w { v = v.min(d[i + 1] + 3); }
            if y + 1 < h {
                v = v.min(d[i + w] + 3);
                if x + 1 < w { v = v.min(d[i + w + 1] + 4); }
                if x > 0 { v = v.min(d[i + w - 1] + 4); }
            }
            d[i] = v;
        }
    }
    d.into_iter()
        .map(|v| if v >= INF { f32::MAX / 2.0 } else { v as f32 / 3.0 })
        .collect()
}

/// Etykietowanie spójnych obszarów (sąsiedztwo 4). Zwraca (etykieta per kafel, rozmiary).
/// Etykieta `u32::MAX` = kafel poza maską.
pub fn components(w: usize, h: usize, mask: impl Fn(usize) -> bool) -> (Vec<u32>, Vec<u32>) {
    let mut label = vec![u32::MAX; w * h];
    let mut sizes = Vec::new();
    let mut stack = Vec::new();
    for start in 0..w * h {
        if label[start] != u32::MAX || !mask(start) {
            continue;
        }
        let id = sizes.len() as u32;
        let mut size = 0u32;
        label[start] = id;
        stack.push(start);
        while let Some(i) = stack.pop() {
            size += 1;
            let (x, y) = (i % w, i / w);
            let mut visit = |j: usize| {
                if label[j] == u32::MAX && mask(j) {
                    label[j] = id;
                    stack.push(j);
                }
            };
            if x > 0 { visit(i - 1); }
            if x + 1 < w { visit(i + 1); }
            if y > 0 { visit(i - w); }
            if y + 1 < h { visit(i + w); }
        }
        sizes.push(size);
    }
    (label, sizes)
}

/// Wartość progowa, powyżej której leży `share` próbek (histogram, bez sortowania).
pub fn quantile_above(values: impl Iterator<Item = f32> + Clone, share: f32) -> f32 {
    const BINS: usize = 4096;
    let (mut lo, mut hi, mut n) = (f32::MAX, f32::MIN, 0usize);
    for v in values.clone() {
        lo = lo.min(v);
        hi = hi.max(v);
        n += 1;
    }
    if n == 0 || hi <= lo {
        return hi;
    }
    let mut hist = vec![0u32; BINS];
    let scale = (BINS - 1) as f32 / (hi - lo);
    for v in values {
        hist[((v - lo) * scale) as usize] += 1;
    }
    let target = (share.clamp(0.0, 1.0) * n as f32) as u32;
    let mut acc = 0u32;
    for b in (0..BINS).rev() {
        acc += hist[b];
        if acc >= target {
            return lo + b as f32 / scale;
        }
    }
    lo
}

/// Pole wartości liczone na rzadszej siatce (co `STEP` kafli) i interpolowane dwuliniowo.
/// Szum jest gładki, więc to ~4× mniej obliczeń bez widocznej straty jakości.
pub struct CoarseField {
    cw: usize,
    ch: usize,
    data: Vec<f32>,
}

impl CoarseField {
    const STEP: usize = 2;

    pub fn new(w: usize, h: usize, f: impl Fn(f32, f32) -> f32) -> Self {
        Self::pair(w, h, |x, y| (f(x, y), 0.0)).0
    }

    /// Dwa pola naraz – gdy oba potrzebują tego samego (kosztownego) domain warpa.
    pub fn pair(w: usize, h: usize, f: impl Fn(f32, f32) -> (f32, f32)) -> (Self, Self) {
        let (cw, ch) = (w / Self::STEP + 2, h / Self::STEP + 2);
        let (mut a, mut b) = (Vec::with_capacity(cw * ch), Vec::with_capacity(cw * ch));
        for cy in 0..ch {
            for cx in 0..cw {
                let (va, vb) = f((cx * Self::STEP) as f32, (cy * Self::STEP) as f32);
                a.push(va);
                b.push(vb);
            }
        }
        (Self { cw, ch, data: a }, Self { cw, ch, data: b })
    }

    pub fn get(&self, x: usize, y: usize) -> f32 {
        let (cx, cy) = (x / Self::STEP, y / Self::STEP);
        let (tx, ty) = ((x % Self::STEP) as f32 / Self::STEP as f32, (y % Self::STEP) as f32 / Self::STEP as f32);
        let (cx1, cy1) = ((cx + 1).min(self.cw - 1), (cy + 1).min(self.ch - 1));
        let at = |x: usize, y: usize| self.data[y * self.cw + x];
        let top = at(cx, cy) + (at(cx1, cy) - at(cx, cy)) * tx;
        let bottom = at(cx, cy1) + (at(cx1, cy1) - at(cx, cy1)) * tx;
        top + (bottom - top) * ty
    }
}
