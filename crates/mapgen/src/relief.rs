//! Ląd, wybrzeża, rzeźba terenu i klasyfikacja (równiny / wyżyny / góry).

use fastnoise_lite::FractalType;

use crate::{
    MapGenParams, MapStats, Terrain,
    layout::Layout,
    util::{self, CoarseField, Rng, components, distance_field, fractal, quantile_above, smoothstep},
};

pub struct Relief {
    pub w: usize,
    pub h: usize,
    pub terrain: Vec<Terrain>,
    /// Wysokość lądu (mniej więcej 0..1.5); woda = 0.
    pub elevation: Vec<f32>,
    /// Odległość kafla lądu od najbliższej wody.
    pub coast_dist: Vec<f32>,
    pub shade: Vec<u8>,
    /// Kafle rzek: odległość do ujścia wzdłuż nurtu (1 = przy ujściu), 0 = nie rzeka. Wypełnia hydrologia.
    pub river_flow: Vec<u16>,
    pub removed_islands: u32,
    pub continents: u32,
}

pub fn build(p: &MapGenParams, l: &Layout, rng: &mut Rng) -> Relief {
    let (w, h) = (p.width as usize, p.height as usize);
    let n = w * h;
    let cs = l.chunk_size();
    let keep_off = p.keep_off_edges;
    let margin = if keep_off { p.edge_margin as f32 } else { 0.0 };

    // --- 1. Pole „ile jestem w głębi lądu" wynikające z układu chunków -------------
    let water_chunk: Vec<bool> = (0..n).map(|i| l.is_water_chunk_at(i % w, i / w)).collect();
    let d_water = distance_field(w, h, |i| water_chunk[i]);
    let d_land = distance_field(w, h, |i| !water_chunk[i]);
    let edge_dist = |x: usize, y: usize| (x.min(w - 1 - x).min(y).min(h - 1 - y)) as f32 + 1.0;
    // Odległość do tego, czego ląd nie może dotykać (chunk wodny, przy keep_off też krawędź mapy).
    let blocked_dist = |i: usize| {
        let d = d_water[i];
        if keep_off { d.min(edge_dist(i % w, i / w)) } else { d }
    };
    let signed: Vec<f32> =
        (0..n).map(|i| if water_chunk[i] { -d_land[i].min(cs * 2.0) } else { blocked_dist(i).min(cs * 2.0) }).collect();

    // --- 2. Maska lądu: pole chunków odkształcone szumem ------------------------------
    // Silny, wolnozmienny domain warp łamie prostokątne kształty chunków,
    // a fBm dodaje poszarpane wybrzeże w małej skali.
    let r = p.coast_roughness;
    let warp = util::warp(rng.noise_seed(), 1.0 / cs, cs * (0.20 + 0.50 * r), 3);
    let coast = fractal(rng.noise_seed(), FractalType::FBm, 2.5 / cs, 5);
    let bays = fractal(rng.noise_seed(), FractalType::FBm, 1.2 / cs, 2);
    let falloff = cs * 0.35;
    let amp = 0.30 + 0.90 * r;
    // Przy keep_off odsuwamy wybrzeże od twardej granicy, żeby nie powstawały proste odcinki.
    let bias = if keep_off { 0.55 * amp + 0.25 } else { 0.0 };
    // Przy keep_off druga, nieodkształcona granica z własnym szumem: wybrzeże przy krawędzi
    // mapy / chunku wodnego jest poszarpane, a nie ucięte prostą linią.
    let edge_coast = fractal(rng.noise_seed(), FractalType::FBm, 3.0 / cs, 4);
    let potential = CoarseField::new(w, h, |fx, fy| {
        let (wx, wy) = warp.domain_warp_2d(fx, fy);
        let sx = (wx.round() as isize).clamp(0, w as isize - 1) as usize;
        let sy = (wy.round() as isize).clamp(0, h as isize - 1) as usize;
        let shape = (signed[sy * w + sx] - margin) / falloff - bias
            + coast.get_noise_2d(wx, wy) * amp
            + bays.get_noise_2d(fx, fy) * (0.5 + 0.8 * r);
        if !keep_off {
            return shape;
        }
        let i = (fy as usize).min(h - 1) * w + (fx as usize).min(w - 1);
        let fence = (blocked_dist(i) - margin) / (cs * 0.2) - 0.45 + edge_coast.get_noise_2d(fx, fy) * 0.55;
        shape.min(fence)
    });

    let mut land = vec![false; n];
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if keep_off && (water_chunk[i] || blocked_dist(i) < margin) {
                continue;
            }
            land[i] = potential.get(x, y) > 0.0;
        }
    }

    // --- 3. Usuń za małe wyspy ---------------------------------------------------------
    let (lab, sizes) = components(w, h, |i| land[i]);
    let min_island = p.min_island_area;
    let removed_islands = sizes.iter().filter(|&&s| s < min_island).count() as u32;
    let continents = sizes.iter().filter(|&&s| s >= min_island).count() as u32;
    for i in 0..n {
        if land[i] && sizes[lab[i] as usize] < min_island {
            land[i] = false;
        }
    }

    // --- 4. Ocean vs. wody zamknięte (jeziora przybrzeżne albo zasypane dziury) -------
    let (wlab, wsizes) = components(w, h, |i| !land[i]);
    let mut is_ocean = vec![false; wsizes.len()];
    for i in 0..n {
        let (x, y) = (i % w, i / w);
        if !land[i] && (water_chunk[i] || x == 0 || y == 0 || x == w - 1 || y == h - 1) {
            is_ocean[wlab[i] as usize] = true;
        }
    }
    let mut terrain = vec![Terrain::Ocean; n];
    for i in 0..n {
        terrain[i] = if land[i] {
            Terrain::Plains
        } else {
            let id = wlab[i] as usize;
            if is_ocean[id] {
                Terrain::Ocean
            } else if p.lakes && wsizes[id] >= p.min_lake_area {
                Terrain::Lake
            } else {
                Terrain::Plains // zbyt mała kałuża – zasypujemy
            }
        };
    }

    // --- 5. Rzeźba: pagórki + podgórza + pasma górskie ---------------------------------
    let coast_dist = distance_field(w, h, |i| !terrain[i].is_land());
    let hills = fractal(rng.noise_seed(), FractalType::FBm, 3.0 / cs, 4);
    let rs = cs * p.range_scale;
    // Osie pasm leżą wzdłuż linii zerowych wolnozmiennego szumu → długie, kręte łańcuchy.
    let axis_noise = fractal(rng.noise_seed(), FractalType::FBm, 0.9 / rs, 2);
    // Drugi szum tnie łańcuchy na odcinki (masywy zamiast jednej nieskończonej linii).
    let segments = fractal(rng.noise_seed(), FractalType::FBm, 1.3 / rs, 1);
    // Ridged noise daje grzbiety i doliny wewnątrz masywu.
    let ridges = fractal(rng.noise_seed(), FractalType::Ridged, 4.0 / cs, 4);
    let range_warp = util::warp(rng.noise_seed(), 1.5 / cs, cs * 0.15, 2);
    let bw = 0.08 + 1.2 * p.mountain_share;

    let rolling = CoarseField::new(w, h, |fx, fy| (hills.get_noise_2d(fx, fy) + 1.0) * 0.5);
    let segment = CoarseField::new(w, h, |fx, fy| smoothstep(-0.35, 0.25, segments.get_noise_2d(fx, fy)));
    let (axis, ridge) = CoarseField::pair(w, h, |fx, fy| {
        let (wx, wy) = range_warp.domain_warp_2d(fx, fy);
        let r = ((ridges.get_noise_2d(wx, wy) + 1.0) * 0.5).clamp(0.0, 1.0);
        (1.0 - axis_noise.get_noise_2d(wx, wy).abs(), 0.25 + 0.75 * r * r)
    });

    let mut elevation = vec![0.0f32; n];
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if !terrain[i].is_land() {
                continue;
            }
            let inland = smoothstep(0.0, cs * 0.8, coast_dist[i]);
            let keep = segment.get(x, y) * smoothstep(0.0, cs * 0.12, coast_dist[i]);
            let a = axis.get(x, y);
            let core = smoothstep(1.0 - bw, 1.0 - bw * 0.15, a) * keep;
            let foothills = smoothstep(1.0 - bw * 3.0, 1.0 - bw * 0.5, a) * keep;
            elevation[i] = 0.07 * inland + 0.22 * rolling.get(x, y) + 0.25 * foothills + core * ridge.get(x, y);
        }
    }

    // --- 6. Klasyfikacja percentylami: stałe proporcje terenu niezależnie od seeda -----
    let land_e = || (0..n).filter(|&i| terrain[i].is_land()).map(|i| elevation[i]);
    let thr = |share: f32| if share <= 0.0 { f32::MAX } else { quantile_above(land_e(), share) };
    let mountain_thr = thr(p.mountain_share);
    let highland_thr = thr(p.mountain_share + p.highland_share);
    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    for e in land_e() {
        lo = lo.min(e);
        hi = hi.max(e);
    }
    let span = (hi - lo).max(1e-6);

    let mut shade = vec![0u8; n];
    for i in 0..n {
        if terrain[i] == Terrain::Plains {
            let e = elevation[i];
            terrain[i] = if e >= mountain_thr {
                Terrain::Mountains
            } else if e >= highland_thr {
                Terrain::Highlands
            } else {
                Terrain::Plains
            };
            shade[i] = (1.0 + (e - lo) / span * 254.0) as u8;
        }
    }

    Relief { w, h, terrain, elevation, coast_dist, shade, river_flow: vec![0; n], removed_islands, continents }
}

impl Relief {
    pub fn stats(&self, lakes: u32, rivers: u32) -> MapStats {
        let mut count = [0u32; 6];
        for &t in &self.terrain {
            count[t as usize] += 1;
        }
        let land = (count[3] + count[4] + count[5]).max(1) as f32;
        MapStats {
            land_share: land / self.terrain.len() as f32,
            plains_share: count[3] as f32 / land,
            highlands_share: count[4] as f32 / land,
            mountains_share: count[5] as f32 / land,
            continents: self.continents,
            removed_islands: self.removed_islands,
            lakes,
            rivers,
            ..Default::default()
        }
    }
}
