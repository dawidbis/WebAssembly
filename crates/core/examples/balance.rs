//! Symulator bilansu gospodarki (docs/gdd/balans.md): na prawdziwych mapach losuje spójne „państwa”
//! (N sąsiadujących prowincji), liczy ich wydobycie przy danym poziomie budynków wydobycia i sprawdza,
//! ile zaopatrzenia da się zrobić z tych surowców według receptur rdzenia – wobec popytu ludności
//! i armii. Stałe i receptury czyta z kodu, więc po zmianie balansu wystarczy uruchomić ponownie.
//!
//!   cargo run -p game-core --release --example balance -- [--maps 3] [--size 10] [--level 2] [--army 30] [--fighting 50]
//!
//! - `--size` – prowincji w państwie, `--level` – poziom wydobycia w każdej prowincji,
//! - `--army` – udział wojska w populacji (%), `--fighting` – część wojska w walce (%),
//! - populacja = 80% limitu (kafle × `PEOPLE_PER_TILE`).

use game_core::{
    economy::{BASIC_PEOPLE_PER_UNIT, LUXURY_PEOPLE_PER_UNIT, MILLI, PEOPLE_PER_TILE, PROVISIONS_PEOPLE_PER_UNIT},
    factories::{Input, RECIPES},
    goods::{BASE_YIELD_PER_DAY, GOLD, RESOURCE_COUNT, RESOURCE_NAMES, SUPPLIES, SUPPLY_COUNT, SUPPLY_NAMES, Supply},
    mapgen::{self, MapGenParams},
    war::SUPPLY_PER_1000_PER_DAY,
};

