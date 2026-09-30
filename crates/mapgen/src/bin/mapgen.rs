//! Podgląd generatora bez przeglądarki:
//!   cargo run -p game-mapgen --release --features cli -- --seed 42 --out map.png [--params p.json] [--view biomes|fertility|political] [--borders] [--border-opacity 0.55] [--no-contours]

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
    let view = arg("--view").unwrap_or_default();
    let (biome_view, fertility_view, political_view) = (view == "biomes", view == "fertility", view == "political");
    let borders = political_view || args.iter().any(|a| a == "--borders");
    // Krycie granic na mapie terenu (jak suwak „Krycie granic” w panelu).
    let border_opacity: f32 = arg("--border-opacity").map_or(BORDER_OPACITY, |v| v.parse().expect("--border-opacity 0..1"));
    let contours = !args.iter().any(|a| a == "--no-contours");

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
    let hash = [&map.forest, &map.fertility]
        .into_iter()
        .flatten()
        .fold(0x811C_9DC5u32, |h, &b| (h ^ b as u32).wrapping_mul(0x0100_0193));
    eprintln!("hash roślinności: {hash:08x}");
    let hash = map.province.iter().flat_map(|v| v.to_le_bytes()).fold(0x811C_9DC5u32, |h, b| (h ^ b as u32).wrapping_mul(0x0100_0193));
    eprintln!("hash prowincji: {hash:08x}");

    let (w, h) = (map.width as usize, map.height as usize);
    let colors = political_colors(&map.province, map.provinces.len(), w, h);
    let mut rgba = vec![0u8; w * h * 4];
    for i in 0..w * h {
        let border = borders && province_border(&map.province, w, h, i);
        if political_view && border {
            let c = POLITICAL_BORDER;
            rgba[i * 4..i * 4 + 4].copy_from_slice(&[c[0] as u8, c[1] as u8, c[2] as u8, 255]);
            continue;
        }
        if political_view {
            let c = match map.terrain[i] {
                0 => POLITICAL_SEA,
                1 => POLITICAL_LAKE,
                _ if map.province[i] > 0 => political_color(&colors, map.province[i]),
                _ => POLITICAL_SEA,
            };
            rgba[i * 4..i * 4 + 4].copy_from_slice(&[c[0] as u8, c[1] as u8, c[2] as u8, 255]);
            continue;
        }
        if map.terrain[i] == 0 {
            let c = ocean_color(&map.terrain, &map.shade, w, h, i, contours).map(|v| v.clamp(0.0, 255.0) as u8);
            rgba[i * 4..i * 4 + 4].copy_from_slice(&[c[0], c[1], c[2], 255]);
            continue;
        }
        let light = hillshade(&map.shade, &map.terrain, w, h, i);
        let (a, b) = (map.biome[i] as usize, map.biome_other[i] as usize);
        let (t, forest) = (map.terrain[i], map.forest[i] as f32 / 255.0);
        let paint = |biome: usize| {
            if fertility_view && t >= 3 {
                fertility_color(map.fertility[i])
            } else if biome_view {
                biome_color(t, map.shade[i], biome, forest)
            } else {
                with_forest(color(t, map.shade[i], biome), biome, forest, grain(i % w, i / w))
            }
        };
        // Płynne przejście: mieszanie kolorów obu biomów według udziału `biome_mix`.
        let k = map.biome_mix[i] as f32 / 256.0;
        let (ca, cb) = (paint(a), paint(b));
        let lit = if map.terrain[i] >= 3 { light } else { 1.0 };
        let mut c = [0, 1, 2].map(|j| (ca[j] + (cb[j] - ca[j]) * k) * lit);
        if border {
            // Półprzezroczysta granica – teren pod nią pozostaje widoczny.
            c = lerp(c, BORDER, border_opacity);
        }
        let c = c.map(|v| v.clamp(0.0, 255.0) as u8);
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
        highlands: [[176., 190., 204.], [196., 208., 220.]],
        rock: [104., 112., 124.],
        snow: [250., 251., 253.],
        snow_start: 0.45,
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
        0 => ocean_depth_color(k),
        1 => p.lake,
        2 => p.river,
        3 => lerp(p.plains[0], p.plains[1], k),
        4 => lerp(p.highlands[0], p.highlands[1], k),
        _ => lerp(p.rock, p.snow, ((k - p.snow_start) / (1.0 - p.snow_start)).clamp(0.0, 1.0)),
    }
}

fn biome_color(t: u8, s: u8, biome: usize, forest: f32) -> [f32; 3] {
    // Las na płaskiej mapie biomów: ten sam kolor, tylko ciemniejszy.
    if t >= 3 { BIOME_FLAT[biome.min(4)].map(|c| c * (1.0 - 0.25 * forest)) } else { color(t, s, biome) }
}

