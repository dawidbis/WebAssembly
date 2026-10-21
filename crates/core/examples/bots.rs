//! Symulacja samych botów (bez przeglądarki): gra na prawdziwej mapie z lobby, tylko boty, raport co miesiąc
//! gry – jak rosną (kafle, ludność, złoto, budynki, pokrycie prowiantu i podstawowego). Do strojenia stałych
//! w `crates/core/src/game/bots.rs`.
//!
//!   cargo run -p game-core --release --example bots -- [--seed 1] [--bots 6] [--months 12] [--size medium]
//!                                                       [--random] [--png mapa.png]
//!
//! Bez `--random` boty wybierają start jak w grze (przeskoki przez `SELECTION_TICKS` tur); `--png` – mapa
//! polityczna na koniec (kształt terytoriów).

use game_core::{
    buildings::Building,
    economy::{MILLI, TAX_TICKS},
    factories::NO_SHORTAGE,
    game::Game,
    goods::{SUPPLY_NAMES, Supply, TICKS_PER_DAY},
    lobby::{MapSettings, MapSize},
    mapgen::{GENERATOR_VERSION, Terrain},
    protocol::{GameConfig, Intent, SERVER, StampedIntent, Turn},
};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let num = |name: &str, default: u32| arg(name).map_or(default, |v| v.parse().unwrap());
    let (seed, count, months) = (num("--seed", 1), num("--bots", 6) as u16, num("--months", 12));
    let size = match arg("--size").as_deref() {
        Some("small") => MapSize::Small,
        Some("large") => MapSize::Large,
        _ => MapSize::Medium,
    };
    let map = MapSettings { size, ..Default::default() }.params(seed);
    let started = std::time::Instant::now();
    let mut game = Game::new(GameConfig { generator_version: GENERATOR_VERSION, map });
    let bots: Vec<u16> = (0..count).collect();
    let random = args.iter().any(|a| a == "--random");
    let begin = Intent::BeginSelection { players: Vec::new(), random, bots: bots.clone() };
    game.apply_turn(&Turn { tick: 0, intents: vec![StampedIntent { player: SERVER, intent: begin }] });
    eprintln!("mapa: {} prowincji, generowanie {:.1} s", game.map().provinces.len(), started.elapsed().as_secs_f64());
    let started = std::time::Instant::now();
    for month in 1..=months {
        for _ in 0..TAX_TICKS {
            let tick = game.tick();
            game.apply_turn(&Turn { tick, intents: Vec::new() });
        }
        println!("── miesiąc {month}");
        println!(
            "{:>4} {:>6} {:>8} {:>8} {:>8} {:>5} {:>6} {:>5} {:>5} {:>5} {:>5} {:>4} {:>5} {:>5} {:>12}  fabryki (!n – brak składnika n)",
            "bot",
            "kafle",
            "ludność",
            "limit",
            "złoto",
            "woj%",
            "obsada",
            "front",
            "wyd",
            "fabr",
            "miast",
            "port",
            "prow",
            "podst",
            "paliwo +/−/d"
        );
        let rates = game.rates();
        for &bot in &bots {
            let Some(n) = game.owners().nation_of(bot) else { continue };
            let (e, r) = (game.economy(n), &rates[n as usize - 1]);
            let mut levels = [0u32; 8];
            let mut products = Vec::new();
            for p in 1..=game.map().provinces.len() as u16 {
                for b in [Building::Extraction, Building::Factory, Building::City, Building::Port] {
                    if game.controller(p, b) == Some(n) {
                        levels[b as usize] += game.buildings(p).level(b) as u32;
                    }
                }
                let b = game.buildings(p);
                if b.level(Building::Factory) > 0 && game.controller(p, Building::Factory) == Some(n) {
                    let short = if b.short == NO_SHORTAGE { String::new() } else { format!("!{}", b.short) };
                    products.push(format!(
                        "{}×{}{short}",
                        &SUPPLY_NAMES[b.product as usize][..4],
                        b.level(Building::Factory)
                    ));
                }
            }
            let fronts = game.reports()[n as usize - 1].attacks.len();
            let fuel = Supply::Fuel as usize;
            let fuel = format!(
                "{}+{}−{}",
                e.supplies[fuel] / MILLI,
                e.supply_output[fuel] * TICKS_PER_DAY / MILLI,
                e.supply_use[fuel] * TICKS_PER_DAY / MILLI
            );
            println!(
                "{:>4} {:>6} {:>8} {:>8} {:>8} {:>5} {:>5}% {:>5} {:>5} {:>5} {:>5} {:>4} {:>4}% {:>4}% {:>12}  {}",
                bot,
                game.owners().area(n),
                e.population() / MILLI,
                r.cap,
                e.gold / 10,
                e.troop_share(),
                r.staff_ppm / 10_000,
                fronts,
                levels[Building::Extraction as usize],
                levels[Building::Factory as usize],
                levels[Building::City as usize],
                levels[Building::Port as usize],
                e.food_ppm / 10_000,
                e.basic_ppm / 10_000,
                fuel,
                products.join(" "),
            );
        }
    }
    let ticks = months * TAX_TICKS;
    let secs = started.elapsed().as_secs_f64();
    eprintln!("{ticks} tur w {secs:.1} s ({:.2} ms na turę)", secs * 1000.0 / ticks as f64);
    if let Some(path) = arg("--png") {
        write_png(&game, &path);
    }
}

/// Mapa polityczna do PNG: woda, góry, ziemia niczyja i państwa w kolorach (złoty kąt na kole barw).
fn write_png(game: &Game, path: &str) {
    let map = game.map();
    let (w, h) = (map.width as usize, map.height as usize);
    let tiles = game.owners().tiles();
    let mut rgba = Vec::with_capacity(w * h * 4);
    for i in 0..w * h {
        let t = map.terrain[i];
        let c = if t == Terrain::Ocean as u8 || t == Terrain::Lake as u8 || t == Terrain::River as u8 {
            [120, 160, 200]
        } else if t == Terrain::Mountains as u8 {
            [150, 140, 130]
        } else if tiles[i] == 0 {
            [200, 198, 190]
        } else {
            hue(tiles[i] as f64 * 137.5)
        };
        rgba.extend_from_slice(&[c[0], c[1], c[2], 255]);
    }
    let file = std::io::BufWriter::new(std::fs::File::create(path).expect("plik PNG"));
    let mut enc = png::Encoder::new(file, w as u32, h as u32);
    enc.set_color(png::ColorType::Rgba);
    enc.write_header().unwrap().write_image_data(&rgba).unwrap();
    eprintln!("mapa polityczna: {path}");
}

/// Nasycony kolor o odcieniu `deg` (stopnie).
fn hue(deg: f64) -> [u8; 3] {
    let h = (deg % 360.0) / 60.0;
    let x = 1.0 - (h % 2.0 - 1.0).abs();
    let (r, g, b) = match h as u32 {
        0 => (1.0, x, 0.0),
        1 => (x, 1.0, 0.0),
        2 => (0.0, 1.0, x),
        3 => (0.0, x, 1.0),
        4 => (x, 0.0, 1.0),
        _ => (1.0, 0.0, x),
    };
    [(40.0 + r * 190.0) as u8, (40.0 + g * 190.0) as u8, (40.0 + b * 190.0) as u8]
}