/// Materiały inżynieryjne na dzień (budowa – stały popyt w symulacji).
const MATERIALS_PER_DAY: f64 = 3.0;
/// Państw na mapę.
const EMPIRES_PER_MAP: usize = 200;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |name: &str, default: f64| {
        args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map_or(default, |v| v.parse().unwrap())
    };
    let (maps, size, level) = (arg("--maps", 3.0) as u32, arg("--size", 10.0) as usize, arg("--level", 2.0));
    let (army_pct, fighting_pct) = (arg("--army", 30.0), arg("--fighting", 50.0));

    // Kolejność zaspokajania (paliwo najpierw – jest składnikiem innych).
    let order = [
        Supply::Fuel,
        Supply::Provisions,
        Supply::BasicCivil,
        Supply::EngineeringMaterials,
        Supply::LuxuryCivil,
        Supply::LightMilitary,
        Supply::StandardMilitary,
        Supply::HeavyMilitary,
    ];
    let mut coverage_sum = [0.0f64; SUPPLY_COUNT];
    let mut full = [0u32; SUPPLY_COUNT];
    let mut limits = vec![[0u32; RESOURCE_COUNT]; SUPPLY_COUNT];
    let mut supply_limits = vec![[0u32; SUPPLY_COUNT]; SUPPLY_COUNT];
    let mut surplus = [0.0f64; RESOURCE_COUNT];
    let mut has = [0u32; RESOURCE_COUNT];
    let mut empires = 0u32;
    let mut people = 0.0;
    let mut rng = 0x1234_5678_u64;
    let mut next = |n: usize| {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        (rng % n as u64) as usize
    };

    for seed in 1..=maps {
        let map = mapgen::generate(&MapGenParams { seed, ..Default::default() });
        let graph = mapgen::province_graph(&map);
        let count = map.provinces.len();
        for _ in 0..EMPIRES_PER_MAP {
            // Spójne państwo: BFS z losowej prowincji (sąsiedzi także przez rzekę).
            let start = next(count);
            let mut members = vec![start];
            let mut k = 0;
            while members.len() < size && k < members.len() {
                for link in &graph[members[k]] {
                    let q = link.province as usize - 1;
                    if members.len() < size && !members.contains(&q) && map.provinces[q].tunnel == 0 {
                        members.push(q);
                    }
                }
                k += 1;
            }
            if members.len() < size {
                continue;
            }
            empires += 1;
            let area: u64 = members.iter().map(|&p| map.provinces[p].area as u64).sum();
            let population = (area * PEOPLE_PER_TILE) as f64 * 0.8;
            people += population;
            let fighting = population * army_pct / 100.0 * fighting_pct / 100.0;
            // Wydobycie na dzień (jednostki).
            let mut pool = [0.0f64; RESOURCE_COUNT];
            let mut seen = [false; RESOURCE_COUNT];
            for &p in &members {
                let r = map.provinces[p].resource as usize;
                pool[r] += BASE_YIELD_PER_DAY as f64 / MILLI as f64 + level;
                seen[r] = true;
            }
            for r in 0..RESOURCE_COUNT {
                has[r] += seen[r] as u32;
            }
            pool[GOLD as usize] = 0.0;
            let demand = |s: Supply| match s {
                Supply::Provisions => population / PROVISIONS_PEOPLE_PER_UNIT as f64,
                Supply::BasicCivil => population / BASIC_PEOPLE_PER_UNIT as f64,
                Supply::LuxuryCivil => population / LUXURY_PEOPLE_PER_UNIT as f64,
                Supply::LightMilitary => fighting / 1000.0 * SUPPLY_PER_1000_PER_DAY as f64 / MILLI as f64,
                Supply::EngineeringMaterials => MATERIALS_PER_DAY,
                // Paliwo – tyle, ile potrzeba jako składnik (liczone niżej); standardowe i ciężkie – bez popytu.
                _ => 0.0,
            };
            let mut supplies = [0.0f64; SUPPLY_COUNT];
            for &s in &order {
                let want =
                    if s == Supply::Fuel { population / BASIC_PEOPLE_PER_UNIT as f64 * 0.2 + 2.0 } else { demand(s) };
                if want <= 0.0 {
                    continue;
                }
                // Ile sztuk pozwala każdy składnik – wymienne wejścia sumują się (kilka fabryk na różnych
                // wejściach), potem zużycie od najobficiej dostępnego.
                let recipe = RECIPES[s as usize];
                let mut made = want;
                let mut limit: Option<Input> = None;
                for ingredient in recipe.iter() {
                    let can: f64 = ingredient
                        .iter()
                        .filter(|o| o.universities == 0)
                        .map(|o| {
                            let have = match o.input {
                                Input::Resource(r) => pool[r as usize],
                                Input::Supply(x) => supplies[x as usize],
                            };
                            have.max(0.0) / (o.amount as f64 / MILLI as f64)
                        })
                        .sum();
                    if can < made {
                        made = can;
                        // Brak: wejście z największą potrzebą (pierwsze w tabeli).
                        limit = ingredient.first().map(|o| o.input);
                    }
                }
                for ingredient in recipe.iter() {
                    let mut need = made;
                    let mut options: Vec<_> = ingredient.iter().filter(|o| o.universities == 0).collect();
                    let units = |o: &&game_core::factories::Alternative, pool: &[f64], supplies: &[f64]| {
                        let have = match o.input {
                            Input::Resource(r) => pool[r as usize],
                            Input::Supply(x) => supplies[x as usize],
                        };
                        have.max(0.0) / (o.amount as f64 / MILLI as f64)
                    };
                    options.sort_by(|x, y| units(y, &pool, &supplies).total_cmp(&units(x, &pool, &supplies)));
                    for o in options {
                        if need <= 0.0 {
                            break;
                        }
                        let take = need.min(units(&o, &pool, &supplies));
                        let amount = o.amount as f64 / MILLI as f64;
                        match o.input {
                            Input::Resource(r) => pool[r as usize] -= take * amount,
                            Input::Supply(x) => supplies[x as usize] -= take * amount,
                        }
                        need -= take;
                    }
                }
                supplies[s as usize] += made;
                if s != Supply::Fuel {
                    let c = (made / want).min(1.0);
                    coverage_sum[s as usize] += c;
                    full[s as usize] += (c >= 0.999) as u32;
                    match limit.filter(|_| c < 0.999) {
                        Some(Input::Resource(r)) => limits[s as usize][r as usize] += 1,
                        Some(Input::Supply(x)) => supply_limits[s as usize][x as usize] += 1,
                        None => {}
                    }
                }
            }
            for r in 0..RESOURCE_COUNT {
                surplus[r] += pool[r].max(0.0);
            }
        }
    }

    let e = empires.max(1) as f64;
    println!(
        "{empires} państw po {size} prowincji ({maps} map), wydobycie poziom {level}, wojsko {army_pct}% (w walce {fighting_pct}%), populacja śr. {:.0}\n",
        people / e
    );
    println!("Zaopatrzenie (śr. pokrycie popytu · państwa z pełnym · najczęstsze braki):");
    for &s in &order[1..] {
        let i = s as usize;
        if limits[i].iter().all(|&n| n == 0) && coverage_sum[i] == 0.0 {
            continue;
        }
        let mut top: Vec<(u32, usize)> =
            limits[i].iter().enumerate().map(|(r, &n)| (n, r)).filter(|x| x.0 > 0).collect();
        top.sort_unstable_by(|a, b| b.cmp(a));
        let mut top: Vec<String> =
            top.iter().take(4).map(|&(n, r)| format!("{} {:.0}%", RESOURCE_NAMES[r], n as f64 * 100.0 / e)).collect();
        for (x, &n) in supply_limits[i].iter().enumerate().filter(|x| *x.1 > 0) {
            top.push(format!("{} {:.0}%", SUPPLY_NAMES[x], n as f64 * 100.0 / e));
        }
        println!(
            "  {:<24} {:5.0}%  pełne {:3.0}%  braki: {}",
            SUPPLY_NAMES[i],
            coverage_sum[i] / e * 100.0,
            full[i] as f64 * 100.0 / e,
            top.join(", ")
        );
    }
    println!("\nSurowce (państwa, które je mają · śr. nadwyżka na dzień po zaspokojeniu popytu):");
    for r in 0..RESOURCE_COUNT {
        if r == GOLD as usize {
            continue;
        }
        println!("  {:<18} {:4.0}%  {:5.2}", RESOURCE_NAMES[r], has[r] as f64 * 100.0 / e, surplus[r] / e);
    }
    let _ = SUPPLIES;
}