/// Kolory koron drzew: liściasty, oazy (palmy), tajga, dżungla, zagajniki stepowe – w kolejności `Biome`.
const CANOPY: [[f32; 3]; 5] = [[52., 98., 44.], [58., 112., 52.], [62., 90., 80.], [22., 78., 34.], [82., 112., 54.]];

fn tile_hash(x: u32, y: u32) -> f32 {
    let h = x.wrapping_mul(0x9E37_79B1) ^ y.wrapping_mul(0x85EB_CA77);
    let h = (h ^ (h >> 15)).wrapping_mul(0x2C1B_3C6D);
    (h ^ (h >> 12)) as f32 / u32::MAX as f32
}

/// Ziarno koron drzew: (jasność, los kafla na skraju lasu, los śniegu na koronie) – jak w render/terrain.ts.
fn grain(x: usize, y: usize) -> (f32, f32, f32) {
    let (x, y) = (x as u32, y as u32);
    let light = 0.72 + 0.34 * tile_hash(x, y) + 0.2 * tile_hash(x >> 1, y >> 1);
    (light, tile_hash(x.wrapping_add(17), y.wrapping_add(31)), tile_hash(x.wrapping_add(53), y.wrapping_add(97)))
}

/// Ile kafli koron jest przyprószonych śniegiem, w kolejności `Biome` (tylko tajga).
const CANOPY_SNOW: [f32; 5] = [0.0, 0.0, 0.42, 0.0, 0.0];
const CANOPY_SNOW_COLOR: [f32; 3] = [226., 234., 240.];

/// Nakłada korony drzew na kolor gruntu według gęstości lasu. Na skraju (gęstość < 1) las
/// rozpada się na pojedyncze drzewa: kafel jest zadrzewiony, gdy jego los < gęstość.
fn with_forest(ground: [f32; 3], biome: usize, forest: f32, (light, roll, snow_roll): (f32, f32, f32)) -> [f32; 3] {
    if forest <= 0.0 {
        return ground;
    }
    let b = biome.min(4);
    let mut canopy = CANOPY[b].map(|c| c * light);
    // Tajga przyprószona śniegiem: lekko rozjaśniona, a część koron z białą plamką.
    if CANOPY_SNOW[b] > 0.0 {
        canopy = lerp(canopy, CANOPY_SNOW_COLOR, 0.15);
        if snow_roll < CANOPY_SNOW[b] {
            canopy = lerp(canopy, CANOPY_SNOW_COLOR, 0.6);
        }
    }
    let cover = if roll < forest { 0.92 } else { forest * 0.25 };
    lerp(ground, canopy, cover)
}

/// Widok żyzności: od jałowego brązu do soczystej zieleni.
fn fertility_color(f: u8) -> [f32; 3] {
    let k = f as f32 / 255.0;
    if k < 0.5 { lerp([120., 96., 70.], [196., 180., 96.], k * 2.0) } else { lerp([196., 180., 96.], [60., 150., 50.], k * 2.0 - 1.0) }
}

/// Kolory oceanu według głębokości (0..1): jasny szelf, wyraźny stok, ciemna głębia.
const OCEAN_STOPS: [(f32, [f32; 3]); 5] = [
    (0.0, [92., 176., 200.]),
    (0.14, [62., 142., 182.]),
    (0.3, [34., 92., 142.]),
    (0.72, [16., 52., 94.]),
    (1.0, [8., 27., 56.]),
];
/// Poziomy izobat (głębokość 0..255): krawędź szelfu, stok, głębia.
const CONTOUR_LEVELS: [u8; 5] = [30, 70, 120, 175, 225];

fn ocean_depth_color(k: f32) -> [f32; 3] {
    for pair in OCEAN_STOPS.windows(2) {
        let ((k0, c0), (k1, c1)) = (pair[0], pair[1]);
        if k <= k1 {
            return lerp(c0, c1, ((k - k0) / (k1 - k0)).clamp(0.0, 1.0));
        }
    }
    OCEAN_STOPS[OCEAN_STOPS.len() - 1].1
}

/// Ocean: kolor głębokości, cieniowanie dna, jasna linia brzegu i izobaty.
fn ocean_color(terrain: &[u8], shade: &[u8], w: usize, h: usize, i: usize, contours: bool) -> [f32; 3] {
    let (x, y) = (i % w, i / w);
    let mut c = ocean_depth_color(shade[i] as f32 / 255.0);
    if x == 0 || y == 0 || x + 1 >= w || y + 1 >= h {
        return c;
    }
    let depth = |j: usize| if terrain[j] == 0 { shade[j] as f32 } else { 0.0 };
    // Linia brzegu: kafel oceanu stykający się z lądem.
    if [i - 1, i + 1, i - w, i + w].iter().any(|&j| terrain[j] != 0) {
        return lerp(c, [196., 230., 236.], 0.55);
    }
    // Dno oświetlone jak ląd (wysokość = -głębokość), słabiej.
    let light = (1.0 + (depth(i - w - 1) - depth(i + w + 1)) * 0.012).clamp(0.85, 1.15);
    c = c.map(|v| v * light);
    if contours {
        let band = |j: usize| CONTOUR_LEVELS.iter().filter(|&&l| shade[j] >= l).count();
        let edge = [i + 1, i + w].iter().any(|&j| terrain[j] == 0 && band(j) != band(i));
        if edge {
            c = lerp(c, [200., 225., 240.], 0.22);
        }
    }
    c
}

