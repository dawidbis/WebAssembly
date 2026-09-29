//! Podgląd generatora bez przeglądarki:
//!   cargo run -p game-mapgen --release --features cli -- --seed 42 --out map.png [--params p.json]

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

    let t0 = Instant::now();
    let map = generate(&params);
    eprintln!("wygenerowano w {:?}: {:?}", t0.elapsed(), map.stats);
    // Ten sam hash pokazuje panel debugu w przeglądarce – szybki test determinizmu native vs wasm.
    let hash = map.terrain.iter().fold(0x811C_9DC5u32, |h, &b| (h ^ b as u32).wrapping_mul(0x0100_0193));
    eprintln!("hash terenu: {hash:08x}");

    let (w, h) = (map.width as usize, map.height as usize);
    let mut rgba = vec![0u8; w * h * 4];
    for i in 0..w * h {
        let light = hillshade(&map.shade, &map.terrain, w, h, i);
        let c = color(map.terrain[i], map.shade[i], light);
        rgba[i * 4..i * 4 + 4].copy_from_slice(&[c[0], c[1], c[2], 255]);
    }
    let mut enc = png::Encoder::new(BufWriter::new(File::create(&out).expect("out file")), w as u32, h as u32);
    enc.set_color(png::ColorType::Rgba);
    enc.write_header().unwrap().write_image_data(&rgba).unwrap();
    eprintln!("zapisano {out}");
}

// Ta sama paleta co w web/src/app/render/terrain.ts.
fn color(t: u8, s: u8, light: f32) -> [u8; 3] {
    let lerp = |a: [f32; 3], b: [f32; 3], k: f32| [0, 1, 2].map(|c| a[c] + (b[c] - a[c]) * k);
    let k = s as f32 / 255.0;
    let rgb = match t {
        0 => lerp([47., 111., 159.], [13., 42., 74.], k.sqrt()),
        1 => [63., 134., 184.],
        2 => [74., 144., 196.],
        3 => lerp([104., 150., 72.], [150., 170., 96.], k),
        4 => lerp([160., 150., 98.], [140., 120., 84.], k),
        _ => lerp([128., 118., 108.], [238., 236., 230.], ((k - 0.55) / 0.45).clamp(0.0, 1.0)),
    };
    let lit = if t >= 3 { light } else { 1.0 };
    rgb.map(|c| (c * lit).clamp(0.0, 255.0) as u8)
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
