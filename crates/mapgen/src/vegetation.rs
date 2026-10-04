//! Roślinność (lasy).
//!
//! Las: gęstość 0..255 na kafel lądu. Typ lasu nie jest zapisywany osobno – wynika z biomu
//! (umiarkowany → liściasty, zimny → tajga, wilgotny → dżungla, step → zagajniki, pustynia → oazy),
//! więc w strefach przejścia biomów las też przechodzi płynnie.
//!
//! Gdzie rośnie las: zwarte masywy z szumu, więcej przy rzekach, jeziorach i wybrzeżu, mniej wyżej.
//! Udział lasu w każdym biomie jest ustalany percentylem (jak proporcje terenu), więc nie zależy
//! od seeda. Próg jest mieszany między biomami tak samo jak kolory, bez szwów na granicy biomów.
//!
//! Etap ma własny RNG i nie zmienia terenu ani biomów.

use fastnoise_lite::FractalType;

use crate::{
    layout::Layout,
    util::{distance_field, fractal, quantile_above, smoothstep, CoarseField, Rng},
    Biome, MapGenParams, Terrain,
};

const VEGETATION_SALT: u64 = 0xD6E8_FEB8_6659_FD93;

/// Waga bliskości wody w ocenie miejsca pod las, dla każdego biomu (kolejność `Biome::ALL`).
/// Na stepie i pustyni las rośnie prawie wyłącznie przy wodzie (lasy łęgowe, oazy).
const MOISTURE_WEIGHT: [f32; 5] = [0.35, 1.6, 0.3, 0.2, 1.1];
/// Udział biomu zimnego (do szybszego rzednięcia tajgi z wysokością).
const COLD: [f32; 5] = {
    let mut v = [0.0; 5];
    v[Biome::Cold as usize] = 1.0;
    v
};

pub struct Vegetation {
    /// Gęstość lasu 0..255 (≥ 128 = kafel leśny w rozgrywce).
    pub forest: Vec<u8>,
}

impl Vegetation {
    /// Udział lasu w lądzie (gęstość ≥ 128).
    pub fn forest_share(&self, terrain: &[Terrain]) -> f32 {
        let (mut land, mut forest) = (0u32, 0u32);
        for (i, t) in terrain.iter().enumerate() {
            if t.is_land() {
                land += 1;
                forest += (self.forest[i] >= 128) as u32;
            }
        }
        forest as f32 / land.max(1) as f32
    }
}

/// Wartość biomowa w kaflu, mieszana tak samo jak kolory: `biome` z wagą 1 − mix, `biome_other` z wagą mix.
fn blend(values: &[f32; 5], biome: u8, other: u8, mix: u8) -> f32 {
    let k = mix as f32 / 256.0;
    values[biome as usize] * (1.0 - k) + values[other as usize] * k
}

pub fn build(
    p: &MapGenParams,
    l: &Layout,
    terrain: &[Terrain],
    shade: &[u8],
    biome: &[u8],
    biome_other: &[u8],
    biome_mix: &[u8],
) -> Vegetation {
    let (w, h) = (p.width as usize, p.height as usize);
    let n = w * h;
    let cs = l.chunk_size();
    let mut rng = Rng::new(p.seed as u64 ^ VEGETATION_SALT);
    let clumps = fractal(rng.noise_seed(), FractalType::FBm, (2.0 + 4.0 * (1.0 - p.forest_clumping)) / cs, 4);
    let glades = fractal(rng.noise_seed(), FractalType::FBm, 9.0 / cs, 2);
    // Dawny szum żyzności – losowanie zostaje, żeby nie zmieniać kolejnych ziaren (i lasów).
    let _ = rng.noise_seed();
    // Drobny szum liczony per kafel: postrzępione skraje lasu zamiast gładkich plam.
    let detail = fractal(rng.noise_seed(), FractalType::FBm, 0.18, 2);

    // Bliskość wody (rzeki, jeziora, morze): 1 przy brzegu, 0 daleko.
    let water_dist = distance_field(w, h, |i| !terrain[i].is_land());
    let reach = (cs * 0.3).max(4.0);
    let moisture = |i: usize| 1.0 - smoothstep(0.0, reach, water_dist[i]);

    let clump = CoarseField::new(w, h, |fx, fy| clumps.get_noise_2d(fx, fy) + glades.get_noise_2d(fx, fy) * 0.25);

    let mut forest = vec![0u8; n];
    let land = |i: usize| terrain[i].is_land();

    // Wysokość lądu 0..1 (w `shade`), kara za wysokość: tajga rzednie szybciej (tundra).
    let height = |i: usize| shade[i] as f32 / 255.0;

    if p.forest {
        let weight = |i: usize| blend(&MOISTURE_WEIGHT, biome[i], biome_other[i], biome_mix[i]);
        let mut score = vec![f32::MIN; n];
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                if !land(i) || terrain[i] == Terrain::Mountains {
                    continue;
                }
                let cold = blend(&COLD, biome[i], biome_other[i], biome_mix[i]);
                let altitude = height(i) * (0.35 + 0.45 * cold) + if terrain[i] == Terrain::Highlands { 0.12 } else { 0.0 };
                score[i] = clump.get(x, y) + detail.get_noise_2d(x as f32, y as f32) * 0.07 + weight(i) * p.forest_moisture * 2.0 * moisture(i) - altitude;
            }
        }

        // Próg per biom z percentyla wśród kafli, w których dany biom dominuje.
        let shares = p.forest_shares();
        let thresholds: [f32; 5] = std::array::from_fn(|b| {
            let values = (0..n).filter(|&i| score[i] > f32::MIN && biome[i] == b as u8).map(|i| score[i]);
            if shares[b] <= 0.0 { f32::MAX } else { quantile_above(values, shares[b]) }
        });
        // Udział liczymy wśród kafli zdatnych pod las (bez gór); względem całego lądu biomu wychodzi odrobinę mniej.
        for i in 0..n {
            if score[i] == f32::MIN {
                continue;
            }
            let t = blend(&thresholds, biome[i], biome_other[i], biome_mix[i]);
            if t >= f32::MAX / 2.0 {
                continue;
            }
            // Szeroka, miękka strefa skraju – renderer rozbija ją na pojedyncze drzewa.
            forest[i] = (smoothstep(t - 0.06, t + 0.06, score[i]) * 255.0) as u8;
        }
    }

    Vegetation { forest }
}
