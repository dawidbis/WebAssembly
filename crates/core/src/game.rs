//! Stan gry i pętla ticków: mapa (niezmienna), licznik ticków, właściciele kafli, gospodarka,
//! budynki, hash stanu. Tu wejdą kolejne egzekucje intencji (walka, dyplomacja, boty).

use std::collections::BTreeMap;

use crate::{
    buildings::{
        self, BUILDINGS, BuildBlock, BuildOption, Building, CANCEL_REFUND_PCT, CITY_POPULATION, Construction,
        ConstructionReport, EXTRACTION_PER_LEVEL_PER_DAY, FERTILIZER_BONUS_PCT, FERTILIZER_PER_LEVEL_PER_DAY,
        FORTIFICATION_CAP, FactoryReport, MAX_LEVEL, POINTS_PER_LEVEL, ProductOption, ProvinceBuildings,
        ProvinceReport, STAFF_PER_LEVEL,
    },
    economy::{Economy, MILLI, NationRates, NationReport, PEOPLE_PER_TILE, TAX_TICKS},
    factories::{self, COMMAND_LEVEL, NO_SHORTAGE, OUTPUT_PER_LEVEL_PER_TICK, RECIPES, RETOOL_TICKS},
    goods::{
        BASE_YIELD_PER_DAY, EXTRACTION, Extraction, FERTILIZER, GOLD, GOLD_PER_UNIT, RESOURCE_COUNT, SUPPLIES,
        SUPPLY_COUNT, SUPPLY_NAMES, Supply, TICKS_PER_DAY,
    },
    mapgen::{self, MapData, ProvinceLink, Terrain},
    owners::Owners,
    protocol::{Catchup, GameConfig, Intent, PlayerId, SELECTION_TICKS, SERVER, Turn},
    war::{
        self, Attack, AttackBlock, AttackInfo, AttackReport, ENEMY_TILE_POINTS, FUEL_PER_1000_PER_DAY, FUEL_SPEED_PCT,
        FrontReport, FrontSide, HORSE_FUEL_PCT, HORSE_MAX_PCT, MAX_ATTACKS, NEUTRAL_TILE_POINTS, NO_SUPPLY_PCT,
        POINTS_PER_TICK, SUPPLY_PCT, SUPPLY_PER_1000_PER_DAY,
    },
};

/// Konie – zamiennik paliwa w ataku (numer surowca).
const HORSES: u8 = 11;

pub struct Game {
    config: GameConfig,
    /// Mapa jest niezmienna – zmienny stan gry (właściciele, wykarczowany las…) trzymaj w osobnych
    /// polach inicjalizowanych w `from_map`, bo `restart` odtwarza grę z tej samej mapy.
    map: MapData,
    /// FNV-1a mapy liczony raz – `state_hash` go kontynuuje, więc nie przechodzi co turę przez całą mapę.
    map_hash: u32,
    /// Graf sąsiedztwa prowincji (`mapgen::province_graph`) – stały dla mapy, więc poza hashem stanu.
    graph: Vec<Vec<ProvinceLink>>,
    /// Prostokąt prowincji `id` (`bounds[id - 1]`: x0, y0, x1, y1 włącznie) – do przechodzenia po
    /// jej kaflach bez przeglądania całej mapy.
    bounds: Vec<[u32; 4]>,
    tick: u32,
    /// Właściciele kafli (stan gry – w hashu).
    owners: Owners,
    phase: Phase,
    /// Uczestnicy gry (z `Intent::BeginSelection`, rosnąco) – każdy dostaje prowincję startową.
    players: Vec<PlayerId>,
    /// Prowincja startowa gracza (wybrana albo wylosowana).
    starts: BTreeMap<PlayerId, u16>,
    /// Gospodarka państw: `economy[n - 1]` dla państwa `n` (populacja, złoto, suwak).
    economy: Vec<Economy>,
    /// Tick początku gry właściwej (od niego liczą się podatki).
    playing_since: u32,
    /// Suwak podziału populacji ustawiony przed wybraniem prowincji startowej (gracz → udział wojska %).
    pending_share: BTreeMap<PlayerId, u8>,
    /// Budynki prowincji (`buildings[id - 1]`): poziomy i trwająca budowa.
    buildings: Vec<ProvinceBuildings>,
    /// Trwające ataki w kolejności rozpoczęcia (w tej kolejności rozliczane co turę).
    attacks: Vec<Attack>,
}

/// Części na milion – pokrycia (obsada, nawozy).
const PPM: u64 = 1_000_000;

/// Pełna produkcja fabryki na turę (w tysięcznych): poziom × obsada × premia uniwersytetów.
fn factory_target(level: u64, r: &NationRates) -> u64 {
    level * OUTPUT_PER_LEVEL_PER_TICK * r.staff_ppm / PPM * (100 + factories::university_bonus(r.universities)) / 100
}

/// Etap gry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Czekanie, aż wszyscy gracze będą mieli gotową mapę (serwer wyśle `Intent::BeginSelection`).
    Waiting,
    /// Wybór prowincji startowych do ticka `end` (wyłącznie; pierwszy tick już po wyborze).
    Selection { end: u32 },
    /// Gra właściwa – każdy uczestnik ma prowincję startową.
    Playing,
}

/// Ziarno losowania startów (XOR z seedem mapy i tickiem) – osobne od strumieni generatora.
const START_SALT: u64 = 0x5747_4152_5453_0001;
/// Ilu najdalszych kandydatów bierze udział w losowaniu kolejnego startu.
const START_SPREAD: usize = 6;
/// Prowincja startowa ma wielkość w tym zakresie średniej (procent) – balans startów.
const START_AREA_PCT: (u64, u64) = (85, 115);

/// SplitMix64 – deterministyczne losowanie startów (natywnie i w wasm to samo).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// Prostokąty prowincji z kafli mapy.
fn province_bounds(map: &MapData) -> Vec<[u32; 4]> {
    let mut bounds = vec![[u32::MAX, u32::MAX, 0, 0]; map.provinces.len()];
    let w = map.width as usize;
    for (i, &p) in map.province.iter().enumerate() {
        if p == 0 {
            continue;
        }
        let (x, y) = ((i % w) as u32, (i / w) as u32);
        let b = &mut bounds[p as usize - 1];
        *b = [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)];
    }
    bounds
}

/// FNV-1a: `Fnv::default()` zaczyna od zera, `Fnv(h)` kontynuuje hash `h` (jak dopisanie bajtów).
struct Fnv(u32);

impl Default for Fnv {
    fn default() -> Self {
        Self(0x811C_9DC5)
    }
}

impl Fnv {
    fn eat(&mut self, bytes: impl IntoIterator<Item = u8>) {
        for b in bytes {
            self.0 = (self.0 ^ b as u32).wrapping_mul(0x0100_0193);
        }
    }
}

impl Game {
    /// Generuje mapę z konfiguracji i zaczyna grę (testy, narzędzia, przyszła weryfikacja replayów).
    /// Przeglądarka ma już mapę z generowania na ekran, więc używa `from_map` – bez generowania drugi raz.
    pub fn new(config: GameConfig) -> Self {
        let map = mapgen::generate(&config.map);
        Self::from_map(config, map)
    }

    /// Gra na gotowej mapie. `map` musi być wynikiem `mapgen::generate(&config.map)` (albo trzyfazowo
    /// `generate_base` + `generate_provinces` + `polish` – wynik jest ten sam), inaczej stan rozjedzie się
    /// z innymi graczami.
    pub fn from_map(config: GameConfig, map: MapData) -> Self {
        let mut h = Fnv::default();
        h.eat(map.terrain.iter().copied());
        h.eat(map.biome.iter().copied());
        h.eat(map.forest.iter().map(|&f| (f >= 128) as u8));
        h.eat(map.province.iter().flat_map(|p| p.to_le_bytes()));
        h.eat(map.provinces.iter().map(|p| p.resource));
        let (graph, bounds, owners) = (mapgen::province_graph(&map), province_bounds(&map), Owners::new(&map));
        // Startowa infrastruktura z cech prowincji (wyżyny – liczba kafli wyżyn w prowincji).
        let mut highlands = vec![0u32; map.provinces.len()];
        for (&p, &t) in map.province.iter().zip(&map.terrain) {
            if p != 0 && t == Terrain::Highlands as u8 {
                highlands[p as usize - 1] += 1;
            }
        }
        let buildings = map
            .provinces
            .iter()
            .zip(&highlands)
            .map(|(p, &h)| ProvinceBuildings::new(buildings::start_infrastructure(p, h)))
            .collect();
        Self {
            config,
            map,
            map_hash: h.0,
            graph,
            bounds,
            tick: 0,
            owners,
            phase: Phase::Waiting,
            players: Vec::new(),
            starts: BTreeMap::new(),
            economy: Vec::new(),
            playing_since: 0,
            pending_share: BTreeMap::new(),
            buildings,
            attacks: Vec::new(),
        }
    }

    /// Zaczyna grę od nowa na tej samej mapie (np. po ponownym połączeniu z serwerem).
    pub fn restart(&mut self) {
        let map = std::mem::take(&mut self.map);
        *self = Self::from_map(self.config.clone(), map);
    }

    pub fn config(&self) -> &GameConfig {
        &self.config
    }

    pub fn map(&self) -> &MapData {
        &self.map
    }

    pub fn tick(&self) -> u32 {
        self.tick
    }

    pub fn owners(&self) -> &Owners {
        &self.owners
    }