/// Cieniowanie rzeźby: światło z lewego górnego rogu (jak `hillshade` w render/terrain.ts).
fn hillshade(shade: &[u8], terrain: &[u8], w: usize, h: usize, i: usize) -> f32 {
    let (x, y) = (i % w, i / w);
    if x == 0 || y == 0 || x + 1 >= w || y + 1 >= h || terrain[i] < 3 {
        return 1.0;
    }
    let a = if terrain[i - w - 1] >= 3 { shade[i - w - 1] } else { shade[i] } as f32;
    let b = if terrain[i + w + 1] >= 3 { shade[i + w + 1] } else { shade[i] } as f32;
    (1.0 + (b - a) * 0.02).clamp(0.7, 1.3)
}

/// Granica prowincji (jak `provinceBorder` w render/provinces.ts): kafel, którego prawy albo
/// dolny sąsiad należy do innej prowincji. Linia ma grubość jednego kafla i leży po stronie
/// lewej/górnej prowincji. Brzeg morza i jezior nie jest granicą.
fn province_border(province: &[u16], w: usize, h: usize, i: usize) -> bool {
    let (x, y) = (i % w, i / w);
    let p = province[i];
    let differs = |j: usize| province[j] != 0 && province[j] != p;
    p != 0 && ((x + 1 < w && differs(i + 1)) || (y + 1 < h && differs(i + w)))
}

/// Kolor granicy prowincji na mapie terenu i na mapie politycznej.
const BORDER: [f32; 3] = [44., 44., 48.];
/// Domyślne krycie granicy na mapie terenu (jak `borderOpacity` w web/src/app/game/map-store.ts).
const BORDER_OPACITY: f32 = 0.3;
const POLITICAL_BORDER: [f32; 3] = [150., 24., 24.];
const POLITICAL_SEA: [f32; 3] = [128., 166., 200.];
const POLITICAL_LAKE: [f32; 3] = [118., 158., 196.];
/// Kolory mapy politycznej: sąsiednie prowincje zawsze w różnych kolorach (jak `POLITICAL` w render/provinces.ts).
const POLITICAL: [[f32; 3]; 8] = [
    [226., 200., 150.],
    [186., 212., 156.],
    [216., 172., 172.],
    [200., 186., 228.],
    [228., 218., 150.],
    [234., 184., 204.],
    [238., 188., 140.],
    [172., 212., 200.],
];

fn political_color(colors: &[u8], id: u16) -> [f32; 3] {
    let k = colors[id as usize - 1] as usize;
    // Lekkie zróżnicowanie jasności w obrębie jednego koloru.
    let light = 0.94 + 0.1 * tile_hash(id as u32, 7);
    POLITICAL[k % POLITICAL.len()].map(|c| (c * light).min(255.0))
}

/// Zachłanne kolorowanie grafu sąsiedztwa prowincji (jak `politicalColors` w render/provinces.ts):
/// prowincje po kolei dostają pierwszy kolor (od przesuniętego o hash numeru) niezajęty przez sąsiadów.
fn political_colors(province: &[u16], count: usize, w: usize, h: usize) -> Vec<u8> {
    let mut adj: Vec<Vec<u16>> = vec![Vec::new(); count];
    for i in 0..w * h {
        let p = province[i];
        if p == 0 {
            continue;
        }
        for j in [(i % w + 1 < w).then(|| i + 1), (i / w + 1 < h).then(|| i + w)].into_iter().flatten() {
            let q = province[j];
            if q != 0 && q != p && !adj[p as usize - 1].contains(&q) {
                adj[p as usize - 1].push(q);
                adj[q as usize - 1].push(p);
            }
        }
    }
    let mut colors = vec![u8::MAX; count];
    let k = POLITICAL.len() as u32;
    for p in 0..count {
        let start = (tile_hash(p as u32 + 1, 3) * k as f32) as u32;
        let used = |c: u8| adj[p].iter().any(|&q| colors[q as usize - 1] == c);
        colors[p] = (0..k).map(|o| ((start + o) % k) as u8).find(|&c| !used(c)).unwrap_or((start % k) as u8);
    }
    colors
}
