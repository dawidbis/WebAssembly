//! Podgląd generatora bez przeglądarki:
//!   cargo run -p game-mapgen --release --features cli -- --seed 42 --out map.png [--params p.json] [--view biomes]

use std::{fs::File, io::BufWriter, time::Instant};

use game_mapgen::{generate, MapGenParams};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();

    let mut params: MapGenParams = match arg("--params") {
        Some(path) => serde_json::from_str(&std::fs::read_to_string(path).expect("params file")).expect("params json"),
        None => MapGenParams::default(),
    };
    if let Some(seed) = arg("--seed") {
        params.seed = seed.parse().expect("seed must be u32");
    }
    let out = arg("--out").unwrap_or_else(|| "map.png".into());
    let biome_view = arg("--view").is_some_and(|v| v == "biomes");

    let t0 = Instant::now();
    let map = generate(&params);
    eprintln!("wygenerowano w {:?}: {:?}", t0.elapsed(), map.stats);
    // Ten sam hash pokazuje panel debugu w przeglądarce – szybki test determinizmu native vs wasm.
    let hash = map.terrain.iter().fold(0x811C_9DC5u32, |h, &b| (h ^ b as u32).wrapping_mul(0x0100_0193));
    eprintln!("hash terenu: {hash:08x}");
    let hash = [&map.biome, &map.biome_other, &map.biome_mix]
        .into_iter()
        .flatten()
        .fold(0x811C_9DC5u32, |h, &b| (h ^ b as u32).wrapping_mul(0x0100_0193));
    eprintln!("hash biomów: {hash:08x}");

    let (w, h) = (map.width as usize, map.height as usize);
    let mut rgba = vec![0u8; w * h * 4];
    for i in 0..w * h {
        let light = hillshade(&map.shade, &map.terrain, w, h, i);
        let (a, b) = (map.biome[i] as usize, map.biome_other[i] as usize);
        let paint = |biome: usize| {
            if biome_view { biome_color(map.terrain[i], map.shade[i], biome) } else { color(map.terrain[i], map.shade[i], biome) }
        };
        // Płynne przejście: mieszanie kolorów obu biomów według udziału `biome_mix`.
        let k = map.biome_mix[i] as f32 / 256.0;
        let (ca, cb) = (paint(a), paint(b));
        let lit = if map.terrain[i] >= 3 { light } else { 1.0 };
        let c = [0, 1, 2].map(|j| ((ca[j] + (cb[j] - ca[j]) * k) * lit).clamp(0.0, 255.0) as u8);
        rgba[i * 4..i * 4 + 4].copy_from_slice(&[c[0], c[1], c[2], 255]);
    }
    let mut enc = png::Encoder::new(BufWriter::new(File::create(&out).expect("out file")), w as u32, h as u32);
    enc.set_color(png::ColorType::Rgba);
    enc.write_header().unwrap().write_image_data(&rgba).unwrap();
    eprintln!("zapisano {out}");
}

// Ta sama paleta co w web/src/app/render/terrain.ts.
struct Palette {
    plains: [[f32; 3]; 2],
    highlands: [[f32; 3]; 2],
    rock: [f32; 3],
    snow: [f32; 3],
    snow_start: f32,
    lake: [f32; 3],
    river: [f32; 3],
}

/// Kolejność jak `game_mapgen::Biome`: umiarkowany, pustynny, zimny, wilgotny, step.
const PALETTES: [Palette; 5] = [
    Palette {
        plains: [[104., 150., 72.], [150., 170., 96.]],
        highlands: [[88., 128., 64.], [110., 118., 76.]],
        rock: [128., 118., 108.],
        snow: [238., 236., 230.],
        snow_start: 0.55,
        lake: [63., 134., 184.],
        river: [74., 144., 196.],
    },
    Palette {
        plains: [[222., 196., 138.], [238., 214., 162.]],
        highlands: [[210., 162., 104.], [184., 130., 88.]],
        rock: [158., 114., 84.],
        snow: [228., 204., 172.],
        snow_start: 0.8,
        lake: [58., 150., 168.],
        river: [70., 156., 176.],
    },
    Palette {
        plains: [[222., 229., 233.], [238., 242., 245.]],
        highlands: [[206., 214., 220.], [226., 231., 235.]],
        rock: [150., 157., 166.],
        snow: [250., 251., 253.],
        snow_start: 0.25,
        lake: [148., 188., 210.],
        river: [126., 174., 206.],
    },
    Palette {
        plains: [[40., 108., 50.], [64., 130., 58.]],
        highlands: [[78., 118., 60.], [98., 112., 68.]],
        rock: [96., 106., 92.],
        snow: [214., 220., 212.],
        snow_start: 0.8,
        lake: [48., 110., 120.],
        river: [58., 122., 138.],
    },
    Palette {
        plains: [[172., 170., 100.], [190., 180., 114.]],
        highlands: [[180., 156., 104.], [158., 130., 90.]],
        rock: [140., 124., 108.],
        snow: [234., 230., 222.],
        snow_start: 0.7,
        lake: [72., 138., 168.],
        river: [80., 146., 182.],
    },
];

/// Płaskie kolory biomów do widoku „mapa biomów”.
const BIOME_FLAT: [[f32; 3]; 5] = [[106., 154., 72.], [224., 196., 138.], [216., 228., 234.], [47., 122., 60.], [184., 174., 102.]];

fn lerp(a: [f32; 3], b: [f32; 3], k: f32) -> [f32; 3] {
    [0, 1, 2].map(|c| a[c] + (b[c] - a[c]) * k)
}

fn color(t: u8, s: u8, biome: usize) -> [f32; 3] {
    let p = &PALETTES[biome.min(4)];
    let k = s as f32 / 255.0;
    match t {
        0 => lerp([47., 111., 159.], [13., 42., 74.], k.sqrt()),
        1 => p.lake,
        2 => p.river,
        3 => lerp(p.plains[0], p.plains[1], k),
        4 => lerp(p.highlands[0], p.highlands[1], k),
        _ => lerp(p.rock, p.snow, ((k - p.snow_start) / (1.0 - p.snow_start)).clamp(0.0, 1.0)),
    }
}

fn biome_color(t: u8, s: u8, biome: usize) -> [f32; 3] {
    if t >= 3 { BIOME_FLAT[biome.min(4)] } else { color(t, s, biome) }
}

fn hillshade(shade: &[u8], terrain: &[u8], w: usize, h: usize, i: usize) -> f32 {
    let (x, y) = (i % w, i / w);
    if x == 0 || y == 0 || x + 1 >= w || y + 1 >= h || terrain[i] < 3 {
        return 1.0;
    }
    let a = if terrain[i - w - 1] >= 3 { shade[i - w - 1] } else { shade[i] } as f32;
    let b = if terrain[i + w + 1] >= 3 { shade[i + w + 1] } else { shade[i] } as f32;
    (1.0 + (a - b) * 0.02).clamp(0.7, 1.3)
}