    /// Sąsiedzi prowincji (numer od 1) po lądzie, rosnąco po numerze.
    pub fn neighbours(&self, province: u16) -> &[ProvinceLink] {
        &self.graph[province as usize - 1]
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// Gospodarka państwa `n` (od 1).
    pub fn economy(&self, nation: u8) -> &Economy {
        &self.economy[nation as usize - 1]
    }

    /// Gospodarka wszystkich państw dla interfejsu (`[n - 1]` – państwo `n`).
    pub fn reports(&self) -> Vec<NationReport> {
        let players = self.owners.nations();
        let rates = self.rates();
        self.economy
            .iter()
            .enumerate()
            .map(|(k, e)| {
                let attacks = self
                    .attacks
                    .iter()
                    .filter(|a| a.nation as usize == k + 1)
                    .map(|a| AttackInfo { province: a.province, troops: (a.troops / MILLI) as f64, taken: a.taken })
                    .collect();
                NationReport::new(players[k], e, &rates[k], attacks)
            })
            .collect()
    }

    /// Budynki prowincji `province` (od 1).
    pub fn buildings(&self, province: u16) -> &ProvinceBuildings {
        &self.buildings[province as usize - 1]
    }

    /// Tick początku gry właściwej (`None` przed nią) – od niego liczy się podatek i kalendarz na ekranie.
    pub fn playing_since(&self) -> Option<u32> {
        (self.phase == Phase::Playing).then_some(self.playing_since)
    }

    /// Tick najbliższego podatku (`None` przed grą właściwą) – do odliczania na ekranie.
    pub fn next_tax_tick(&self) -> Option<u32> {
        (self.phase == Phase::Playing).then(|| {
            let since = self.tick - self.playing_since;
            self.tick + (TAX_TICKS - since % TAX_TICKS)
        })
    }

    /// Prowincje startowe graczy (wybrane albo wylosowane).
    pub fn starts(&self) -> &BTreeMap<PlayerId, u16> {
        &self.starts
    }

    /// Wykonuje jedną turę z serwera. Tury muszą przychodzić po kolei. Intencje w kolejności
    /// z serwera; niepoprawna (np. wybór cudzej prowincji) przepada bez śladu w stanie gry.
    /// Koniec czasu wyboru (`Phase::Selection { end }`) wypada po turze `end - 1` – u każdego
    /// gracza w tym samym ticku, bez zegara.
    pub fn apply_turn(&mut self, turn: &Turn) {
        assert_eq!(turn.tick, self.tick, "tura spoza kolejki");
        for stamped in &turn.intents {
            self.execute(stamped.player, &stamped.intent);
        }
        self.tick += 1;
        if let Phase::Selection { end } = self.phase
            && self.tick >= end
        {
            self.finish_selection();
        } else if self.phase == Phase::Playing {
            self.economy_tick();
        }
    }

    /// Tura gospodarki (tylko w grze właściwej): przyrost populacji, suwak wojsko/robotnicy, co
    /// `TAX_TICKS` tur – podatek. Ten sam tick u każdego gracza.
    ///
    /// Wielkości państw (limit populacji, obsada, nawozy, wydobycie) liczone na początku tury z tego samego
    /// stanu – wydobycie na dzień dzielone przez `TICKS_PER_DAY` trafia do puli co turę, więc raport
    /// pokazuje dokładnie to, co dostaje pula. Złoto z kopalni – od razu do skarbca. Potem fabryki
    /// (z surowców już po wydobyciu tej tury) i budowy.
    fn economy_tick(&mut self) {
        let since = self.tick - self.playing_since;
        let tax = since > 0 && since.is_multiple_of(TAX_TICKS);
        let rates = self.rates();
        for (e, r) in self.economy.iter_mut().zip(&rates) {
            e.tick(r.cap, tax);
            e.resource_use = [0; RESOURCE_COUNT];
            e.supply_output = [0; SUPPLY_COUNT];
            e.supply_use = [0; SUPPLY_COUNT];
            // Nawozy zużywane w proporcji pokrycia (pokrycie liczone z zapasu przed turą).
            let stock = &mut e.stock[FERTILIZER as usize];
            let used = (r.fertilizer_need / TICKS_PER_DAY * r.fertilizer_ppm / PPM).min(*stock);
            *stock -= used;
            e.resource_use[FERTILIZER as usize] += used;
            for (res, &per_day) in r.production.iter().enumerate() {
                if res == GOLD as usize {
                    e.gold += per_day / TICKS_PER_DAY;
                } else {
                    e.stock[res] += per_day / TICKS_PER_DAY;
                }
            }
        }
        self.factory_tick(&rates);
        self.construction_tick();
        self.war_tick();
    }

    /// Tura ataków w kolejności rozpoczęcia; zakończone (prowincja zajęta, brak styku, koniec wojska)
    /// oddają resztę wojska do kraju.
    fn war_tick(&mut self) {
        let mut i = 0;
        while i < self.attacks.len() {
            if self.attack_tick(i) {
                let a = self.attacks.remove(i);
                let e = &mut self.economy[a.nation as usize - 1];
                e.deployed -= a.troops;
                e.troops += a.troops;
            } else {
                i += 1;
            }
        }
    }

    /// Tura jednego ataku: paliwo (szybkość), zaopatrzenie wojskowe (gdy front styka się z graczem),
    /// potem zajmowanie kafli frontu od najłatwiejszych, póki starcza punktów szybkości i wojska.
    /// Kolejność (`war::front_score`): teren (niziny bez lasu najpierw, wyżyny z lasem na końcu),
    /// otoczenie naszymi kaflami (kieszenie znikają od razu, front jest spójny) i „zawirowanie
    /// taktyczne” – losowy składnik, który daje szansę każdemu kafelkowi. `true` – atak się kończy.
    fn attack_tick(&mut self, i: usize) -> bool {
        let Attack { nation: n, province: p, troops, .. } = self.attacks[i];
        let mut front = self.frontier(n, p);
        if front.is_empty() {
            return true;
        }
        let tiles = self.owners.tiles();
        let fights = front.iter().any(|&(t, _)| tiles[t] != 0);
        let fuel_ppm = self.burn_fuel(n, troops);
        let (attacker_pct, supply) = if fights { self.burn_supplies(n, troops) } else { (100, war::NOT_FIGHTING) };
        self.attacks[i].supply = supply;
        let mut points = POINTS_PER_TICK * (100 + FUEL_SPEED_PCT * fuel_ppm / PPM) / 100;
        let tick = self.tick;
        let mut scored: Vec<(i64, usize)> = front
            .drain(..)
            .map(|(t, around)| (war::front_score(self.terrain_pct(t), around, war::front_key(t, tick)), t))
            .collect();
        scored.sort_unstable();
        for (_, t) in scored {
            let owner = self.owners.tiles()[t];
            let terrain = self.terrain_pct(t);
            let need = if owner == 0 { NEUTRAL_TILE_POINTS } else { ENEMY_TILE_POINTS } * terrain / 100;
            if points < need {
                break;
            }
            let (cost, killed) = if owner == 0 {
                (war::NEUTRAL_TILE_COST * MILLI * terrain / 100, 0)
            } else {
                let density = self.density(owner);
                (war::enemy_cost(density, self.defense_pct(owner), attacker_pct) * terrain / 100, density)
            };
            if self.attacks[i].troops < cost {
                return true;
            }
            points -= need;
            self.attacks[i].troops -= cost;
            self.attacks[i].taken += 1;
            self.economy[n as usize - 1].deployed -= cost;
            if owner != 0 {
                let d = &mut self.economy[owner as usize - 1];
                d.troops -= killed.min(d.troops);
            }
            self.owners.set(t, p, n);
        }
        // Cała prowincja nasza – koniec.
        self.owners.shares(p) == [(n, self.map.provinces[p as usize - 1].area)]
    }

    /// Trudność terenu kafla (%): niziny 100, las +25, wyżyny +50, wyżyny z lasem 190 – mnoży koszt
    /// zajęcia, punkty szybkości i ustala kolejność frontu.
    fn terrain_pct(&self, tile: usize) -> u64 {
        let forest = self.map.forest[tile] >= 128;
        match (self.map.terrain[tile] == Terrain::Highlands as u8, forest) {
            (false, false) => 100,
            (false, true) => 125,
            (true, false) => 150,
            (true, true) => 190,
        }
    }

    /// Front: kafle prowincji `province` nie należące do państwa `nation`, stykające się bokiem z jego
    /// kaflami (od centrum dowodzenia 2 także w poprzek wąskiej rzeki – `war::RIVER_CROSSING_TILES`),
    /// z liczbą naszych kafli w otoczeniu (8 sąsiadów, przeprawa liczy się za 1).
    fn frontier(&self, nation: u8, province: u16) -> Vec<(usize, u32)> {
        let [x0, y0, x1, y1] = self.bounds[province as usize - 1];
        let (w, h) = (self.map.width as i64, self.map.height as i64);
        let tiles = self.owners.tiles();
        let ours = |x: i64, y: i64| x >= 0 && y >= 0 && x < w && y < h && tiles[(y * w + x) as usize] == nation;
        let crossing = self.command_level(nation) >= war::RIVER_CROSSING_LEVEL;
        // Nasz kafel za rzeką: w linii prostej przez same kafle rzeki (najwyżej `RIVER_CROSSING_TILES`).
        let river = |x: i64, y: i64| {
            x >= 0 && y >= 0 && x < w && y < h && self.map.terrain[(y * w + x) as usize] == Terrain::River as u8
        };
        let across = |x: i64, y: i64, dx: i64, dy: i64| {
            let (mut cx, mut cy) = (x + dx, y + dy);
            for _ in 0..war::RIVER_CROSSING_TILES {
                if !river(cx, cy) {
                    return false;
                }
                (cx, cy) = (cx + dx, cy + dy);
                if ours(cx, cy) {
                    return true;
                }
            }
            false
        };
        let mut out = Vec::new();
        for y in y0 as i64..=y1 as i64 {
            for x in x0 as i64..=x1 as i64 {
                let i = (y * w + x) as usize;
                if self.map.province[i] != province || tiles[i] == nation {
                    continue;
                }
                let sides = [(1, 0), (-1, 0), (0, 1), (0, -1)];
                let side = sides.iter().any(|&(dx, dy)| ours(x + dx, y + dy));
                let bridged = !side && crossing && sides.iter().any(|&(dx, dy)| across(x, y, dx, dy));
                if !side && !bridged {
                    continue;
                }
                let around = (-1..=1)
                    .flat_map(|dy| (-1..=1).map(move |dx| (dx, dy)))
                    .filter(|&(dx, dy)| (dx, dy) != (0, 0) && ours(x + dx, y + dy))
                    .count() as u32;
                out.push((i, around.max(bridged as u32)));
            }
        }
        out
    }

    /// Gęstość wojska obrońcy (tysięczne osoby na kafel): wojsko w kraju / kafle państwa.
    fn density(&self, nation: u8) -> u64 {
        self.economy[nation as usize - 1].troops / (self.owners.area(nation) as u64).max(1)
    }

    /// Współczynnik siły obrońcy (%): najlepsze odblokowane zaopatrzenie wojskowe w jego puli (obrona go
    /// nie zużywa), bez zaopatrzenia – `NO_SUPPLY_PCT`.
    fn defense_pct(&self, nation: u8) -> u64 {
        self.defense_supply(nation).map_or(NO_SUPPLY_PCT, |(_, pct)| pct)
    }

    /// Najlepsze odblokowane zaopatrzenie wojskowe w puli państwa i jego współczynnik siły (%).
    fn defense_supply(&self, nation: u8) -> Option<(Supply, u64)> {
        let e = &self.economy[nation as usize - 1];
        let command = self.command_level(nation);
        SUPPLY_PCT.iter().copied().find(|&(s, _)| e.supplies[s as usize] > 0 && command >= COMMAND_LEVEL[s as usize])
    }

    /// Zmiany właścicieli kafli od poprzedniego wywołania (`kafel << 8 | państwo`) – dla ekranu.
    pub fn take_owner_changes(&mut self) -> Vec<u32> {
        self.owners.take_changes()
    }

    /// Aktywne fronty gracza `viewer`: jego ataki i ataki na prowincje, w których ma kafle.
    pub fn fronts(&self, viewer: PlayerId) -> Vec<FrontReport> {
        let Some(v) = self.owners.nation_of(viewer) else { return Vec::new() };
        let player = |n: u8| self.owners.nations()[n as usize - 1];
        self.attacks
            .iter()
            .filter(|a| a.nation == v || self.owners.shares(a.province).iter().any(|s| s.0 == v))
            .map(|a| {
                let shares = self.owners.shares(a.province);
                let pr = &self.map.provinces[a.province as usize - 1];
                let tiles_of = |n: u8| shares.iter().find(|s| s.0 == n).map_or(0, |s| s.1);
                // Środek frontu: średnia kafli frontu (bez frontu – środek prowincji).
                let front = self.frontier(a.nation, a.province);
                let w = self.map.width as usize;
                let (cx, cy) = if front.is_empty() {
                    (pr.center_x as f64, pr.center_y as f64)
                } else {
                    let (sx, sy) = front.iter().fold((0usize, 0usize), |(sx, sy), &(t, _)| (sx + t % w, sy + t / w));
                    (sx as f64 / front.len() as f64, sy as f64 / front.len() as f64)
                };
                FrontReport {
                    province: a.province,
                    attacker: FrontSide {
                        player: player(a.nation),
                        troops: (a.troops / MILLI) as f64,
                        supply: SUPPLIES.get(a.supply as usize).copied(),
                        tiles: tiles_of(a.nation),
                    },
                    defenders: shares
                        .iter()
                        .filter(|s| s.0 != a.nation)
                        .map(|&(n, tiles)| FrontSide {
                            player: player(n),
                            troops: (self.economy(n).troops / MILLI) as f64,
                            supply: self.defense_supply(n).map(|(s, _)| s),
                            tiles,
                        })
                        .collect(),
                    occupation: tiles_of(a.nation) as f64 / pr.area as f64,
                    neutral_tiles: pr.area - shares.iter().map(|s| s.1).sum::<u32>(),
                    center_x: cx,
                    center_y: cy,
                    mine: a.nation == v,
                }
            })
            .collect()
    }

    /// Zużycie zaopatrzenia wojskowego przez `troops` w ataku (najlepsze odblokowane najpierw); wynik –
    /// współczynnik siły (%) jako średnia ważona pokryciem, reszta bez zaopatrzenia, i rodzaj, którego
    /// zużyto najwięcej (`war::NO_SUPPLY` – żadnego).
    fn burn_supplies(&mut self, nation: u8, troops: u64) -> (u64, u8) {
        let need = troops * SUPPLY_PER_1000_PER_DAY / (MILLI * MILLI * TICKS_PER_DAY);
        if need == 0 {
            return (NO_SUPPLY_PCT, war::NO_SUPPLY);
        }
        let command = self.command_level(nation);
        let e = &mut self.economy[nation as usize - 1];
        let (mut left, mut weighted, mut main) = (need, 0, (0, war::NO_SUPPLY));
        for &(s, pct) in &SUPPLY_PCT {
            if command < COMMAND_LEVEL[s as usize] {
                continue;
            }
            let take = e.supplies[s as usize].min(left);
            e.supplies[s as usize] -= take;
            e.supply_use[s as usize] += take;
            weighted += take * pct;
            left -= take;
            if take > main.0 {
                main = (take, s as u8);
            }
        }
        ((weighted + left * NO_SUPPLY_PCT) / need, main.1)
    }

    /// Zużycie paliwa przez `troops` w ataku (konie – zamiennik do `HORSE_MAX_PCT`% potrzeby); wynik –
    /// pokrycie paliwem (części na milion).
    fn burn_fuel(&mut self, nation: u8, troops: u64) -> u64 {
        let need = troops * FUEL_PER_1000_PER_DAY / (MILLI * MILLI * TICKS_PER_DAY);
        if need == 0 {
            return 0;
        }
        let e = &mut self.economy[nation as usize - 1];
        let fuel = e.supplies[Supply::Fuel as usize].min(need);
        e.supplies[Supply::Fuel as usize] -= fuel;
        e.supply_use[Supply::Fuel as usize] += fuel;
        let horses = e.stock[HORSES as usize];
        let by_horses = (need - fuel).min(need * HORSE_MAX_PCT / 100).min(horses * HORSE_FUEL_PCT / 100);
        let used = (by_horses * 100).div_ceil(HORSE_FUEL_PCT).min(horses);
        e.stock[HORSES as usize] -= used;
        e.resource_use[HORSES as usize] += used;
        (fuel + by_horses) * PPM / need
    }

    /// Czy gracz może zaatakować prowincję (albo dosłać wojsko do trwającego ataku).
    pub fn check_attack(&self, player: PlayerId, province: u16) -> Result<(), AttackBlock> {
        let Some(n) = self.owners.nation_of(player).filter(|_| self.phase == Phase::Playing) else {
            return Err(AttackBlock::NotPlaying);
        };
        if province == 0 || province as usize > self.map.provinces.len() {
            return Err(AttackBlock::NoContact);
        }
        let pr = &self.map.provinces[province as usize - 1];
        if pr.tunnel > 0 {
            return Err(AttackBlock::Tunnel);
        }
        if self.owners.shares(province) == [(n, pr.area)] {
            return Err(AttackBlock::Owned);
        }
        let running = self.attacks.iter().filter(|a| a.nation == n);
        let max = MAX_ATTACKS[self.command_level(n).min(6) as usize];
        if !running.clone().any(|a| a.province == province) && running.count() >= max {
            return Err(AttackBlock::Limit);
        }
        if self.frontier(n, province).is_empty() {
            return Err(AttackBlock::NoContact);
        }
        if self.economy[n as usize - 1].troops < war::neutral_cost() {
            return Err(AttackBlock::NoTroops);
        }
        Ok(())
    }

    /// Atak: `percent` wojska z kraju idzie na front prowincji (do trwającego ataku – dosłane).
    fn attack(&mut self, player: PlayerId, province: u16, percent: u8) {
        if self.check_attack(player, province).is_err() {
            return;
        }
        let n = self.owners.nation_of(player).unwrap();
        let e = &mut self.economy[n as usize - 1];
        let troops = e.troops * percent.min(100) as u64 / 100;
        if troops < war::neutral_cost() {
            return;
        }
        e.troops -= troops;
        e.deployed += troops;
        match self.attacks.iter_mut().find(|a| a.nation == n && a.province == province) {
            Some(a) => a.troops += troops,
            None => self.attacks.push(Attack { nation: n, province, troops, taken: 0, supply: war::NOT_FIGHTING }),
        }
    }

    /// Odwołanie ataku – wojsko wraca do kraju.
    fn cancel_attack(&mut self, player: PlayerId, province: u16) {
        let Some(n) = self.owners.nation_of(player) else { return };
        let Some(k) = self.attacks.iter().position(|a| a.nation == n && a.province == province) else { return };
        let a = self.attacks.remove(k);
        let e = &mut self.economy[n as usize - 1];
        e.deployed -= a.troops;
        e.troops += a.troops;
    }

    /// Atak na prowincję dla ramki prowincji (z punktu widzenia `viewer`).
    fn attack_report(&self, viewer: PlayerId, province: u16) -> AttackReport {
        let n = self.owners.nation_of(viewer);
        let shares = self.owners.shares(province);
        let area = self.map.provinces[province as usize - 1].area;
        let ours = shares.iter().find(|s| Some(s.0) == n).map_or(0, |s| s.1);
        let enemy_tiles: u32 = shares.iter().filter(|s| Some(s.0) != n).map(|s| s.1).sum();
        // Szacunek kafla cudzego: największy inny udział, nasze zaopatrzenie jak u obrońcy (bez zużycia).
        let enemy = shares.iter().filter(|s| Some(s.0) != n).max_by_key(|s| (s.1, std::cmp::Reverse(s.0)));
        let enemy_cost = match (enemy, n) {
            (Some(&(d, _)), Some(n)) => war::enemy_cost(self.density(d), self.defense_pct(d), self.defense_pct(n)),
            _ => 0,
        };
        let attack = self.attacks.iter().find(|a| Some(a.nation) == n && a.province == province);
        AttackReport {
            blocked: self.check_attack(viewer, province).err(),
            neutral_tiles: area - ours - enemy_tiles,
            enemy_tiles,
            neutral_cost: (war::neutral_cost() / MILLI) as f64,
            enemy_cost: (enemy_cost / MILLI) as f64,
            available: n.map_or(0.0, |n| (self.economy[n as usize - 1].troops / MILLI) as f64),
            troops: attack.map(|a| (a.troops / MILLI) as f64),
            taken: attack.map_or(0, |a| a.taken),
        }
    }

    /// Tura fabryk w kolejności prowincji (rosnąco – deterministycznie przy wspólnych składnikach):
    /// przezbrojenie albo produkcja `OUTPUT_PER_LEVEL_PER_TICK` za poziom × obsada × premia uniwersytetów,
    /// ograniczona składnikami w puli. Fabryka bez właściciela (> 50% kafli) stoi.
    fn factory_tick(&mut self, rates: &[NationRates]) {
        for k in 0..self.buildings.len() {
            let owner = self.owners.province_owner(k as u16 + 1);
            let b = &mut self.buildings[k];
            (b.output, b.short) = (0, NO_SHORTAGE);
            let level = b.level(Building::Factory) as u64;
            let Some(n) = owner.filter(|_| level > 0) else { continue };
            if b.retool > 0 {
                b.retool -= 1;
                continue;
            }
            let r = &rates[n as usize - 1];
            let target = factory_target(level, r);
            let e = &mut self.economy[n as usize - 1];
            let p = factories::produce(b.product, target, r.universities, e);
            (b.output, b.short) = (p.produced, p.short);
        }
    }

    /// Poziom centrum dowodzenia państwa (w stolicy, co najmniej 1) – odblokowania: rodzaje zaopatrzenia,
    /// liczba ataków naraz, przeprawy przez rzeki, umocnienia.
    pub fn command_level(&self, nation: u8) -> u8 {
        self.capital(nation).map_or(1, |c| self.buildings(c).level(Building::CommandCenter).max(1))
    }

    /// Wybór produkcji fabryki w prowincji (tylko właściciel, rodzaj odblokowany centrum dowodzenia).
    /// Zmiana rodzaju w działającej fabryce – przezbrojenie `RETOOL_TICKS` tur bez produkcji.
    fn set_factory(&mut self, player: PlayerId, province: u16, supply: Supply) {
        if province == 0 || province as usize > self.map.provinces.len() {
            return;
        }
        let Some(n) = self.owners.nation_of(player) else { return };
        if self.owners.province_owner(province) != Some(n) || self.command_level(n) < COMMAND_LEVEL[supply as usize] {
            return;
        }
        let b = &mut self.buildings[province as usize - 1];
        if b.product == supply {
            return;
        }
        b.product = supply;
        if b.level(Building::Factory) > 0 {
            b.retool = RETOOL_TICKS;
        }
    }

    /// Postęp budów: stoi, gdy prowincja nie ma właściciela (nikt nie ma ponad 50% kafli).
    fn construction_tick(&mut self) {
        for (k, b) in self.buildings.iter_mut().enumerate() {
            let Some(c) = &mut b.construction else { continue };
            if self.owners.province_owner(k as u16 + 1).is_none() {
                continue;
            }
            c.done += 1;
            if c.done >= c.total {
                b.levels[c.building as usize] = c.level;
                b.construction = None;
            }
        }
    }

    /// Wielkości każdego państwa (`[n - 1]`) z mapy, budynków i zapasów: limit populacji (kafle + miasta),
    /// obsada budynków i jej pokrycie robotnikami, nawozy i wydobycie na dzień.
    pub fn rates(&self) -> Vec<NationRates> {
        let mut out: Vec<NationRates> = (0..self.economy.len())
            .map(|k| NationRates {
                cap: self.owners.area(k as u8 + 1) as u64 * PEOPLE_PER_TILE,
                staff: 0,
                staff_ppm: PPM,
                fertilizer_need: 0,
                fertilizer_ppm: PPM,
                production: [0; RESOURCE_COUNT],
                universities: 0,
                command_level: self.command_level(k as u8 + 1),
            })
            .collect();
        let owned: Vec<(usize, usize)> = (0..self.map.provinces.len())
            .filter_map(|k| self.owners.province_owner(k as u16 + 1).map(|n| (k, n as usize - 1)))
            .collect();
        for &(k, n) in &owned {
            let (b, r) = (&self.buildings[k], &mut out[n]);
            r.cap += b.level(Building::City) as u64 * CITY_POPULATION;
            r.staff += b.staff();
            r.universities += b.level(Building::University) as u32;
            if EXTRACTION[self.map.provinces[k].resource as usize] == Extraction::Farm {
                r.fertilizer_need += b.level(Building::Extraction) as u64 * FERTILIZER_PER_LEVEL_PER_DAY;
            }
        }
        for (r, e) in out.iter_mut().zip(&self.economy) {
            // Bez obsady albo bez pól – pełne pokrycie.
            r.staff_ppm = (e.workers / MILLI * PPM).checked_div(r.staff).map_or(PPM, |p| p.min(PPM));
            let need = r.fertilizer_need / TICKS_PER_DAY;
            r.fertilizer_ppm = (e.stock[FERTILIZER as usize] * PPM).checked_div(need).map_or(PPM, |p| p.min(PPM));
        }
        for &(k, n) in &owned {
            let resource = self.map.provinces[k].resource;
            let units = self.yield_per_day(k, &out[n]);
            let per_day = if resource == GOLD { units * GOLD_PER_UNIT / MILLI } else { units };
            out[n].production[resource as usize] += per_day;
        }
        out
    }

    /// Wydobycie prowincji (indeks `k`) na dzień w tysięcznych: bazowe 0,10 + poziomy budynku wydobycia
    /// × pokrycie obsady; pola uprawne – do +50% z nawozami.
    fn yield_per_day(&self, k: usize, r: &NationRates) -> u64 {
        let level = self.buildings[k].level(Building::Extraction) as u64;
        let mut built = level * EXTRACTION_PER_LEVEL_PER_DAY * r.staff_ppm / PPM;
        if EXTRACTION[self.map.provinces[k].resource as usize] == Extraction::Farm {
            built = built * (100 * PPM + FERTILIZER_BONUS_PCT * r.fertilizer_ppm) / (100 * PPM);
        }
        BASE_YIELD_PER_DAY + built
    }

    /// Czy gracz może zbudować kolejny poziom `building` w prowincji; `Ok` – koszt (złoto surowe,
    /// materiały w tysięcznych).
    pub fn check_build(&self, player: PlayerId, province: u16, building: Building) -> Result<(u64, u64), BuildBlock> {
        if self.phase != Phase::Playing {
            return Err(BuildBlock::NotPlaying);
        }
        if province == 0 || province as usize > self.map.provinces.len() {
            return Err(BuildBlock::NotOwner);
        }
        let Some(nation) = self.owners.nation_of(player).filter(|&n| self.owners.province_owner(province) == Some(n))
        else {
            return Err(BuildBlock::NotOwner);
        };
        let b = self.buildings(province);
        let level = b.level(building);
        if level >= MAX_LEVEL[building as usize] {
            return Err(BuildBlock::MaxLevel);
        }
        // Stałe powody (położenie, odblokowania) przed chwilowymi – interfejs po nich ukrywa wiersz portu.
        if building == Building::Port && !self.map.provinces[province as usize - 1].coastal {
            return Err(BuildBlock::NotCoastal);
        }
        if building == Building::CommandCenter && self.capital(nation) != Some(province) {
            return Err(BuildBlock::NotCapital);
        }
        if building == Building::Fortifications
            && level >= FORTIFICATION_CAP[self.command_level(nation).min(6) as usize]
        {
            return Err(BuildBlock::Locked);
        }
        if b.construction.is_some() {
            return Err(BuildBlock::Busy);
        }
        if b.used_points() + POINTS_PER_LEVEL[building as usize] > b.points() {
            return Err(BuildBlock::NoPoints);
        }
        let (gold, materials) = buildings::cost(building, level + 1);
        let e = &self.economy[nation as usize - 1];
        if e.gold < gold {
            return Err(BuildBlock::NoGold);
        }
        if e.supplies[Supply::EngineeringMaterials as usize] < materials {
            return Err(BuildBlock::NoMaterials);
        }
        Ok((gold, materials))
    }

    /// Budowa: koszt pobierany od razu, poziom powstaje po `buildings::build_ticks` turach.
    fn build(&mut self, player: PlayerId, province: u16, building: Building) {
        let Ok((gold, materials)) = self.check_build(player, province, building) else { return };
        let n = self.owners.nation_of(player).unwrap();
        let e = &mut self.economy[n as usize - 1];
        e.gold -= gold;
        e.supplies[Supply::EngineeringMaterials as usize] -= materials;
        let b = &mut self.buildings[province as usize - 1];
        let level = b.level(building) + 1;
        b.construction =
            Some(Construction { building, level, done: 0, total: buildings::build_ticks(level), gold, materials });
    }

    /// Anulowanie budowy przez właściciela prowincji – zwrot `CANCEL_REFUND_PCT`% kosztu.
    fn cancel_build(&mut self, player: PlayerId, province: u16) {
        if province == 0 || province as usize > self.map.provinces.len() {
            return;
        }
        let Some(n) = self.owners.nation_of(player) else { return };
        if self.owners.province_owner(province) != Some(n) {
            return;
        }
        let Some(c) = self.buildings[province as usize - 1].construction.take() else { return };
        let e = &mut self.economy[n as usize - 1];
        e.gold += c.gold * CANCEL_REFUND_PCT / 100;
        e.supplies[Supply::EngineeringMaterials as usize] += c.materials * CANCEL_REFUND_PCT / 100;
    }

    /// Budynki prowincji dla interfejsu z punktu widzenia gracza `viewer` (`None` – nie ma takiej prowincji).
    pub fn province_report(&self, viewer: PlayerId, province: u16) -> Option<ProvinceReport> {
        if province == 0 || province as usize > self.map.provinces.len() {
            return None;
        }
        let b = self.buildings(province);
        let owner = self.owners.province_owner(province);
        let resource = self.map.provinces[province as usize - 1].resource;
        let yield_per_day = owner.map_or(0.0, |n| {
            let units = self.yield_per_day(province as usize - 1, &self.rates()[n as usize - 1]);
            if resource == GOLD { (units * GOLD_PER_UNIT / MILLI) as f64 / 10.0 } else { units as f64 / MILLI as f64 }
        });
        let options = BUILDINGS
            .iter()
            .map(|&building| {
                let level = b.level(building);
                let next = (level + 1).min(MAX_LEVEL[building as usize]);
                let (gold, materials) = buildings::cost(building, next);
                BuildOption {
                    building,
                    level,
                    max_level: MAX_LEVEL[building as usize],
                    gold: (gold / 10) as f64,
                    materials: materials as f64 / MILLI as f64,
                    days: buildings::build_ticks(next) / TICKS_PER_DAY as u32,
                    points: POINTS_PER_LEVEL[building as usize],
                    staff: STAFF_PER_LEVEL[building as usize] as f64,
                    blocked: self.check_build(viewer, province, building).err(),
                }
            })
            .collect();
        Some(ProvinceReport {
            province,
            points: b.points(),
            used_points: b.used_points(),
            construction: b.construction.as_ref().map(|c| ConstructionReport {
                building: c.building,
                level: c.level,
                done: c.done,
                total: c.total,
                paused: owner.is_none(),
            }),
            options,
            mine: owner.is_some() && owner == self.owners.nation_of(viewer),
            extraction: EXTRACTION[resource as usize].name().to_string(),
            yield_per_day,
            factory: self.factory_report(province, owner),
            attack: self.attack_report(viewer, province),
        })
    }

    /// Fabryka prowincji dla interfejsu: produkt, przezbrojenie, produkcja ostatniej tury i pełna, czego
    /// brakuje, rodzaje do wyboru (receptury przy uniwersytetach właściciela, odblokowania).
    fn factory_report(&self, province: u16, owner: Option<u8>) -> FactoryReport {
        let b = self.buildings(province);
        let rates = owner.map(|n| self.rates().swap_remove(n as usize - 1));
        let universities = rates.as_ref().map_or(0, |r| r.universities);
        let command = owner.map_or(1, |n| self.command_level(n));
        let capacity = rates.as_ref().map_or(0, |r| factory_target(b.level(Building::Factory) as u64, r));
        let missing = (b.short != NO_SHORTAGE).then(|| {
            RECIPES[b.product as usize][b.short as usize]
                .iter()
                .filter(|o| o.universities <= universities)
                .map(|o| factories::input_name(o.input))
                .collect::<Vec<_>>()
                .join(" | ")
        });
        let per_day = |m: u64| (m * TICKS_PER_DAY) as f64 / MILLI as f64;
        FactoryReport {
            product: b.product,
            retool_days: b.retool as f64 / TICKS_PER_DAY as f64,
            output_per_day: per_day(b.output),
            capacity_per_day: per_day(capacity),
            missing,
            short: (b.short != NO_SHORTAGE).then_some(b.short),
            ingredients: factories::ingredients(b.product, universities, owner.map(|n| self.economy(n))),
            products: SUPPLIES
                .iter()
                .map(|&s| ProductOption {
                    supply: s,
                    name: SUPPLY_NAMES[s as usize].to_string(),
                    recipe: factories::recipe_text(s, universities),
                    command_level: (command < COMMAND_LEVEL[s as usize]).then_some(COMMAND_LEVEL[s as usize]),
                })
                .collect(),
        }
    }

    /// Walidacja i skutek intencji – w rdzeniu, nie na serwerze (serwer tylko stempluje gracza).
    fn execute(&mut self, player: PlayerId, intent: &Intent) {
        match intent {
            Intent::Ping { .. } => {}
            Intent::ChooseStart { province } => {
                self.choose_start(player, *province);
            }
            Intent::BeginSelection { players, random } => {
                if player == SERVER && self.phase == Phase::Waiting {
                    self.begin_selection(players, *random);
                }
            }
            Intent::SetTroopShare { percent } => {
                let percent = (*percent).min(100);
                let before_play = self.phase != Phase::Playing;
                match self.owners.nation_of(player) {
                    Some(n) => {
                        let e = &mut self.economy[n as usize - 1];
                        e.target_share = percent;
                        // Przed końcem wyboru startu podział ustala się od razu, bez bezwładności.
                        if before_play {
                            e.split_now();
                        }
                    }
                    // Jeszcze bez państwa – zapamiętane do chwili wybrania prowincji startowej.
                    None if before_play => {
                        self.pending_share.insert(player, percent);
                    }
                    None => {}
                }
            }
            Intent::Build { province, building } => self.build(player, *province, *building),
            Intent::CancelBuild { province } => self.cancel_build(player, *province),
            Intent::SetFactory { province, supply } => self.set_factory(player, *province, *supply),
            Intent::Attack { province, percent } => self.attack(player, *province, *percent),
            Intent::CancelAttack { province } => self.cancel_attack(player, *province),
            #[cfg(feature = "debug")]
            Intent::Debug { .. } => {}
        }
    }

    fn begin_selection(&mut self, players: &[PlayerId], random: bool) {
        let mut players: Vec<PlayerId> = players.iter().copied().filter(|&p| p != SERVER).collect();
        players.sort_unstable();
        players.dedup();
        self.players = players;
        // Pierwszy tick po wyborze: ta tura + SELECTION_TICKS kolejnych; przy starcie losowym – zaraz po
        // tej turze (starty losowane w `apply_turn`, jak na koniec zwykłego wyboru).
        let ticks = if random { 0 } else { SELECTION_TICKS };
        self.phase = Phase::Selection { end: self.tick + 1 + ticks };
    }

    /// Koniec wyboru: kto nie wybrał (uczestnik albo nie) – start losowy; potem gra właściwa.
    fn finish_selection(&mut self) {
        let mut rng = Rng(self.config.map.seed as u64 ^ ((self.tick as u64) << 32) ^ START_SALT);
        for player in self.players.clone() {
            if !self.starts.contains_key(&player)
                && let Some(province) = self.random_start(&mut rng)
            {
                self.take_start(player, province);
            }
        }
        // Stolica = prowincja startowa: centrum dowodzenia na poziomie 1.
        for &province in self.starts.values() {
            let cd = &mut self.buildings[province as usize - 1].levels[Building::CommandCenter as usize];
            *cd = (*cd).max(1);
        }
        self.phase = Phase::Playing;
        self.playing_since = self.tick;
    }

    /// Stolica państwa: prowincja startowa jego gracza.
    pub fn capital(&self, nation: u8) -> Option<u16> {
        let player = self.owners.nations().get(nation as usize - 1)?;
        self.starts.get(player).copied()
    }

    /// Wybór prowincji startowej. `false` – nie czas wyboru, prowincja nie istnieje, jest w niej
    /// inny gracz albo jest dostępna tylko tunelem. Ponowny wybór przenosi start.
    fn choose_start(&mut self, player: PlayerId, province: u16) -> bool {
        if !matches!(self.phase, Phase::Selection { .. }) || player == SERVER || !self.free_start(province) {
            return false;
        }
        // Gracz spoza listy (dołączył w czasie wyboru, tryb otwarty) – też uczestnik.
        if let Err(k) = self.players.binary_search(&player) {
            self.players.insert(k, player);
        }
        self.take_start(player, province)
    }

    /// Czy prowincja nadaje się na start: istnieje, nikt jej nie ma, nie wymaga tunelu.
    fn free_start(&self, province: u16) -> bool {
        province != 0
            && (province as usize) <= self.map.provinces.len()
            && self.map.provinces[province as usize - 1].tunnel == 0
            && self.owners.shares(province).is_empty()
    }

    /// Start gracza w prowincji (poprzedni start gracza wraca do niczyich).
    fn take_start(&mut self, player: PlayerId, province: u16) -> bool {
        let Some(nation) = self.owners.nation_for(player) else {
            return false;
        };
        // Nowe państwo – populacja startowa i pusty skarbiec (przeniesienie startu ich nie zmienia).
        if self.economy.len() < nation as usize {
            let mut e = Economy::default();
            if let Some(share) = self.pending_share.remove(&player) {
                e.target_share = share;
                e.split_now();
            }
            self.economy.push(e);
        }
        if let Some(old) = self.starts.insert(player, province) {
            self.paint_province(old, 0);
        }
        self.paint_province(province, nation);
        true
    }

    /// Właściciel wszystkich kafli prowincji.
    fn paint_province(&mut self, province: u16, nation: u8) {
        let [x0, y0, x1, y1] = self.bounds[province as usize - 1];
        let w = self.map.width as usize;
        for y in y0 as usize..=y1 as usize {
            for i in y * w + x0 as usize..=y * w + x1 as usize {
                if self.map.province[i] == province {
                    self.owners.set(i, province, nation);
                }
            }
        }
    }

    /// Losowa prowincja startowa z odstępem od innych startów. Kandydaci: wolne, bez tunelu, wielkość
    /// blisko średniej (`START_AREA_PCT`), nie graniczące z cudzym startem; z nich losowo jeden spośród
    /// `START_SPREAD` najdalszych od istniejących startów. Bez kandydatów – coraz luźniejsze zasady.
    fn random_start(&self, rng: &mut Rng) -> Option<u16> {
        let provinces = &self.map.provinces;
        if provinces.is_empty() {
            return None;
        }
        let mean = provinces.iter().map(|p| p.area as u64).sum::<u64>() / provinces.len() as u64;
        let taken: Vec<u16> = self.starts.values().copied().collect();
        let near_start = |p: u16| self.neighbours(p).iter().any(|l| taken.contains(&l.province));
        let balanced = |p: u16| {
            let area = provinces[p as usize - 1].area as u64 * 100;
            (mean * START_AREA_PCT.0..=mean * START_AREA_PCT.1).contains(&area)
        };
        let all = 1..=provinces.len() as u16;
        let mut candidates: Vec<u16> =
            all.clone().filter(|&p| self.free_start(p) && balanced(p) && !near_start(p)).collect();
        if candidates.is_empty() {
            candidates = all.clone().filter(|&p| self.free_start(p) && !near_start(p)).collect();
        }
        if candidates.is_empty() {
            candidates = all.filter(|&p| self.free_start(p)).collect();
        }
        if candidates.is_empty() {
            return None;
        }
        if taken.is_empty() {
            return Some(candidates[rng.below(candidates.len())]);
        }
        // Odległość (kwadrat, w kaflach) do najbliższego startu – im dalej, tym lepiej.
        let center = |p: u16| {
            let pr = &provinces[p as usize - 1];
            (pr.center_x as i64, pr.center_y as i64)
        };
        let gap = |p: u16| {
            let (x, y) = center(p);
            taken.iter().map(|&t| center(t)).map(|(tx, ty)| (x - tx).pow(2) + (y - ty).pow(2)).min().unwrap_or(0)
        };
        let mut by_gap: Vec<(i64, u16)> = candidates.iter().map(|&p| (gap(p), p)).collect();
        by_gap.sort_unstable_by(|a, b| b.cmp(a));
        let best = &by_gap[..by_gap.len().min(START_SPREAD)];
        Some(best[rng.below(best.len())].1)
    }

    /// Nadrabia przebieg gry sprzed dołączenia: tury z `catchup.turns` w swoich tickach,
    /// pozostałe do `catchup.tick` puste. Tury, które gra już wykonała, są pomijane.
    pub fn catch_up(&mut self, catchup: &Catchup) {
        let start = self.tick;
        let mut turns = catchup.turns.iter().filter(|t| t.tick >= start).peekable();
        while self.tick < catchup.tick {
            match turns.next_if(|t| t.tick == self.tick) {
                Some(turn) => self.apply_turn(turn),
                None => self.apply_turn(&Turn { tick: self.tick, intents: Vec::new() }),
            }
        }
    }

    /// Hash stanu do wykrywania desynców (FNV-1a): mapa (teren, biom dominujący, kafle leśne,
    /// prowincje, zasoby), potem stan gry: tick i właściciele (skrót kafli liczony przyrostowo).
    /// Rozszerzaj o każde nowe pole stanu.
    pub fn state_hash(&self) -> u32 {
        let mut h = Fnv(self.map_hash);
        h.eat(self.tick.to_le_bytes());
        h.eat(self.owners.hash_bytes());
        let phase = match self.phase {
            Phase::Waiting => [0, 0, 0, 0, 0],
            Phase::Selection { end } => {
                let e = end.to_le_bytes();
                [1, e[0], e[1], e[2], e[3]]
            }
            Phase::Playing => [2, 0, 0, 0, 0],
        };
        h.eat(phase);
        h.eat(self.players.iter().flat_map(|p| p.to_le_bytes()));
        h.eat(self.starts.iter().flat_map(|(p, s)| p.to_le_bytes().into_iter().chain(s.to_le_bytes())));
        h.eat(self.playing_since.to_le_bytes());
        h.eat(self.pending_share.iter().flat_map(|(p, s)| p.to_le_bytes().into_iter().chain([*s])));
        for e in &self.economy {
            h.eat(e.hash_bytes());
        }
        for b in &self.buildings {
            h.eat(b.hash_bytes());
        }
        for a in &self.attacks {
            h.eat(a.hash_bytes());
        }
        h.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        mapgen::{GENERATOR_VERSION, MapGenParams},
        protocol::{Intent, PlayerId, SERVER, StampedIntent},
    };

    fn config() -> GameConfig {
        GameConfig {
            generator_version: GENERATOR_VERSION,
            map: MapGenParams { width: 320, height: 200, ..Default::default() },
        }
    }

    fn turn(tick: u32) -> Turn {
        // Tura 0: serwer zaczyna wybór (gracze 0 i 1). Co siódma tura z intencjami – reszta pusta,
        // jak zwykle w logu serwera. Wybory startu zmieniają stan (część przepada – prowincja zajęta).
        let intents = if tick == 0 {
            vec![begin(&[0, 1], false)]
        } else if tick % 7 == 3 {
            let player = (tick % 2) as u16;
            vec![
                StampedIntent { player, intent: Intent::Ping { nonce: tick } },
                StampedIntent { player, intent: Intent::ChooseStart { province: (1 + tick / 7 % 4) as u16 } },
            ]
        } else {
            Vec::new()
        };
        Turn { tick, intents }
    }

    fn begin(players: &[PlayerId], random: bool) -> StampedIntent {
        StampedIntent { player: SERVER, intent: Intent::BeginSelection { players: players.to_vec(), random } }
    }

    fn choose(player: PlayerId, province: u16) -> StampedIntent {
        StampedIntent { player, intent: Intent::ChooseStart { province } }
    }

    /// Gra po `ticks` turach z podanymi intencjami w kolejnych turach (reszta pusta).
    fn play(game: &mut Game, intents: Vec<Vec<StampedIntent>>) {
        for intents in intents {
            game.apply_turn(&Turn { tick: game.tick(), intents });
        }
    }

    fn empty_turns(game: &mut Game, n: u32) {
        for _ in 0..n {
            game.apply_turn(&Turn { tick: game.tick(), intents: Vec::new() });
        }
    }

    /// Pierwsza prowincja, która może być startem (bez tunelu), od numeru `from`.
    fn startable(game: &Game, from: u16) -> u16 {
        (from..=game.map().provinces.len() as u16).find(|&p| game.map().provinces[p as usize - 1].tunnel == 0).unwrap()
    }

    #[test]
    fn choosing_a_start_only_during_selection() {
        let mut game = Game::new(config());
        let p = startable(&game, 2);
        // Przed wyborem – nic; wybór może zacząć tylko serwer.
        play(&mut game, vec![vec![choose(7, p)], vec![StampedIntent { player: 7, ..begin(&[7], false) }]]);
        assert_eq!(game.phase(), Phase::Waiting);
        assert!(game.owners().nations().is_empty());

        play(&mut game, vec![vec![begin(&[3, 7], false)]]);
        assert_eq!(game.phase(), Phase::Selection { end: 3 + SELECTION_TICKS });
        let area = game.map().provinces[p as usize - 1].area;
        play(&mut game, vec![vec![choose(7, p)]]);
        assert_eq!(game.owners().shares(p), &[(1, area)]);
        assert_eq!(game.starts().get(&7), Some(&p));
        let tiles = game.map().province.iter().enumerate().filter(|&(_, &q)| q == p);
        assert!(tiles.clone().all(|(i, _)| game.owners().owner(i) == Some(7)));
        let owned = game.owners().tiles().iter().filter(|&&n| n != 0).count();
        assert_eq!(owned, tiles.count(), "poza prowincją nic się nie zmienia");

        // Zajęta przez innego gracza – przepada; spoza mapy – przepada.
        let count = game.map().provinces.len() as u16;
        play(&mut game, vec![vec![choose(3, p), choose(3, 0), choose(3, count + 1)]]);
        assert_eq!(game.starts().get(&3), None);
        assert_eq!(game.owners().shares(p), &[(1, area)]);

        // Ponowny wybór przenosi start; stara prowincja znów niczyja.
        let q = startable(&game, p + 1);
        play(&mut game, vec![vec![choose(7, q)]]);
        assert!(game.owners().shares(p).is_empty());
        assert_eq!(game.owners().shares(q)[0].0, 1);
        assert_eq!(game.starts().len(), 1);

        // Koniec czasu: gracz 3 nie wybrał – start losowy; dalej wybór już nie działa.
        empty_turns(&mut game, SELECTION_TICKS);
        assert_eq!(game.phase(), Phase::Playing);
        let s3 = *game.starts().get(&3).expect("start losowy");
        assert!(s3 != q && game.owners().shares(s3)[0].0 == 2);
        let r = startable(&game, 1);
        play(&mut game, vec![vec![choose(7, r)]]);
        assert_eq!(game.starts().get(&7), Some(&q));
    }

    #[test]
    fn economy_grows_in_play_and_taxes_every_30_seconds() {
        let mut game = Game::new(config());
        play(&mut game, vec![vec![begin(&[1, 2], true)]]);
        let n = game.owners().nation_of(1).unwrap();
        let start = game.economy(n).clone();
        assert_eq!((start.population() / crate::economy::MILLI, start.gold), (25_000, 0));
        assert!(game.next_tax_tick().is_some());

        // Suwak: gracz bez państwa nic nie zmienia; gracz 1 – cel 80% wojska.
        play(
            &mut game,
            vec![vec![
                StampedIntent { player: 9, intent: Intent::SetTroopShare { percent: 0 } },
                StampedIntent { player: 1, intent: Intent::SetTroopShare { percent: 200 } },
            ]],
        );
        assert_eq!(game.economy(n).target_share, 100, "przycięte do 100%");
        let tax_at = game.next_tax_tick().unwrap();
        while game.tick() < tax_at - 1 {
            empty_turns(&mut game, 1);
        }
        assert_eq!(game.economy(n).gold, 0, "przed pierwszym podatkiem");
        empty_turns(&mut game, 1);
        let e = game.economy(n);
        assert_eq!(e.gold, e.workers / crate::economy::MILLI, "1 złota od robotnika (wojsko nie płaci)");
        assert_ne!(e.population(), start.population(), "populacja się zmienia");
        assert!(e.troop_share() > 50 && e.troops > e.workers);
        let reports = game.reports();
        assert_eq!(reports[n as usize - 1].player, 1);

        // Gospodarka jest w hashu – ten sam przebieg, ten sam hash; inny suwak – inny.
        let mut other = Game::new(config());
        play(&mut other, vec![vec![begin(&[1, 2], true)], vec![]]);
        let mut same = Game::new(config());
        play(&mut same, vec![vec![begin(&[1, 2], true)], vec![]]);
        assert_eq!(other.state_hash(), same.state_hash());
        play(&mut same, vec![vec![StampedIntent { player: 1, intent: Intent::SetTroopShare { percent: 10 } }]]);
        play(&mut other, vec![vec![]]);
        assert_ne!(other.state_hash(), same.state_hash());
    }

    #[test]
    fn share_is_set_at_once_before_the_game_starts() {
        let share = |player: PlayerId, percent: u8| StampedIntent { player, intent: Intent::SetTroopShare { percent } };
        let mut game = Game::new(config());
        let (p, q) = (startable(&game, 2), startable(&game, 40));
        // Gracz 2 ustawia suwak, zanim wybierze prowincję – zapamiętane; gracz 1 – po wyborze.
        play(&mut game, vec![vec![begin(&[1, 2], false)], vec![share(2, 70), choose(1, p)], vec![share(1, 80)]]);
        let n1 = game.owners().nation_of(1).unwrap();
        assert_eq!(game.economy(n1).troop_share(), 80, "w czasie wyboru – od razu, bez bezwładności");
        play(&mut game, vec![vec![choose(2, q)]]);
        let n2 = game.owners().nation_of(2).unwrap();
        assert_eq!((game.economy(n2).target_share, game.economy(n2).troop_share()), (70, 70));
        // W grze właściwej – znowu z bezwładnością.
        empty_turns(&mut game, SELECTION_TICKS);
        assert_eq!(game.phase(), Phase::Playing);
        play(&mut game, vec![vec![share(1, 20)]]);
        assert!(game.economy(n1).troop_share() > 70, "po starcie podział zmienia się stopniowo");
    }

    #[test]
    fn provinces_yield_their_resource_to_the_owner_pool() {
        let mut game = Game::new(config());
        play(&mut game, vec![vec![begin(&[1, 2], true)]]);
        let n = game.owners().nation_of(1).unwrap();
        let start = *game.starts().get(&1).unwrap();
        assert_eq!(game.owners().province_owner(start), Some(n), "start w całości – właściciel");
        let resource = game.map().provinces[start as usize - 1].resource;
        let gold_before = game.economy(n).gold;
        // 10 dni gry: 0,10/dzień → 1 jednostka surowca prowincji startowej (albo 500 złota z kopalni).
        empty_turns(&mut game, 10 * crate::goods::TICKS_PER_DAY as u32);
        let e = game.economy(n);
        if resource == GOLD {
            assert_eq!(e.stock.iter().sum::<u64>(), 0);
        } else {
            assert_eq!(e.stock[resource as usize], MILLI, "1,0 jednostki po 10 dniach");
            assert_eq!(e.stock.iter().sum::<u64>(), MILLI, "tylko surowiec własnej prowincji");
            assert!(e.gold >= gold_before);
        }
        let report = &game.reports()[n as usize - 1];
        assert_eq!(report.stock.len(), RESOURCE_COUNT);
        if resource != GOLD {
            assert_eq!((report.stock[resource as usize], report.production[resource as usize]), (1.0, 0.1));
        }
        // Niczyja prowincja nie ma właściciela.
        let free = (1..=game.map().provinces.len() as u16).find(|&p| game.owners().shares(p).is_empty()).unwrap();
        assert_eq!(game.owners().province_owner(free), None);
    }

    fn build(player: PlayerId, province: u16, building: Building) -> StampedIntent {
        StampedIntent { player, intent: Intent::Build { province, building } }
    }

    /// Gra z losowymi startami graczy 1 i 2; gracz 1 dostaje `gold` złota (surowego).
    fn rich_game(gold: u64) -> (Game, u8, u16) {
        let mut game = Game::new(config());
        play(&mut game, vec![vec![begin(&[1, 2], true)]]);
        let n = game.owners().nation_of(1).unwrap();
        game.economy[n as usize - 1].gold = gold;
        let start = *game.starts().get(&1).unwrap();
        (game, n, start)
    }

    #[test]
    fn building_costs_gold_takes_time_and_raises_the_level() {
        let (mut game, n, start) = rich_game(1_000_000);
        let other = *game.starts().get(&2).unwrap();
        // Cudza prowincja i gracz bez państwa – nic; anulowanie bez budowy – nic.
        assert_eq!(game.check_build(1, other, Building::Extraction), Err(BuildBlock::NotOwner));
        assert_eq!(game.check_build(9, start, Building::Extraction), Err(BuildBlock::NotOwner));
        assert_eq!(game.check_build(1, start, Building::Fortifications), Err(BuildBlock::Locked));
        let before = game.state_hash();
        play(&mut game, vec![vec![build(1, other, Building::Extraction), build(9, start, Building::City)]]);
        assert!(game.buildings(other).construction.is_none() && game.buildings(start).construction.is_none());
        assert_ne!(before, game.state_hash(), "tura minęła");

        // Wydobycie poziom 1: 10 000 złota (×10 surowe), bez materiałów; 15 dni.
        play(&mut game, vec![vec![build(1, start, Building::Extraction)]]);
        let e = game.economy(n);
        assert!(e.gold < 1_000_000 - 100_000 + 1_000, "pobrane od razu (plus wydobycie/podatek tej tury)");
        assert_eq!(e.supplies[Supply::EngineeringMaterials as usize], 30 * MILLI, "pierwszy poziom bez materiałów");
        assert_eq!(game.check_build(1, start, Building::City), Err(BuildBlock::Busy), "jedna budowa naraz");
        let c = game.buildings(start).construction.clone().unwrap();
        assert_eq!((c.building, c.level, c.total), (Building::Extraction, 1, 150));
        empty_turns(&mut game, 149);
        assert_eq!(game.buildings(start).level(Building::Extraction), 1);
        assert!(game.buildings(start).construction.is_none());

        // Drugi poziom kosztuje materiały (15) – anulowanie oddaje połowę.
        let gold = game.economy(n).gold;
        play(&mut game, vec![vec![build(1, start, Building::Extraction)]]);
        assert_eq!(game.economy(n).supplies[Supply::EngineeringMaterials as usize], 15 * MILLI);
        play(&mut game, vec![vec![StampedIntent { player: 1, intent: Intent::CancelBuild { province: start } }]]);
        let e = game.economy(n);
        assert_eq!(e.supplies[Supply::EngineeringMaterials as usize], 22_500, "zwrot 50% materiałów");
        assert!(e.gold < gold && e.gold > gold - 150_000 / 2 - 1_000, "zwrot 50% złota");
        assert_eq!(game.buildings(start).level(Building::Extraction), 1, "poziom zostaje");
    }

    #[test]
    fn extraction_level_yields_with_staffing_and_cities_raise_the_cap() {
        let (mut game, n, start) = rich_game(10_000_000);
        let resource = game.map().provinces[start as usize - 1].resource;
        let cap = game.rates()[n as usize - 1].cap;
        play(&mut game, vec![vec![build(1, start, Building::Extraction)]]);
        empty_turns(&mut game, 150);
        let r = &game.rates()[n as usize - 1];
        assert_eq!(r.staff, 10_000);
        assert_eq!(r.staff_ppm, PPM, "12 500 robotników obsadza 10 000");
        // 0,1 bazowo + 1,0 z poziomu (pola uprawne bez nawozów – bez premii).
        let units = 1100;
        let expected = if resource == GOLD { units * GOLD_PER_UNIT / MILLI } else { units };
        assert_eq!(r.production[resource as usize], expected);
        // Mniej robotników – mniej wydobycia.
        game.economy[n as usize - 1].workers = 5_000 * MILLI;
        let r = &game.rates()[n as usize - 1];
        assert_eq!(r.staff_ppm, 500_000);
        let units = 600;
        assert_eq!(
            r.production[resource as usize],
            if resource == GOLD { units * GOLD_PER_UNIT / MILLI } else { units }
        );
        // Miasto: +25 000 do limitu populacji (infrastruktura startowa mieści 25 punktów miasta).
        game.economy[n as usize - 1].supplies[Supply::EngineeringMaterials as usize] = 100 * MILLI;
        play(&mut game, vec![vec![build(1, start, Building::City)]]);
        assert!(game.buildings(start).construction.is_some(), "{:?}", game.check_build(1, start, Building::City));
        empty_turns(&mut game, 150);
        assert_eq!(game.rates()[n as usize - 1].cap, cap + CITY_POPULATION);
    }

    #[test]
    fn farms_use_fertilizer_for_a_bonus() {
        let mut game = Game::new(config());
        play(&mut game, vec![vec![begin(&[1], true)]]);
        // Dowolna prowincja z polami uprawnymi – przejęta przez gracza 1, poziom 2 wydobycia.
        let farm = (1..=game.map().provinces.len() as u16)
            .find(|&p| EXTRACTION[game.map().provinces[p as usize - 1].resource as usize] == Extraction::Farm)
            .unwrap();
        let n = game.owners().nation_of(1).unwrap();
        game.paint_province(farm, n);
        game.buildings[farm as usize - 1].levels[Building::Extraction as usize] = 2;
        game.economy[n as usize - 1].workers = 100_000 * MILLI;
        let resource = game.map().provinces[farm as usize - 1].resource as usize;
        let r = &game.rates()[n as usize - 1];
        assert_eq!((r.fertilizer_need, r.fertilizer_ppm), (400, 0));
        let without = r.production[resource];
        // Pełny zapas nawozów: +50% z budynku; zużycie 0,4/dzień.
        game.economy[n as usize - 1].stock[FERTILIZER as usize] = 10 * MILLI;
        let r = &game.rates()[n as usize - 1];
        assert_eq!(r.fertilizer_ppm, PPM);
        assert!(r.production[resource] >= without + 1000, "premia 50% z 2,0/dzień");
        empty_turns(&mut game, 10);
        let left = game.economy(n).stock[FERTILIZER as usize];
        assert!(left <= 10 * MILLI - 400 + 2 * BASE_YIELD_PER_DAY, "zużyte 0,4 w 10 tur: zostało {left}");
    }

    #[test]
    fn factory_turns_resources_into_supplies_and_retools() {
        let (mut game, n, start) = rich_game(0);
        let k = n as usize - 1;
        game.buildings[start as usize - 1].levels[Building::Factory as usize] = 1;
        game.economy[k].workers = 100_000 * MILLI;
        // Składniki na 2 sztuki materiałów inżynieryjnych (konie zamiast paliwa).
        for (r, units) in [(32, 1000), (14, 600), (19, 400), (11, 400)] {
            game.economy[k].stock[r] = units;
        }
        let materials = game.economy(n).supplies[Supply::EngineeringMaterials as usize];
        empty_turns(&mut game, 10);
        let e = game.economy(n);
        assert_eq!(e.supplies[Supply::EngineeringMaterials as usize], materials + 1000, "1,0 sztuki w dzień");
        assert_eq!(e.supply_output[Supply::EngineeringMaterials as usize], 100, "ostatnia tura");
        // Kolejny dzień – składniki kończą się w połowie; raport wskazuje brakujący składnik.
        empty_turns(&mut game, 20);
        assert_eq!(game.economy(n).supplies[Supply::EngineeringMaterials as usize], materials + 2000);
        let report = game.province_report(1, start).unwrap().factory;
        assert_eq!(report.output_per_day, 0.0);
        assert_eq!(report.capacity_per_day, 1.0);
        assert_eq!(report.missing.as_deref(), Some("Cement"));

        // Standardowe wojskowe wymaga centrum dowodzenia 2 – odrzucone; podstawowe cywilne – przezbrojenie.
        let set = |supply| StampedIntent { player: 1, intent: Intent::SetFactory { province: start, supply } };
        play(&mut game, vec![vec![set(Supply::StandardMilitary)]]);
        assert_eq!(game.buildings(start).product, Supply::EngineeringMaterials);
        play(&mut game, vec![vec![set(Supply::BasicCivil), StampedIntent { player: 2, ..set(Supply::Fuel) }]]);
        let b = game.buildings(start);
        assert_eq!((b.product, b.retool), (Supply::BasicCivil, RETOOL_TICKS - 1), "cudzy wybór przepada");
        let report = game.province_report(1, start).unwrap().factory;
        assert_eq!(report.products[Supply::HeavyMilitary as usize].command_level, Some(4));
        assert_eq!(report.products[Supply::LightMilitary as usize].command_level, None);
        let before = game.state_hash();
        game.buildings[start as usize - 1].retool = 0;
        assert_ne!(before, game.state_hash(), "fabryka w hashu stanu");
    }

    fn attack(player: PlayerId, province: u16, percent: u8) -> StampedIntent {
        StampedIntent { player, intent: Intent::Attack { province, percent } }
    }

    /// Sąsiad prowincji po lądzie (bez rzeki i tunelu), spełniający warunek.
    fn land_neighbour(game: &Game, province: u16, ok: impl Fn(u16) -> bool) -> u16 {
        game.neighbours(province)
            .iter()
            .filter(|l| !l.river && game.map().provinces[l.province as usize - 1].tunnel == 0)
            .map(|l| l.province)
            .find(|&q| ok(q))
            .expect("sąsiad po lądzie")
    }

    #[test]
    fn attack_takes_neutral_land_tile_by_tile_and_returns_the_rest() {
        let (mut game, n, start) = rich_game(0);
        let target = land_neighbour(&game, start, |q| game.owners().shares(q).is_empty());
        let area = game.map().provinces[target as usize - 1].area;
        game.economy[n as usize - 1].troops = 100_000 * MILLI;
        play(&mut game, vec![vec![attack(1, target, 50)]]);
        assert_eq!(game.economy(n).deployed, game.attacks[0].troops, "wojsko w ataku poza krajem");
        assert!(game.attacks[0].troops < 50_000 * MILLI, "pierwsze kafle już kosztowały");
        assert!(game.attacks[0].taken > 0 && game.attacks[0].taken <= 3, "3 kafle niczyje na turę");
        // Drugi atak naraz – limit (centrum dowodzenia 1); dosłanie do trwającego – tak.
        let other = land_neighbour(&game, start, |q| q != target && game.owners().shares(q).is_empty());
        assert_eq!(game.check_attack(1, other), Err(AttackBlock::Limit));
        assert_eq!(game.check_attack(1, target), Ok(()));
        for _ in 0..2000 {
            if game.attacks.is_empty() {
                break;
            }
            empty_turns(&mut game, 1);
        }
        assert!(game.attacks.is_empty(), "atak skończony");
        assert_eq!(game.owners().shares(target), &[(n, area)], "cała prowincja zajęta");
        let e = game.economy(n);
        assert_eq!(e.deployed, 0, "resztka wróciła");
        assert!(e.troops > 0);
        assert_eq!(game.check_attack(1, target), Err(AttackBlock::Owned));
    }

    #[test]
    fn attack_on_a_player_costs_by_defender_density_and_kills_defenders() {
        // Dwie takie same gry: sąsiednia prowincja gracza 2 (przemalowana na potrzeby testu); w jednej atak.
        let setup = || {
            let (mut game, n1, start) = rich_game(0);
            let n2 = game.owners().nation_of(2).unwrap();
            let target = land_neighbour(&game, start, |_| true);
            game.paint_province(target, n2);
            game.economy[n1 as usize - 1].troops = 200_000 * MILLI;
            (game, n1, n2, target)
        };
        let (mut game, n1, n2, target) = setup();
        let (mut calm, ..) = setup();
        play(&mut game, vec![vec![attack(1, target, 100)]]);
        empty_turns(&mut game, 20);
        empty_turns(&mut calm, 21);
        let ours = game.owners().shares(target).iter().find(|s| s.0 == n1).map_or(0, |s| s.1);
        assert!(ours > 0 && ours <= 21 * 2, "cudze kafle wolniej (2 na turę): {ours}");
        assert!(game.economy(n2).troops < calm.economy(n2).troops, "obrońca traci wojsko");
        let report = game.province_report(1, target).unwrap().attack;
        assert!(report.enemy_cost > report.neutral_cost && report.troops.is_some());
        // Odwołanie – wojsko wraca.
        play(&mut game, vec![vec![StampedIntent { player: 1, intent: Intent::CancelAttack { province: target } }]]);
        assert!(game.attacks.is_empty());
        assert_eq!(game.economy(n1).deployed, 0);
        // Gracz bez państwa i prowincja bez styku – odrzucone.
        assert_eq!(game.check_attack(9, target), Err(AttackBlock::NotPlaying));
        let far = (1..=game.map().provinces.len() as u16)
            .find(|&q| {
                game.map().provinces[q as usize - 1].tunnel == 0
                    && game.frontier(n1, q).is_empty()
                    && game.owners().shares(q).is_empty()
            })
            .unwrap();
        assert_eq!(game.check_attack(1, far), Err(AttackBlock::NoContact));
    }

    #[test]
    fn command_center_in_the_capital_unlocks_more() {
        let (mut game, n, start) = rich_game(10_000_000);
        game.economy[n as usize - 1].supplies[Supply::EngineeringMaterials as usize] = 1000 * MILLI;
        assert_eq!(game.capital(n), Some(start));
        assert_eq!(game.buildings(start).level(Building::CommandCenter), 1, "stolica ma CD 1 od startu");
        assert_eq!(game.command_level(n), 1);
        assert_eq!(game.check_build(1, start, Building::Fortifications), Err(BuildBlock::Locked));
        let other = land_neighbour(&game, start, |_| true);
        game.paint_province(other, n);
        assert_eq!(game.check_build(1, other, Building::CommandCenter), Err(BuildBlock::NotCapital));
        // Poziom 2: 30 000 złota i 30 materiałów; potem ×2.
        assert_eq!(game.check_build(1, start, Building::CommandCenter), Ok((300_000, 30 * MILLI)));
        assert_eq!(buildings::cost(Building::CommandCenter, 4), (1_200_000, 120 * MILLI));
        play(&mut game, vec![vec![build(1, start, Building::CommandCenter)]]);
        empty_turns(&mut game, buildings::build_ticks(2));
        assert_eq!(game.command_level(n), 2);
        // CD 2: standardowe zaopatrzenie i więcej ataków naraz.
        let report = game.province_report(1, start).unwrap().factory;
        assert_eq!(report.products[Supply::StandardMilitary as usize].command_level, None);
        assert_eq!(report.products[Supply::HeavyMilitary as usize].command_level, Some(4));
        assert_eq!(MAX_ATTACKS[2], 3);
        assert_eq!(game.reports()[n as usize - 1].command_level, 2);
    }

    #[test]
    fn fronts_show_both_sides_of_an_attack() {
        let (mut game, n1, start) = rich_game(0);
        let n2 = game.owners().nation_of(2).unwrap();
        let target = land_neighbour(&game, start, |_| true);
        game.paint_province(target, n2);
        game.economy[n1 as usize - 1].troops = 200_000 * MILLI;
        game.take_owner_changes();
        play(&mut game, vec![vec![attack(1, target, 100)]]);
        empty_turns(&mut game, 5);
        let changes = game.take_owner_changes();
        assert!(!changes.is_empty() && changes.iter().all(|&c| c & 0xff == n1 as u32), "zmiany kafli dla ekranu");
        assert!(game.take_owner_changes().is_empty(), "wydane raz");
        let mine = game.fronts(1);
        let theirs = game.fronts(2);
        assert_eq!((mine.len(), theirs.len()), (1, 1));
        assert!(mine[0].mine && !theirs[0].mine);
        assert_eq!(mine[0].province, target);
        assert_eq!(mine[0].defenders[0].player, 2);
        assert!(mine[0].occupation > 0.0 && mine[0].attacker.troops > 0.0);
        assert!(game.fronts(9).is_empty());
    }

    #[test]
    fn construction_is_in_the_state_hash() {
        let (mut a, _, start) = rich_game(1_000_000);
        let (mut b, _, _) = rich_game(1_000_000);
        play(&mut a, vec![vec![build(1, start, Building::Extraction)]]);
        play(&mut b, vec![vec![]]);
        assert_ne!(a.state_hash(), b.state_hash());
        let report = a.province_report(1, start).unwrap();
        assert!(report.mine && report.construction.is_some());
        assert_eq!(report.options.len(), BUILDINGS.len());
        assert_eq!(report.options[Building::City as usize].blocked, Some(BuildBlock::Busy));
        assert_eq!(b.province_report(2, start).unwrap().options[0].blocked, Some(BuildBlock::NotOwner));
    }

    #[test]
    fn random_start_spreads_players_at_once() {
        let mut game = Game::new(config());
        play(&mut game, vec![vec![begin(&[0, 1, 2], true)]]);
        assert_eq!(game.phase(), Phase::Playing);
        let starts: Vec<u16> = game.starts().values().copied().collect();
        assert_eq!(starts.len(), 3);
        for &s in &starts {
            assert_eq!(game.map().provinces[s as usize - 1].tunnel, 0);
            for &t in &starts {
                assert!(s == t || !game.neighbours(s).iter().any(|l| l.province == t), "{s} i {t} sąsiadują");
            }
        }
        // Ten sam przebieg – te same starty.
        let mut again = Game::new(config());
        play(&mut again, vec![vec![begin(&[0, 1, 2], true)]]);
        assert_eq!(again.starts(), game.starts());
        assert_eq!(again.state_hash(), game.state_hash());
    }

    #[test]
    fn owners_are_in_the_state_hash() {
        let start = |player: PlayerId| {
            let mut g = Game::new(config());
            let p = startable(&g, 5);
            play(&mut g, vec![vec![begin(&[1, 2], false)], vec![choose(player, p)]]);
            g
        };
        let (a, c, d) = (start(1), start(2), start(1));
        let mut b = Game::new(config());
        play(&mut b, vec![vec![begin(&[1, 2], false)], vec![]]);
        assert_ne!(a.state_hash(), b.state_hash());
        // Ta sama prowincja, inny gracz – inny hash; te same wybory – ten sam hash.
        assert_ne!(a.state_hash(), c.state_hash());
        assert_eq!(a.state_hash(), d.state_hash());
    }

    #[test]
    fn province_graph_is_available_in_the_game() {
        let game = Game::new(config());
        let p = (1..=game.map().provinces.len() as u16).find(|&p| !game.neighbours(p).is_empty()).unwrap();
        let q = game.neighbours(p)[0].province;
        assert!(game.neighbours(q).iter().any(|l| l.province == p));
    }

    #[test]
    fn two_games_stay_in_sync() {
        let (mut a, mut b) = (Game::new(config()), Game::new(config()));
        for tick in 0..50 {
            a.apply_turn(&turn(tick));
            b.apply_turn(&turn(tick));
            assert_eq!(a.state_hash(), b.state_hash());
        }
    }

    #[test]
    fn phased_map_gives_the_same_game() {
        let config = config();
        let (mut map, input) = mapgen::generate_base(&config.map);
        mapgen::generate_provinces(&input).apply(&mut map);
        mapgen::polish(&mut map, &config.map);
        let a = Game::new(config.clone());
        let b = Game::from_map(config, map);
        assert_eq!(a.state_hash(), b.state_hash());
    }

    #[test]
    fn state_hash_covers_the_map() {
        let config = config();
        let mut map = mapgen::generate(&config.map);
        let a = Game::from_map(config.clone(), map.clone());
        let i = map.province.iter().position(|&p| p != 0).unwrap();
        map.province[i] += 1;
        let b = Game::from_map(config, map);
        assert_ne!(a.state_hash(), b.state_hash());
    }

    #[test]
    fn catch_up_matches_playing_turn_by_turn() {
        let mut live = Game::new(config());
        let mut late = Game::from_map(config(), live.map().clone());
        for tick in 0..40 {
            live.apply_turn(&turn(tick));
        }
        // Serwer wysyła w `Catchup` tylko tury z intencjami.
        let turns = (0..40).map(turn).filter(|t| !t.intents.is_empty()).collect();
        late.catch_up(&Catchup { tick: 40, turns });
        assert_eq!(late.tick(), 40);
        assert_eq!(late.state_hash(), live.state_hash());
        for tick in 40..50 {
            live.apply_turn(&turn(tick));
            late.apply_turn(&turn(tick));
        }
        assert_eq!(late.state_hash(), live.state_hash());
    }

    #[test]
    fn restart_returns_to_the_initial_state() {
        let mut game = Game::new(config());
        let initial = game.state_hash();
        for tick in 0..20 {
            game.apply_turn(&turn(tick));
        }
        assert_ne!(game.state_hash(), initial);
        assert!(!game.owners().nations().is_empty(), "w turach były zajęcia prowincji");
        game.restart();
        assert_eq!(game.tick(), 0);
        assert_eq!(game.state_hash(), initial);
        assert!(game.owners().nations().is_empty() && game.owners().tiles().iter().all(|&n| n == 0));
        game.catch_up(&Catchup { tick: 5, turns: vec![turn(3)] });
        assert_eq!(game.tick(), 5);
    }

    #[test]
    fn protocol_json_roundtrip() {
        let msg = crate::protocol::ClientMsg::Hash { tick: 3, hash: 42 };
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(json, r#"{"type":"hash","tick":3,"hash":42}"#);
    }
}
