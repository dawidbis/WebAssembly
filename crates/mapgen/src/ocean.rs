//! Dno oceanu: szelf, stok kontynentalny, głębia i rzeźba dna.
//!
//! Profil głębokości zależy od odległości od lądu: płytki szelf (którego szerokość zmienia się
//! szumem – szerokie ławice obok urwisk), potem stromy stok i głębia. Na stoku i w głębi
//! szum grzbietowy dodaje podwodne grzbiety, rowy i góry podwodne.
//!
//! Etap ma własny RNG i nie zmienia typów kafli, więc nie rusza terenu, rzek ani biomów.

use fastnoise_lite::FractalType;

use crate::{
    MapGenParams, Terrain,
    layout::Layout,
    util::{CoarseField, Rng, distance_field, fractal, smoothstep},
};

const OCEAN_SALT: u64 = 0x2545_F491_4F6C_DD1D;

/// Udział głębokości (0..1) na krawędzi szelfu i u podnóża stoku.
const SHELF_DEPTH: f32 = 0.14;
const SLOPE_DEPTH: f32 = 0.6;
/// Głębokość równiny abisalnej – poniżej 1, żeby rzeźba dna mogła iść w obie strony (rowy i grzbiety).
const ABYSS_DEPTH: f32 = 0.82;

/// Zapisuje głębokość oceanu (0..255) do `shade` dla kafli oceanu.
pub fn build(p: &MapGenParams, l: &Layout, terrain: &[Terrain], shade: &mut [u8]) {
    let (w, h) = (p.width as usize, p.height as usize);
    let cs = l.chunk_size();
    let mut rng = Rng::new(p.seed as u64 ^ OCEAN_SALT);
    let shelf_noise = fractal(rng.noise_seed(), FractalType::FBm, 1.6 / cs, 3);
    let ridges = fractal(rng.noise_seed(), FractalType::Ridged, 1.1 / cs, 3);
    let swell = fractal(rng.noise_seed(), FractalType::FBm, 0.6 / cs, 2);

    let dist = distance_field(w, h, |i| terrain[i] != Terrain::Ocean);
    let (shelf_var, floor) = CoarseField::pair(w, h, |fx, fy| {
        let r = (ridges.get_noise_2d(fx, fy) + 1.0) * 0.5;
        // Ujemne = grzbiety i góry podwodne (płycej), dodatnie = rowy (głębiej).
        (shelf_noise.get_noise_2d(fx, fy), swell.get_noise_2d(fx, fy) * 0.6 - (r * r - 0.3) * 0.9)
    });

    let shelf_base = p.shelf_width as f32;
    // Stromość 1 = stok na ~1/4 szerokości szelfu, 0 = łagodny spadek na ~3 szerokości.
    let slope_k = 3.0 - 2.75 * p.slope_steepness;
    let abyss = cs * 0.8;
    let relief_amp = 0.3 * p.seabed_relief;

    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if terrain[i] != Terrain::Ocean {
                continue;
            }
            let d = dist[i];
            let shelf = (shelf_base * (1.0 + p.shelf_variation * 1.4 * shelf_var.get(x, y))).max(1.0);
            let slope = (shelf * slope_k).max(1.5);
            let base = SHELF_DEPTH * smoothstep(0.0, shelf, d)
                + (SLOPE_DEPTH - SHELF_DEPTH) * smoothstep(shelf, shelf + slope, d)
                + (ABYSS_DEPTH - SLOPE_DEPTH) * smoothstep(shelf + slope, shelf + slope + abyss, d);
            // Rzeźba dna tylko poza szelfem – szelf zostaje płaski i jasny.
            let deep = smoothstep(shelf, shelf + slope * 1.5, d);
            let depth = (base + floor.get(x, y) * relief_amp * deep).clamp(0.02, 1.0);
            shade[i] = (depth * 255.0) as u8;
        }
    }
}
