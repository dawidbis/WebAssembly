//! Stan gry i pętla ticków: mapa (niezmienna), licznik ticków, właściciele kafli, gospodarka,
//! budynki, hash stanu. Tu wejdą kolejne egzekucje intencji (walka, dyplomacja, boty).

use std::collections::BTreeMap;

use crate::{
    buildings::{
        self, BUILDINGS, BuildBlock, BuildOption, Building, CANCEL_REFUND_PCT, CAPITAL_MOVE_GOLD, CAPITAL_MOVE_TICKS,
        CITY_POPULATION, CapitalBlock, CapitalReport, Construction, ConstructionReport, EXTRACTION_PER_LEVEL_PER_DAY,
        FERTILIZER_BONUS_PCT, FERTILIZER_PER_LEVEL_PER_DAY, FORTIFICATION_CAP, FactoryReport, MAX_LEVEL,
        POINTS_PER_LEVEL, ProductOption, ProvinceBuildings, ProvinceReport, STAFF_PER_LEVEL,
    },
    economy::{Economy, MILLI, NationRates, NationReport, PEOPLE_PER_TILE, TAX_TICKS},
    factories::{self, COMMAND_LEVEL, Inputs, NO_SHORTAGE, OUTPUT_PER_LEVEL_PER_TICK, RECIPES, RETOOL_TICKS},
    goods::{
        BASE_PRICE, BASE_YIELD_PER_DAY, EXTRACTION, Extraction, FERTILIZER, GOLD, GOLD_PER_UNIT, RESOURCE_COUNT,
        SUPPLIES, SUPPLY_COUNT, SUPPLY_NAMES, Supply, TICKS_PER_DAY,
    },
    mapgen::{self, MapData, ProvinceLink, Terrain},
    owners::Owners,
    protocol::{Catchup, GameConfig, Intent, PlayerId, SELECTION_TICKS, SERVER, Turn},
    war::{
        self, Attack, AttackBlock, AttackInfo, AttackReport, ENEMY_TILE_POINTS, FUEL_NO_SUPPLY, FUEL_SPEED_PCT,
        FrontOpponent, FrontReport, FrontSide, HORSE_FUEL_PCT, HORSE_MAX_PCT, LANDING_PCT, Landing, LandingReport,
        MAX_FRONT_PROVINCES, MAX_FRONTS, MAX_SEA_LANDINGS, MILITARY, NEUTRAL_TILE_POINTS, NO_SUPPLY_PCT,
        POCKET_DEFENSE_PCT, POINTS_PER_TICK, SUPPLY_PER_1000_PER_DAY, SailingReport,
    },
};

/// Konie – zamiennik paliwa w ataku (numer surowca).
const HORSES: u8 = crate::goods::id::HORSES;

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
    /// Armia tubylcza w niczyich kaflach prowincji (`natives[id - 1]`, w tysięcznych osoby) – w hashu.
    natives: Vec<u64>,
    /// Zbiorniki wody do desantów (z mapy, poza hashem).
    water: Water,
    /// Rośnie przy każdej zmianie poziomu budynku – UI pobiera poziomy (ikony na mapie) tylko po zmianie.
    buildings_revision: u32,
    /// Kotły: `pocket[kafel]` = 1 – kafel państwa okrążony (`update_pockets`, co `war::POCKET_TICKS` tur,
    /// tylko gdy trwa walka z graczem; pusty – brak kotłów). Wynika ze stanu w stałych turach, więc poza hashem.
    pocket: Vec<u8>,
    /// Państwa przyłączone: pokonany → zdobywca (w hashu).
    annexed: BTreeMap<u8, u8>,
    /// Stolice zdobyte w tej turze (zdobywca, pokonany) – aneksja po rozliczeniu wszystkich frontów.
    pending_annex: Vec<(u8, u8)>,
    /// Zmiany stolicy gracza: ile razy i w której turze ostatnio (w hashu). Stolica to `starts`.
    capital_moves: BTreeMap<PlayerId, (u32, u32)>,
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
        // Startowa infrastruktura i miejsca budynków to stałe mapy (`mapgen::slots`).
        let buildings = map.provinces.iter().map(|p| ProvinceBuildings::new(p.infrastructure)).collect();
        let natives = map.provinces.iter().map(|p| war::natives(p.area, p.infrastructure)).collect();
        let water = Water::new(&map);
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
            natives,
            water,
            buildings_revision: 0,
            pocket: Vec::new(),
            annexed: BTreeMap::new(),
            pending_annex: Vec::new(),
            capital_moves: BTreeMap::new(),
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
                    .map(|a| AttackInfo {
                        provinces: a.provinces.clone(),
                        troops: (a.troops / MILLI) as f64,
                        taken: a.taken,
                    })
                    .collect();
                let mut report = NationReport::new(players[k], e, &rates[k], attacks);
                report.annexed_by = self.annexed.get(&(k as u8 + 1)).map(|&w| players[w as usize - 1]);
                report
            })
            .collect()
    }

    /// Rośnie przy każdej zmianie poziomu któregoś budynku.
    pub fn buildings_revision(&self) -> u32 {
        self.buildings_revision
    }

    /// Poziomy budynków wszystkich prowincji: `[(id - 1) * BUILDING_COUNT + Building as usize]`.
    pub fn building_levels(&self) -> Vec<u8> {
        self.buildings.iter().flat_map(|b| b.levels).collect()
    }

    /// Produkt fabryki każdej prowincji (`[id - 1]` = `Supply as u8` + 1; 0 – bez fabryki) – ikona na mapie.
    pub fn factory_products(&self) -> Vec<u8> {
        self.buildings.iter().map(|b| if b.level(Building::Factory) > 0 { b.product as u8 + 1 } else { 0 }).collect()
    }

    /// Trwające budowy wszystkich prowincji (`[(id - 1) * 3]`): rodzaj + 1 (0 – brak), docelowy poziom,
    /// postęp 0–255.
    pub fn constructions(&self) -> Vec<u8> {
        self.buildings
            .iter()
            .flat_map(|b| match &b.construction {
                Some(c) => [c.building as u8 + 1, c.level, (c.done as u64 * 255 / c.total.max(1) as u64) as u8],
                None => [0; 3],
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
            e.resource_use = [0; RESOURCE_COUNT];
            e.supply_output = [0; SUPPLY_COUNT];
            e.supply_use = [0; SUPPLY_COUNT];
            e.tick(r.cap, tax);
            // Nawozy zużywane w proporcji pokrycia (pokrycie liczone z zapasu przed turą).
            let stock = &mut e.stock[FERTILIZER as usize];
            let used = (r.fertilizer_need / TICKS_PER_DAY * r.fertilizer_ppm / PPM).min(*stock);
            *stock -= used;
            e.resource_use[FERTILIZER as usize] += used;
            for (res, &per_day) in r.production.iter().enumerate() {
                if res == GOLD as usize {
                    e.gold += per_day / TICKS_PER_DAY;
                    e.month.mining += per_day / TICKS_PER_DAY;
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
        if self.tick.is_multiple_of(war::POCKET_TICKS) {
            let fighting = self
                .attacks
                .iter()
                .any(|a| a.provinces.iter().any(|&p| self.owners.shares(p).iter().any(|s| s.0 != a.nation)));
            if fighting {
                self.update_pockets();
            } else {
                self.pocket.clear();
            }
        }
        let count = self.attacks.len();
        for i in 0..count {
            for j in i + 1..count {
                if self.opposed(i, j) {
                    self.clash(i, j);
                }
            }
        }
        // Kolejność frontów odwraca się co turę (żaden gracz nie ma stałej przewagi pierwszego ruchu na wspólnym
        // froncie); zakończone wracają do kraju po rozliczeniu wszystkich.
        let mut done = Vec::new();
        for k in 0..count {
            let i = if self.tick.is_multiple_of(2) { k } else { count - 1 - k };
            if self.attack_tick(i) {
                done.push(i);
            }
        }
        done.sort_unstable();
        for &i in done.iter().rev() {
            let a = self.attacks.remove(i);
            let e = &mut self.economy[a.nation as usize - 1];
            e.deployed -= a.troops;
            e.troops += a.troops;
        }
        for (winner, loser) in std::mem::take(&mut self.pending_annex) {
            self.annex(winner, loser);
        }
    }

    /// Kafel stolicy państwa: miejsce miasta (i centrum dowodzenia) w prowincji startowej.
    pub fn capital_tile(&self, nation: u8) -> Option<usize> {
        let c = self.capital(nation)?;
        let [x, y] = self.map.provinces[c as usize - 1].slots[mapgen::slots::SLOT_CITY];
        (x != u16::MAX).then(|| y as usize * self.map.width as usize + x as usize)
    }

    /// Mnożnik obrony (%) kafli państwa `owner` w prowincji z umocnieniami, które kontroluje (`war::FORT_DEFENSE_PCT`).
    fn fort_pct(&self, province: u16, owner: u8) -> u64 {
        war::FORT_DEFENSE_PCT[self.fort_level(province, owner) as usize]
    }

    /// Poziom umocnień prowincji działający dla państwa `owner` (0 – brak albo kontroluje je kto inny).
    fn fort_level(&self, province: u16, owner: u8) -> u8 {
        let level = self.buildings(province).level(Building::Fortifications).min(3);
        if level > 0 && self.controller(province, Building::Fortifications) == Some(owner) { level } else { 0 }
    }

    /// Czy kafel jest w kotle (`update_pockets`).
    fn in_pocket(&self, tile: usize) -> bool {
        self.pocket.get(tile).is_some_and(|&p| p != 0)
    }

    /// Kotły: kafle państw, do których nie da się dojść po własnych kaflach (bokiem i przez wąską rzekę) od
    /// stolicy ani od kafla stykającego się z ziemią niczyją albo z morzem – z każdej strony wrogowie
    /// (albo góry, rzeki, jeziora). Kocioł może obejmować wiele prowincji.
    fn update_pockets(&mut self) {
        let (w, h) = (self.map.width as usize, self.map.height as usize);
        let tiles = self.owners.tiles();
        let capitals: Vec<Option<usize>> =
            (1..=self.owners.nations().len()).map(|n| self.capital_tile(n as u8)).collect();
        let step = |i: usize, dx: i64, dy: i64| {
            let (x, y) = ((i % w) as i64 + dx, (i / w) as i64 + dy);
            (x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h).then(|| y as usize * w + x as usize)
        };
        const SIDES: [(i64, i64); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
        let mut safe = vec![false; w * h];
        let mut stack = Vec::new();
        for i in 0..w * h {
            let m = tiles[i];
            if m == 0 {
                continue;
            }
            let open = capitals[m as usize - 1] == Some(i)
                || SIDES.iter().filter_map(|&(dx, dy)| step(i, dx, dy)).any(|j| {
                    let body = self.water.body[j];
                    (self.map.province[j] != 0 && tiles[j] == 0) || (body != 0 && self.water.sea[body as usize - 1])
                });
            if open {
                safe[i] = true;
                stack.push(i);
            }
        }
        while let Some(i) = stack.pop() {
            let m = tiles[i];
            for &(dx, dy) in &SIDES {
                let mut j = step(i, dx, dy);
                // Przeprawa: w linii prostej przez najwyżej `RIVER_CROSSING_TILES` kafli rzeki.
                for _ in 0..war::RIVER_CROSSING_TILES {
                    match j {
                        Some(k) if self.map.terrain[k] == Terrain::River as u8 => j = step(k, dx, dy),
                        _ => break,
                    }
                }
                if let Some(k) = j
                    && tiles[k] == m
                    && !safe[k]
                {
                    safe[k] = true;
                    stack.push(k);
                }
            }
        }
        let pocket = (0..w * h).map(|i| (tiles[i] != 0 && !safe[i]) as u8).collect();
        self.pocket = pocket;
    }

    /// Aneksja: zdobywca `winner` dostaje całe terytorium `loser` i część jego skarbca, ludności i magazynu
    /// (`war::ANNEX_*`); fronty pokonanego przepadają, jego centrum dowodzenia jest zniszczone.
    fn annex(&mut self, winner: u8, loser: u8) {
        if winner == loser || self.annexed.contains_key(&loser) || self.annexed.contains_key(&winner) {
            return;
        }
        for i in 0..self.map.province.len() {
            if self.owners.tiles()[i] == loser {
                self.owners.set(i, self.map.province[i], winner);
            }
        }
        self.attacks.retain(|a| a.nation != loser);
        if let Some(c) = self.capital(loser) {
            self.buildings[c as usize - 1].levels[Building::CommandCenter as usize] = 0;
            self.buildings_revision = self.buildings_revision.wrapping_add(1);
        }
        let l = std::mem::take(&mut self.economy[loser as usize - 1]);
        let part = |v: u64, pct: u64| v * pct / 100;
        let e = &mut self.economy[winner as usize - 1];
        e.gold += part(l.gold, war::ANNEX_GOLD_PCT);
        e.month.loot += part(l.gold, war::ANNEX_GOLD_PCT);
        e.troops += part(l.troops, war::ANNEX_POPULATION_PCT);
        e.workers += part(l.workers, war::ANNEX_POPULATION_PCT);
        for (s, &v) in e.stock.iter_mut().zip(&l.stock) {
            *s += part(v, war::ANNEX_STOCK_PCT);
        }
        for (s, &v) in e.supplies.iter_mut().zip(&l.supplies) {
            *s += part(v, war::ANNEX_STOCK_PCT);
        }
        // Pokonany: bez ludzi, złota i zapasów (zostaje na liście państw – numery się nie zmieniają).
        self.economy[loser as usize - 1] = Economy {
            troops: 0,
            workers: 0,
            deployed: 0,
            gold: 0,
            stock: [0; RESOURCE_COUNT],
            supplies: [0; SUPPLY_COUNT],
            ..Economy::default()
        };
        self.annexed.insert(loser, winner);
    }

    /// Tura jednego frontu: zaopatrzenie wojskowe (gdy front styka się z graczem), paliwo według
    /// zaopatrzenia w użyciu (szybkość), potem zajmowanie kafli frontu wszystkich jego prowincji od
    /// najłatwiejszych, póki starcza punktów szybkości i wojska. Kolejność (`war::front_score`): teren
    /// (niziny bez lasu najpierw, wyżyny z lasem na końcu), otoczenie naszymi kaflami (kieszenie znikają
    /// od razu, front jest spójny) i „zawirowanie taktyczne” – losowy składnik, który daje szansę każdemu
    /// kafelkowi. Kafel niczyj – walka z armią tubylczą prowincji. `true` – front się kończy.
    fn attack_tick(&mut self, i: usize) -> bool {
        let n = self.attacks[i].nation;
        // Desant w drodze – jeszcze nie walczy.
        if self.attacks[i].sailing > 0 {
            self.attacks[i].sailing -= 1;
            return false;
        }
        // Armia wykrwawiona albo zastój bez inicjatywy (na wspólnym froncie – po obu stronach): koniec.
        let a = &self.attacks[i];
        if a.troops < MILLI || a.troops < a.sent * war::AUTO_END_PCT / 100 {
            return true;
        }
        if a.idle >= war::STALL_TICKS
            && (0..self.attacks.len()).filter(|&j| self.opposed(i, j)).all(|j| self.attacks[j].idle >= war::STALL_TICKS)
        {
            return true;
        }
        let taken_before = a.taken;
        // Prowincje zajęte w całości odpadają z frontu.
        let left: Vec<u16> = self.attacks[i].provinces.iter().copied().filter(|&p| !self.all_ours(n, p)).collect();
        self.attacks[i].provinces = left;
        let landing = self.attacks[i].landing.map(|l| l.body);
        let mut front = Vec::new();
        for &p in &self.attacks[i].provinces {
            front.extend(self.frontier(n, p, landing).into_iter().map(|(t, around, water)| (t, around, water, p)));
        }
        if front.is_empty() {
            return true;
        }
        let troops = self.attacks[i].troops;
        let tiles = self.owners.tiles();
        let fights = front.iter().any(|f| tiles[f.0] != 0);
        let (attacker_pct, supply, fuel_rate) =
            if fights { self.burn_supplies(n, troops) } else { (100, war::NOT_FIGHTING, FUEL_NO_SUPPLY) };
        self.attacks[i].supply = supply;
        let (fuel_ppm, horses) = self.burn_fuel(n, troops, fuel_rate);
        self.attacks[i].fuel_pct = (fuel_ppm * 100 / PPM) as u8;
        self.attacks[i].horses = horses;
        let tick = self.tick;
        // Los frontu w tej turze (ten sam u wszystkich graczy – skrót numeru państwa, prowincji i ticka).
        let first = self.attacks[i].provinces[0] as usize;
        let seed = |salt: u64| war::front_key(((n as usize) << 16) | first, tick ^ salt as u32);
        let speed = war::roll(seed(1), war::SPEED_MIN_PCT, war::SPEED_MAX_PCT);
        // Wspólny front: przeciwne fronty (państwo, numer) – bronią swoich kafli razem z wojskiem w kraju.
        let opp: Vec<(u8, usize, u64)> = (0..self.attacks.len())
            .filter(|&j| self.opposed(i, j))
            .map(|j| (self.attacks[j].nation, j, self.front_density(i, j)))
            .collect();
        self.attacks[i].front = front.len() as u32;
        self.attacks[i].strength = attacker_pct as u16;
        let pace = self.pace(troops, attacker_pct, &front, &opp);
        self.attacks[i].pace = pace as u16;
        let mut points = POINTS_PER_TICK * (100 + FUEL_SPEED_PCT * fuel_ppm / PPM) / 100 * speed / 100 * pace / 100;
        // Potyczki z tubylcami na kaflach niczyich frontu.
        if self.skirmish(i, &front, seed(2)) {
            return true;
        }
        // Zrywy: nowe punkty natarcia co `THRUST_TICKS` tur (albo gdy ich brak).
        if tick.is_multiple_of(war::THRUST_TICKS) || self.attacks[i].thrusts.is_empty() {
            let count = 1 + front.len() / war::THRUST_PER_TILES;
            let mut keyed: Vec<(u64, u32)> =
                front.iter().map(|f| (war::front_key(f.0, (tick / war::THRUST_TICKS) ^ 0x7417), f.0 as u32)).collect();
            keyed.sort_unstable();
            self.attacks[i].thrusts = keyed.into_iter().take(count).map(|k| k.1).collect();
        }
        let w = self.map.width as i64;
        let thrusts: Vec<(i64, i64)> = self.attacks[i].thrusts.iter().map(|&t| (t as i64 % w, t as i64 / w)).collect();
        let clump = |t: usize| {
            let (x, y) = (t % w as usize / war::CLUMP_TILES, t / w as usize / war::CLUMP_TILES);
            war::front_key((y << 16) | x, (tick / war::CLUMP_TICKS) ^ 0xC1u32) % (war::CLUMP_NOISE + 1)
        };
        let thrust = |t: usize| {
            let (x, y) = (t as i64 % w, t as i64 / w);
            thrusts
                .iter()
                .map(|&(tx, ty)| {
                    let d = (x - tx).abs().max((y - ty).abs());
                    war::THRUST_BONUS * (war::THRUST_RADIUS - d).max(0) / war::THRUST_RADIUS
                })
                .max()
                .unwrap_or(0)
        };
        let mut scored: Vec<(i64, usize, bool, u16)> = front
            .into_iter()
            .map(|(t, around, water, p)| {
                let score = war::front_score(self.terrain_pct(t), around, war::front_key(t, tick)) + clump(t) as i64
                    - thrust(t);
                (score, t, water, p)
            })
            .collect();
        scored.sort_unstable();
        for (_, t, water, p) in scored {
            let owner = self.owners.tiles()[t];
            let terrain = self.terrain_pct(t) * if water { LANDING_PCT } else { 100 } / 100;
            let need = if owner == 0 { NEUTRAL_TILE_POINTS } else { ENEMY_TILE_POINTS } * terrain / 100;
            if points < need {
                break;
            }
            let (cost, killed, by) = if owner == 0 {
                let density = self.native_density(p);
                (war::native_cost(density) * terrain / 100, density, None)
            } else {
                let (density, pct, by) = self.defense(owner, &opp);
                let density = density * if self.in_pocket(t) { POCKET_DEFENSE_PCT } else { 100 } / 100
                    * self.fort_pct(p, owner)
                    / 100;
                (war::enemy_cost(density, pct, attacker_pct) * terrain / 100, density, by)
            };
            if self.attacks[i].troops < cost {
                return true;
            }
            points -= need;
            self.attacks[i].troops -= cost;
            self.attacks[i].lost += cost;
            self.attacks[i].taken += 1;
            self.economy[n as usize - 1].deployed -= cost;
            if owner == 0 {
                let natives = &mut self.natives[p as usize - 1];
                *natives -= killed.min(*natives);
            } else {
                // Na wspólnym froncie ginie też wojsko przeciwnego frontu (według jego udziału w obronie).
                let mut home = killed;
                if let Some((j, share)) = by {
                    let a = &mut self.attacks[j];
                    let dead = (killed * share / PPM).min(a.troops);
                    a.troops -= dead;
                    a.lost += dead;
                    self.economy[owner as usize - 1].deployed -= dead;
                    home -= killed * share / PPM;
                }
                let d = &mut self.economy[owner as usize - 1];
                d.troops -= home.min(d.troops);
            }
            self.owners.set(t, p, n);
            // Stolica obrońcy zdobyta – aneksja po rozliczeniu frontów tej tury.
            if owner != 0 && self.capital_tile(owner) == Some(t) {
                self.pending_annex.push((n, owner));
                break;
            }
        }
        let a = &mut self.attacks[i];
        a.idle = if a.taken > taken_before { 0 } else { a.idle + 1 };
        // Wszystkie prowincje frontu nasze – koniec.
        self.attacks[i].provinces.iter().all(|&p| self.all_ours(n, p))
    }

    /// Tempo frontu (%): √(siła ataku na kafel frontu / obrona na kafel), przycięte (`war::PACE_*`).
    fn pace(&self, troops: u64, attacker_pct: u64, front: &[(usize, u32, bool, u16)], opp: &[(u8, usize, u64)]) -> u64 {
        let tiles = self.owners.tiles();
        let mut per_owner: Vec<(u8, u64)> = Vec::new();
        let mut defense = 0u64;
        for f in front {
            let owner = tiles[f.0];
            defense += if owner == 0 {
                self.native_density(f.3) * NO_SUPPLY_PCT / 100
            } else {
                let d = match per_owner.iter().find(|o| o.0 == owner) {
                    Some(o) => o.1,
                    None => {
                        let (density, pct, _) = self.defense(owner, opp);
                        let d = density * pct / 100;
                        per_owner.push((owner, d));
                        d
                    }
                };
                let d = d * self.fort_pct(f.3, owner) / 100;
                if self.in_pocket(f.0) { d * POCKET_DEFENSE_PCT / 100 } else { d }
            };
        }
        let len = front.len().max(1) as u64;
        let defense = (defense / len).max(war::MIN_DEFENSE * MILLI);
        let attack = troops * attacker_pct / 100 / len;
        (attack * 10_000 / defense).isqrt().clamp(war::PACE_MIN_PCT, war::PACE_MAX_PCT)
    }

    /// Obrona kafli państwa `owner` przed frontem z przeciwnikami `opp` (państwo, front, gęstość jego armii):
    /// gęstość (tysięczne osoby na kafel) i siła (%) obrońcy. Na wspólnym froncie bronią razem wojsko w kraju
    /// (wojsko / kafle państwa, najlepsze zaopatrzenie w puli) i armia przeciwnego frontu (jego siła) – wtedy
    /// także front i część strat, która przypada na jego armię (części na milion, według siły).
    fn defense(&self, owner: u8, opp: &[(u8, usize, u64)]) -> (u64, u64, Option<(usize, u64)>) {
        let (home, home_pct) = (self.density(owner), self.defense_pct(owner));
        match opp.iter().find(|o| o.0 == owner) {
            Some(&(_, j, front)) => {
                let front_pct = self.attacks[j].strength as u64;
                let (h, f) = (home * home_pct, front * front_pct);
                let density = home + front;
                let pct = (h + f) / density.max(1);
                (density, pct, Some((j, (f * PPM).checked_div(h + f).unwrap_or(0))))
            }
            None => (home, home_pct, None),
        }
    }

    /// Gęstość armii przeciwnego frontu `j` na wspólnym froncie z `i`: jego wojsko rozłożone na kafle jego
    /// państwa w prowincjach obu frontów (tak jak wojsko w kraju na kafle państwa).
    fn front_density(&self, i: usize, j: usize) -> u64 {
        let (a, b) = (&self.attacks[i], &self.attacks[j]);
        let mut tiles = 0u64;
        for (k, &p) in a.provinces.iter().chain(&b.provinces).enumerate() {
            let repeated = k >= a.provinces.len() && a.provinces.contains(&p);
            if !repeated {
                tiles += self.owners.shares(p).iter().find(|s| s.0 == b.nation).map_or(0, |s| s.1) as u64;
            }
        }
        b.troops / tiles.max(1)
    }

    /// Czy fronty `i` i `j` tworzą wspólny front: różne państwa, oba na lądzie, te same albo sąsiednie
    /// prowincje i każde ma kafle w prowincjach frontu drugiego.
    fn opposed(&self, i: usize, j: usize) -> bool {
        let (a, b) = (&self.attacks[i], &self.attacks[j]);
        if i == j || a.nation == b.nation || a.sailing > 0 || b.sailing > 0 {
            return false;
        }
        let holds = |n: u8, ps: &[u16]| ps.iter().any(|&p| self.owners.shares(p).iter().any(|s| s.0 == n));
        let near = a.provinces.iter().any(|&p| {
            b.provinces.contains(&p) || self.graph[p as usize - 1].iter().any(|l| b.provinces.contains(&l.province))
        });
        near && holds(b.nation, &a.provinces) && holds(a.nation, &b.provinces)
    }

    /// Starcie armii wspólnego frontu (`war::CLASH_PCT`): razem ginie dziennie 2 × `CLASH_PCT`% mniejszej
    /// armii (× los), a każda strona traci część proporcjonalną do siły przeciwnika (wojsko × siła zaopatrzenia).
    fn clash(&mut self, i: usize, j: usize) {
        let (a, b) = (&self.attacks[i], &self.attacks[j]);
        let (sa, sb) = (a.troops as u128 * a.strength as u128, b.troops as u128 * b.strength as u128);
        if sa + sb == 0 {
            return;
        }
        let key = war::front_key(((a.nation as usize) << 8) | b.nation as usize, self.tick ^ 0xC1A5);
        let luck = war::roll(key, war::SKIRMISH_MIN_PCT, war::SKIRMISH_MAX_PCT);
        let base = (a.troops.min(b.troops) * war::CLASH_PCT / 100 / TICKS_PER_DAY * luck / 100) as u128;
        for (k, loss) in [(i, base * 2 * sb / (sa + sb)), (j, base * 2 * sa / (sa + sb))] {
            let a = &mut self.attacks[k];
            let dead = (loss as u64).min(a.troops);
            a.troops -= dead;
            a.lost += dead;
            let nation = a.nation;
            self.economy[nation as usize - 1].deployed -= dead;
        }
    }

    /// Potyczki frontu `i` z tubylcami na kaflach niczyich frontu `front` (każda prowincja osobno): obie
    /// strony tracą ludzi (`war::NATIVE_HIT_PCT`, `NATIVE_LOSS_PCT` dziennie × los). `true` – na froncie
    /// nie zostało wojska.
    fn skirmish(&mut self, i: usize, front: &[(usize, u32, bool, u16)], key: u64) -> bool {
        let n = self.attacks[i].nation;
        let mut contact: Vec<(u16, u64)> = Vec::new();
        for f in front.iter().filter(|f| self.owners.tiles()[f.0] == 0) {
            match contact.iter_mut().find(|c| c.0 == f.3) {
                Some(c) => c.1 += 1,
                None => contact.push((f.3, 1)),
            }
        }
        let luck = war::roll(key, war::SKIRMISH_MIN_PCT, war::SKIRMISH_MAX_PCT);
        for (p, tiles) in contact {
            let natives = self.natives[p as usize - 1];
            let engaged = (tiles * self.native_density(p)).min(natives);
            let hit = (engaged * war::NATIVE_HIT_PCT / 100 / TICKS_PER_DAY * luck / 100).min(self.attacks[i].troops);
            let dead = engaged * war::NATIVE_LOSS_PCT / 100 / TICKS_PER_DAY;
            self.natives[p as usize - 1] -= dead.min(natives);
            let a = &mut self.attacks[i];
            a.troops -= hit;
            a.lost += hit;
            self.economy[n as usize - 1].deployed -= hit;
        }
        self.attacks[i].troops < MILLI
    }

    /// Czy cała prowincja należy do państwa.
    fn all_ours(&self, nation: u8, province: u16) -> bool {
        self.owners.shares(province) == [(nation, self.map.provinces[province as usize - 1].area)]
    }

    /// Gęstość armii tubylczej (tysięczne osoby na kafel niczyj prowincji).
    fn native_density(&self, province: u16) -> u64 {
        let area = self.map.provinces[province as usize - 1].area;
        let held: u32 = self.owners.shares(province).iter().map(|s| s.1).sum();
        self.natives[province as usize - 1].checked_div((area - held) as u64).unwrap_or(0)
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
    /// kaflami, w poprzek wąskiej rzeki (`war::RIVER_CROSSING_TILES`) albo z wodą zbiornika, nad którym
    /// państwo ma brzeg (desant: jezioro zawsze, morze `sea` – desant z portu, inne morze – tylko na wyspę
    /// prowincji, której brzeg już mamy), z liczbą naszych kafli w otoczeniu
    /// (8 sąsiadów; przeprawa liczy się za 1, desant za 0) i tym, czy kafel jest zajmowany z wody.
    fn frontier(&self, nation: u8, province: u16, sea: Option<u32>) -> Vec<(usize, u32, bool)> {
        let [x0, y0, x1, y1] = self.bounds[province as usize - 1];
        let (w, h) = (self.map.width as i64, self.map.height as i64);
        let tiles = self.owners.tiles();
        let ours = |x: i64, y: i64| x >= 0 && y >= 0 && x < w && y < h && tiles[(y * w + x) as usize] == nation;
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
        // Zbiorniki, przez które można desantować na tę prowincję.
        let bodies: Vec<u32> = self.water.province_bodies[province as usize - 1]
            .iter()
            .copied()
            .filter(|&b| {
                let own_coast = || {
                    self.water.coast[b as usize - 1]
                        .iter()
                        .any(|&t| tiles[t as usize] == nation && self.map.province[t as usize] == province)
                };
                sea == Some(b)
                    || ((!self.water.sea[b as usize - 1] || own_coast()) && self.water.reaches(b, tiles, nation))
            })
            .collect();
        let shore = |x: i64, y: i64| {
            x >= 0 && y >= 0 && x < w && y < h && {
                let b = self.water.body[(y * w + x) as usize];
                b != 0 && bodies.contains(&b)
            }
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
                let bridged = !side && sides.iter().any(|&(dx, dy)| across(x, y, dx, dy));
                let landed =
                    !side && !bridged && !bodies.is_empty() && sides.iter().any(|&(dx, dy)| shore(x + dx, y + dy));
                if !side && !bridged && !landed {
                    continue;
                }
                let around = (-1..=1)
                    .flat_map(|dy| (-1..=1).map(move |dx| (dx, dy)))
                    .filter(|&(dx, dy)| (dx, dy) != (0, 0) && ours(x + dx, y + dy))
                    .count() as u32;
                out.push((i, around.max(bridged as u32), landed));
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
        MILITARY
            .iter()
            .find(|m| e.supplies[m.supply as usize] > 0 && command >= COMMAND_LEVEL[m.supply as usize])
            .map(|m| (m.supply, m.strength(command)))
    }

    /// Zmiany właścicieli kafli od poprzedniego wywołania (`kafel << 8 | państwo`) – dla ekranu.
    pub fn take_owner_changes(&mut self) -> Vec<u32> {
        self.owners.take_changes()
    }

    /// Płynące desanty wszystkich państw (widoczne dla każdego) – kropki na mapie.
    pub fn landings(&self) -> Vec<SailingReport> {
        let w = self.map.width;
        self.attacks
            .iter()
            .filter(|a| a.sailing > 0)
            .filter_map(|a| {
                let l = a.landing?;
                let arrive = self.tick + a.sailing;
                Some(SailingReport {
                    nation: a.nation,
                    troops: (a.troops / MILLI) as f64,
                    from_x: l.from % w,
                    from_y: l.from / w,
                    to_x: l.to % w,
                    to_y: l.to / w,
                    depart: arrive.saturating_sub(l.distance / war::SAIL_TILES_PER_TICK),
                    arrive,
                })
            })
            .collect()
    }

    /// Aktywne fronty gracza `viewer`: jego i te na prowincje, w których ma kafle.
    pub fn fronts(&self, viewer: PlayerId) -> Vec<FrontReport> {
        let Some(v) = self.owners.nation_of(viewer) else { return Vec::new() };
        let player = |n: u8| self.owners.nations()[n as usize - 1];
        // Wspólny front: front oglądającego z przeciwnym frontem w jednym wierszu (przeciwny nie ma osobnego).
        let opponent = |i: usize| {
            (self.attacks[i].nation == v).then(|| (0..self.attacks.len()).find(|&j| self.opposed(i, j))).flatten()
        };
        let merged: Vec<usize> = (0..self.attacks.len()).filter_map(opponent).collect();
        self.attacks
            .iter()
            .enumerate()
            .filter(|&(k, a)| {
                !merged.contains(&k)
                    && (a.nation == v || a.provinces.iter().any(|&p| self.owners.shares(p).iter().any(|s| s.0 == v)))
            })
            .map(|(k, a)| {
                let opponent = opponent(k);
                let mut provinces = a.provinces.clone();
                for &p in opponent.map_or(&[][..], |j| &self.attacks[j].provinces[..]) {
                    if !provinces.contains(&p) {
                        provinces.push(p);
                    }
                }
                // Udziały we wszystkich prowincjach frontu (wspólnego – obu stron).
                let mut held: BTreeMap<u8, u32> = BTreeMap::new();
                let (mut area, mut natives) = (0, 0);
                let mut front = Vec::new();
                for &p in &provinces {
                    area += self.map.provinces[p as usize - 1].area;
                    natives += self.natives[p as usize - 1];
                    for &(n, t) in self.owners.shares(p) {
                        *held.entry(n).or_default() += t;
                    }
                }
                for &p in &a.provinces {
                    front.extend(self.frontier(a.nation, p, a.landing.map(|l| l.body)));
                }
                let tiles_of = |n: u8| held.get(&n).copied().unwrap_or(0);
                let pocket_tiles = if self.pocket.is_empty() {
                    0
                } else {
                    a.provinces.iter().map(|&p| self.pocket_tiles(p, a.nation)).sum()
                };
                // Środek frontu: średnia kafli frontu (bez frontu – środek pierwszej prowincji).
                let w = self.map.width as usize;
                let (cx, cy) = match (front.is_empty(), a.provinces.first()) {
                    (false, _) => {
                        let (sx, sy) =
                            front.iter().fold((0usize, 0usize), |(sx, sy), &(t, ..)| (sx + t % w, sy + t / w));
                        (sx as f64 / front.len() as f64, sy as f64 / front.len() as f64)
                    }
                    (true, Some(&p)) => {
                        let pr = &self.map.provinces[p as usize - 1];
                        (pr.center_x as f64, pr.center_y as f64)
                    }
                    (true, None) => (0.0, 0.0),
                };
                FrontReport {
                    opponent: opponent.map(|j| {
                        let b = &self.attacks[j];
                        FrontOpponent {
                            side: FrontSide {
                                player: player(b.nation),
                                troops: (b.troops / MILLI) as f64,
                                supply: SUPPLIES.get(b.supply as usize).copied(),
                                tiles: tiles_of(b.nation),
                            },
                            lost: (b.lost / MILLI) as f64,
                            pace: b.pace as f64 / 100.0,
                            strength: b.strength as f64 / 100.0,
                            fuel: b.fuel_pct as f64 / 100.0,
                        }
                    }),
                    provinces,
                    attacker: FrontSide {
                        player: player(a.nation),
                        troops: (a.troops / MILLI) as f64,
                        supply: SUPPLIES.get(a.supply as usize).copied(),
                        tiles: tiles_of(a.nation),
                    },
                    defenders: held
                        .iter()
                        .filter(|&(&n, _)| n != a.nation)
                        .map(|(&n, &tiles)| FrontSide {
                            player: player(n),
                            troops: (self.economy(n).troops / MILLI) as f64,
                            supply: self.defense_supply(n).map(|(s, _)| s),
                            tiles,
                        })
                        .collect(),
                    occupation: tiles_of(a.nation) as f64 / area.max(1) as f64,
                    neutral_tiles: area - held.values().sum::<u32>(),
                    natives: (natives / MILLI) as f64,
                    landing: a.landing.is_some(),
                    sailing_days: a.sailing as f64 / TICKS_PER_DAY as f64,
                    lost: (a.lost / MILLI) as f64,
                    pace: a.pace as f64 / 100.0,
                    center_x: cx,
                    center_y: cy,
                    mine: a.nation == v,
                    pocket_tiles,
                    strength: a.strength as f64 / 100.0,
                    fuel: a.fuel_pct as f64 / 100.0,
                    horses: a.horses,
                    idle_days: a.idle as f64 / TICKS_PER_DAY as f64,
                }
            })
            .collect()
    }

    /// Kafle kotła w prowincji, poza państwem `except`.
    fn pocket_tiles(&self, province: u16, except: u8) -> u32 {
        let [x0, y0, x1, y1] = self.bounds[province as usize - 1];
        let w = self.map.width as usize;
        let tiles = self.owners.tiles();
        (y0 as usize..=y1 as usize)
            .flat_map(|y| (x0 as usize..=x1 as usize).map(move |x| y * w + x))
            .filter(|&i| self.map.province[i] == province && tiles[i] != except && self.in_pocket(i))
            .count() as u32
    }

    /// Zużycie zaopatrzenia wojskowego przez `troops` na froncie (najlepsze odblokowane najpierw); wynik –
    /// współczynnik siły (%) jako średnia ważona pokryciem (reszta bez zaopatrzenia), rodzaj, którego
    /// zużyto najwięcej (`war::NO_SUPPLY` – żadnego), i paliwo na 1 000 wojska na dzień według zużytych
    /// rodzajów (`war::Military::fuel`).
    fn burn_supplies(&mut self, nation: u8, troops: u64) -> (u64, u8, u64) {
        let need = troops * SUPPLY_PER_1000_PER_DAY / (MILLI * MILLI * TICKS_PER_DAY);
        if need == 0 {
            return (NO_SUPPLY_PCT, war::NO_SUPPLY, FUEL_NO_SUPPLY);
        }
        let command = self.command_level(nation);
        let e = &mut self.economy[nation as usize - 1];
        let (mut left, mut weighted, mut fuel, mut main) = (need, 0, 0, (0, war::NO_SUPPLY));
        for m in &MILITARY {
            let s = m.supply as usize;
            if command < COMMAND_LEVEL[s] {
                continue;
            }
            let take = e.supplies[s].min(left);
            e.supplies[s] -= take;
            e.supply_use[s] += take;
            weighted += take * m.strength(command);
            fuel += take * m.fuel(command);
            left -= take;
            if take > main.0 {
                main = (take, s as u8);
            }
        }
        ((weighted + left * NO_SUPPLY_PCT) / need, main.1, (fuel + left * FUEL_NO_SUPPLY) / need)
    }

    /// Zużycie paliwa przez `troops` na froncie przy `per_1000` paliwa na 1 000 wojska na dzień (konie –
    /// zamiennik do `HORSE_MAX_PCT`% potrzeby); wynik – pokrycie paliwem (części na milion).
    fn burn_fuel(&mut self, nation: u8, troops: u64, per_1000: u64) -> (u64, bool) {
        let need = troops * per_1000 / (MILLI * MILLI * TICKS_PER_DAY);
        if need == 0 {
            return (0, false);
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
        ((fuel + by_horses) * PPM / need, used > 0)
    }

    /// Czy gracz może zaatakować prowincje `provinces` jednym frontem (albo dosłać wojsko do trwającego
    /// frontu z którąś z nich – reszta dołącza do niego). Limity według centrum dowodzenia: frontów naraz,
    /// prowincji na froncie, desantów morskich (z portu, paliwo na rejs). `Ok` – desant morski albo `None`.
    pub fn check_attack(&self, player: PlayerId, provinces: &[u16]) -> Result<Option<Landing>, AttackBlock> {
        let Some(n) = self.owners.nation_of(player).filter(|_| self.phase == Phase::Playing) else {
            return Err(AttackBlock::NotPlaying);
        };
        let count = self.map.provinces.len();
        if provinces.is_empty() || provinces.iter().any(|&p| p == 0 || p as usize > count) {
            return Err(AttackBlock::NoContact);
        }
        if provinces.iter().any(|&p| self.map.provinces[p as usize - 1].tunnel > 0) {
            return Err(AttackBlock::Tunnel);
        }
        if provinces.iter().any(|&p| self.all_ours(n, p)) {
            return Err(AttackBlock::Owned);
        }
        let command = self.command_level(n).min(6) as usize;
        let ours: Vec<&Attack> = self.attacks.iter().filter(|a| a.nation == n).collect();
        let with: Vec<&&Attack> = ours.iter().filter(|a| a.provinces.iter().any(|p| provinces.contains(p))).collect();
        if with.len() > 1 {
            return Err(AttackBlock::OtherFront);
        }
        let mut all: Vec<u16> = with.first().map_or(Vec::new(), |a| a.provinces.clone());
        for &p in provinces {
            if !all.contains(&p) {
                all.push(p);
            }
        }
        if all.len() > MAX_FRONT_PROVINCES[command] {
            return Err(AttackBlock::Provinces);
        }
        let landing = match with.first() {
            Some(a) => a.landing,
            None => {
                if ours.len() >= MAX_FRONTS[command] {
                    return Err(AttackBlock::Limit);
                }
                if provinces.iter().any(|&p| !self.frontier(n, p, None).is_empty()) {
                    None
                } else if provinces.iter().any(|&p| self.touches_sea(p)) {
                    if ours.iter().filter(|a| a.landing.is_some()).count() >= MAX_SEA_LANDINGS[command] {
                        return Err(AttackBlock::Landing);
                    }
                    let Some(landing) = self.landing_route(n, provinces) else { return Err(AttackBlock::NoPort) };
                    if landing.distance > war::MAX_LANDING_DISTANCE[command] {
                        return Err(AttackBlock::TooFar);
                    }
                    let fuel = self.economy[n as usize - 1].supplies[Supply::Fuel as usize];
                    if landing.troops_for(fuel) < war::MIN_ATTACK_TROOPS * MILLI {
                        return Err(AttackBlock::NoFuel);
                    }
                    Some(landing)
                } else {
                    return Err(AttackBlock::NoContact);
                }
            }
        };
        if self.economy[n as usize - 1].troops < war::MIN_ATTACK_TROOPS * MILLI {
            return Err(AttackBlock::NoTroops);
        }
        Ok(landing)
    }

    /// Desant dla interfejsu: port, czas rejsu, paliwo i limity wojska.
    pub fn landing_report(&self, nation: u8, l: &Landing) -> LandingReport {
        let command = self.command_level(nation).min(6) as usize;
        let fuel = self.economy[nation as usize - 1].supplies[Supply::Fuel as usize];
        LandingReport {
            port: l.port,
            days: l.sailing() as f64 / TICKS_PER_DAY as f64,
            fuel_per_1000: l.fuel(1000 * MILLI) as f64 / MILLI as f64,
            max_troops: war::LANDING_TROOPS[command] as f64,
            fuel_troops: (l.troops_for(fuel) / MILLI) as f64,
        }
    }

    /// Czy prowincja styka się z morzem.
    fn touches_sea(&self, province: u16) -> bool {
        self.water.province_bodies[province as usize - 1].iter().any(|&b| self.water.sea[b as usize - 1])
    }

    /// Najkrótszy rejs desantu na którąś z prowincji: z naszego portu (budynek, który kontrolujemy) nad
    /// morzem, z którym styka się cel, do najbliższego kafla brzegu celu nad tym morzem.
    fn landing_route(&self, nation: u8, provinces: &[u16]) -> Option<Landing> {
        let w = self.map.width as usize;
        let xy = |t: usize| ((t % w) as i64, (t / w) as i64);
        let mut best: Option<Landing> = None;
        for q in 1..=self.map.provinces.len() as u16 {
            if self.buildings(q).level(Building::Port) == 0 || self.controller(q, Building::Port) != Some(nation) {
                continue;
            }
            let [px, py] = self.map.provinces[q as usize - 1].slots[mapgen::slots::SLOT_PORT];
            if [px, py] == mapgen::slots::NO_SLOT {
                continue;
            }
            let port = py as usize * w + px as usize;
            let Some(body) = self.water.sea_next_to(port, w, self.map.height as usize) else { continue };
            for &p in provinces {
                let coast = self.water.coast[body as usize - 1].iter().filter(|&&t| self.map.province[t as usize] == p);
                let (sx, sy) = (px as i64, py as i64);
                let Some((d2, to)) = coast
                    .map(|&t| {
                        let (x, y) = xy(t as usize);
                        ((x - sx).pow(2) + (y - sy).pow(2), t)
                    })
                    .min()
                else {
                    continue;
                };
                let distance = (d2 as u64).isqrt() as u32;
                if best.is_none_or(|b| (distance, q) < (b.distance, b.port)) {
                    best = Some(Landing { body, port: q, distance, from: port as u32, to });
                }
            }
        }
        best
    }

    /// Atak: `percent` wojska z kraju idzie na front prowincji `provinces` (do trwającego frontu z którąś
    /// z nich – dosłane, a nowe prowincje dołączają do niego).
    fn attack(&mut self, player: PlayerId, provinces: &[u16], percent: u8) {
        let Ok(planned) = self.check_attack(player, provinces) else { return };
        let n = self.owners.nation_of(player).unwrap();
        let command = self.command_level(n).min(6) as usize;
        // Desant (nowy albo dosłanie do trwającego): limit wojska centrum dowodzenia i paliwo na rejs.
        let running = self.attacks.iter().find(|a| a.nation == n && a.provinces.iter().any(|p| provinces.contains(p)));
        let landing = running.map_or(planned, |a| a.landing);
        let on_board = running.map_or(0, |a| a.troops);
        let e = &mut self.economy[n as usize - 1];
        let mut troops = e.troops * percent.min(100) as u64 / 100;
        if let Some(l) = landing {
            troops = troops
                .min((war::LANDING_TROOPS[command] * MILLI).saturating_sub(on_board))
                .min(l.troops_for(e.supplies[Supply::Fuel as usize]));
        }
        if troops < war::MIN_ATTACK_TROOPS * MILLI {
            return;
        }
        if let Some(l) = landing {
            let fuel = l.fuel(troops).min(e.supplies[Supply::Fuel as usize]);
            e.supplies[Supply::Fuel as usize] -= fuel;
            e.supply_use[Supply::Fuel as usize] += fuel;
        }
        e.troops -= troops;
        e.deployed += troops;
        let mut targets: Vec<u16> = Vec::new();
        for &p in provinces {
            if !targets.contains(&p) {
                targets.push(p);
            }
        }
        match self.attacks.iter_mut().find(|a| a.nation == n && a.provinces.iter().any(|p| targets.contains(p))) {
            Some(a) => {
                a.troops += troops;
                a.sent += troops;
                a.idle = 0;
                for p in targets {
                    if !a.provinces.contains(&p) {
                        a.provinces.push(p);
                    }
                }
            }
            None => self.attacks.push(Attack {
                nation: n,
                provinces: targets,
                troops,
                taken: 0,
                supply: war::NOT_FIGHTING,
                landing,
                sailing: landing.map_or(0, |l| l.sailing()),
                lost: 0,
                thrusts: Vec::new(),
                pace: 100,
                front: 0,
                strength: 100,
                fuel_pct: 0,
                horses: false,
                sent: troops,
                idle: 0,
            }),
        }
    }

    /// Odwołanie frontu z prowincją `province` – wojsko wraca do kraju.
    fn cancel_attack(&mut self, player: PlayerId, province: u16) {
        let Some(n) = self.owners.nation_of(player) else { return };
        let Some(k) = self.attacks.iter().position(|a| a.nation == n && a.provinces.contains(&province)) else {
            return;
        };
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
        let fortifications = enemy.map_or(0, |&(d, _)| self.fort_level(province, d));
        let enemy_cost = match (enemy, n) {
            (Some(&(d, _)), Some(n)) => war::enemy_cost(
                self.density(d) * war::FORT_DEFENSE_PCT[fortifications as usize] / 100,
                self.defense_pct(d),
                self.defense_pct(n),
            ),
            _ => 0,
        };
        let attack = self.attacks.iter().find(|a| Some(a.nation) == n && a.provinces.contains(&province));
        let check = self.check_attack(viewer, &[province]);
        AttackReport {
            landing: match (&check, n) {
                (Ok(Some(l)), Some(n)) => Some(self.landing_report(n, l)),
                _ => None,
            },
            blocked: check.err(),
            neutral_tiles: area - ours - enemy_tiles,
            enemy_tiles,
            neutral_cost: (war::native_cost(self.native_density(province)) / MILLI) as f64,
            enemy_cost: (enemy_cost / MILLI) as f64,
            natives: (self.natives[province as usize - 1] / MILLI) as f64,
            available: n.map_or(0.0, |n| (self.economy[n as usize - 1].troops / MILLI) as f64),
            troops: attack.map(|a| (a.troops / MILLI) as f64),
            taken: attack.map_or(0, |a| a.taken),
            front: attack.map_or(Vec::new(), |a| a.provinces.clone()),
            fortifications,
        }
    }

    /// Tura fabryk w kolejności prowincji (rosnąco – deterministycznie przy wspólnych składnikach):
    /// przezbrojenie albo produkcja `OUTPUT_PER_LEVEL_PER_TICK` za poziom × obsada × premia uniwersytetów,
    /// ograniczona składnikami w puli. Fabryka bez kontrolującego (kafel fabryki niczyj) stoi.
    fn factory_tick(&mut self, rates: &[NationRates]) {
        for k in 0..self.buildings.len() {
            let owner = self.controller(k as u16 + 1, Building::Factory);
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
            let p = factories::produce(b.product, target, r.universities, &mut b.inputs, e);
            (b.output, b.short) = (p.produced, p.short);
        }
    }

    /// Państwo kontrolujące budynek w prowincji: właściciel kafla, na którym stoi (`slots`; centrum
    /// dowodzenia – kafel miasta), niezależnie od udziału w prowincji. Budynki bez miejsca na mapie
    /// (infrastruktura, umocnienia, port w prowincji bez morza) – właściciel prowincji (> 50% kafli).
    pub fn controller(&self, province: u16, building: Building) -> Option<u8> {
        let k = province as usize - 1;
        let slot =
            buildings::slot(building).map(|i| self.map.provinces[k].slots[i]).filter(|&s| s != mapgen::slots::NO_SLOT);
        match slot {
            Some([x, y]) => {
                let n = self.owners.tiles()[y as usize * self.map.width as usize + x as usize];
                (n != 0).then_some(n)
            }
            None => self.owners.province_owner(province),
        }
    }

    /// Poziom centrum dowodzenia państwa (w stolicy, co najmniej 1) – odblokowania: rodzaje zaopatrzenia,
    /// liczba ataków naraz, przeprawy przez rzeki, umocnienia.
    pub fn command_level(&self, nation: u8) -> u8 {
        self.capital(nation).map_or(1, |c| self.buildings(c).level(Building::CommandCenter).max(1))
    }

    /// Ręczny (`Some`) albo automatyczny (`None`) wybór wejścia składnika fabryki – tylko kontrolujący fabrykę.
    fn set_factory_input(&mut self, player: PlayerId, province: u16, ingredient: u8, option: Option<u8>) {
        if province == 0 || province as usize > self.map.provinces.len() {
            return;
        }
        let Some(n) = self.owners.nation_of(player) else { return };
        if self.controller(province, Building::Factory) != Some(n) {
            return;
        }
        let universities = self.rates()[n as usize - 1].universities;
        let b = &mut self.buildings[province as usize - 1];
        let Some(options) = RECIPES[b.product as usize].get(ingredient as usize) else { return };
        let k = ingredient as usize;
        match option {
            Some(o) if (o as usize) < options.len() && options[o as usize].universities <= universities => {
                b.inputs.choice[k] = o;
                b.inputs.manual |= 1 << k;
            }
            Some(_) => {}
            None => {
                b.inputs.choice[k] = factories::UNSET;
                b.inputs.manual &= !(1 << k);
            }
        }
    }

    /// Wybór produkcji fabryki w prowincji (tylko kontrolujący fabrykę, rodzaj odblokowany centrum dowodzenia).
    /// Zmiana rodzaju w działającej fabryce – przezbrojenie `RETOOL_TICKS` tur bez produkcji.
    fn set_factory(&mut self, player: PlayerId, province: u16, supply: Supply) {
        if province == 0 || province as usize > self.map.provinces.len() {
            return;
        }
        let Some(n) = self.owners.nation_of(player) else { return };
        if self.controller(province, Building::Factory) != Some(n)
            || self.command_level(n) < COMMAND_LEVEL[supply as usize]
        {
            return;
        }
        let b = &mut self.buildings[province as usize - 1];
        if b.product == supply {
            return;
        }
        b.product = supply;
        b.inputs = Inputs::default();
        if b.level(Building::Factory) > 0 {
            b.retool = RETOOL_TICKS;
        }
        // Produkt fabryki widać na mapie (ikona) – UI pobiera go razem z poziomami.
        self.buildings_revision = self.buildings_revision.wrapping_add(1);
    }

    /// Postęp budów: stoi, gdy budynku nikt nie kontroluje (`controller`).
    fn construction_tick(&mut self) {
        for k in 0..self.buildings.len() {
            let Some(building) = self.buildings[k].construction.as_ref().map(|c| c.building) else { continue };
            if self.controller(k as u16 + 1, building).is_none() {
                continue;
            }
            let b = &mut self.buildings[k];
            let Some(c) = &mut b.construction else { continue };
            c.done += 1;
            if c.done >= c.total {
                b.levels[c.building as usize] = c.level;
                b.construction = None;
                self.buildings_revision = self.buildings_revision.wrapping_add(1);
            }
        }
    }

    /// Wielkości każdego państwa (`[n - 1]`) z mapy, budynków i zapasów: limit populacji (kafle + miasta),
    /// obsada budynków i jej pokrycie robotnikami, nawozy i wydobycie na dzień. Budynek liczy się państwu,
    /// które go kontroluje (`controller` – właściciel kafla budynku), niezależnie od udziału w prowincji.
    pub fn rates(&self) -> Vec<NationRates> {
        let mut out: Vec<NationRates> = (0..self.economy.len())
            .map(|k| NationRates {
                cap: self.owners.area(k as u8 + 1) as u64 * PEOPLE_PER_TILE,
                cap_cities: 0,
                staff: 0,
                staff_ppm: PPM,
                fertilizer_need: 0,
                fertilizer_ppm: PPM,
                production: [0; RESOURCE_COUNT],
                universities: 0,
                command_level: self.command_level(k as u8 + 1),
            })
            .collect();
        // Wydobycie (prowincja, państwo kontrolujące kafel wydobycia) – także bez budynku (wydobycie bazowe).
        let mut extraction: Vec<(usize, usize)> = Vec::new();
        for k in 0..self.map.provinces.len() {
            let (province, b) = (k as u16 + 1, &self.buildings[k]);
            for &building in &BUILDINGS {
                let level = b.level(building);
                let Some(n) =
                    self.controller(province, building).filter(|_| level > 0 || building == Building::Extraction)
                else {
                    continue;
                };
                let r = &mut out[n as usize - 1];
                r.staff += level as u64 * STAFF_PER_LEVEL[building as usize];
                match building {
                    Building::City => {
                        r.cap += level as u64 * CITY_POPULATION;
                        r.cap_cities += level as u64 * CITY_POPULATION;
                    }
                    Building::University => r.universities += level as u32,
                    Building::Extraction => {
                        if EXTRACTION[self.map.provinces[k].resource as usize] == Extraction::Farm {
                            r.fertilizer_need += level as u64 * FERTILIZER_PER_LEVEL_PER_DAY;
                        }
                        extraction.push((k, n as usize - 1));
                    }
                    _ => {}
                }
            }
        }
        for (r, e) in out.iter_mut().zip(&self.economy) {
            // Bez obsady albo bez pól – pełne pokrycie.
            r.staff_ppm = (e.workers / MILLI * PPM).checked_div(r.staff).map_or(PPM, |p| p.min(PPM));
            let need = r.fertilizer_need / TICKS_PER_DAY;
            r.fertilizer_ppm = (e.stock[FERTILIZER as usize] * PPM).checked_div(need).map_or(PPM, |p| p.min(PPM));
        }
        for &(k, n) in &extraction {
            let resource = self.map.provinces[k].resource;
            let units = self.yield_per_day(k, &out[n]);
            let per_day = if resource == GOLD { units * GOLD_PER_UNIT / MILLI } else { units };
            out[n].production[resource as usize] += per_day;
        }
        out
    }

    /// Koszt poziomu `level` budynku w prowincji (złoto surowe, materiały w tysięcznych): jak
    /// `buildings::cost`, ale wydobycie rolnicze, hodowlane i połowy – tylko za złoto.
    fn build_cost(&self, province: u16, building: Building, level: u8) -> (u64, u64) {
        let (gold, materials) = buildings::cost(building, level);
        let resource = self.map.provinces[province as usize - 1].resource;
        if building == Building::Extraction && EXTRACTION[resource as usize].gold_only() {
            (gold, 0)
        } else {
            (gold, materials)
        }
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
        let Some(nation) = self.owners.nation_of(player).filter(|&n| self.controller(province, building) == Some(n))
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
        if building == Building::University && self.buildings(province).level(Building::City) == 0 {
            return Err(BuildBlock::NoCity);
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
        let (gold, materials) = self.build_cost(province, building, level + 1);
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
        e.month.buildings += gold;
        e.supplies[Supply::EngineeringMaterials as usize] -= materials;
        let b = &mut self.buildings[province as usize - 1];
        let level = b.level(building) + 1;
        b.construction =
            Some(Construction { building, level, done: 0, total: buildings::build_ticks(level), gold, materials });
    }

    /// Anulowanie budowy przez kontrolującego budowany budynek – zwrot `CANCEL_REFUND_PCT`% kosztu.
    fn cancel_build(&mut self, player: PlayerId, province: u16) {
        if province == 0 || province as usize > self.map.provinces.len() {
            return;
        }
        let Some(n) = self.owners.nation_of(player) else { return };
        let Some(building) = self.buildings[province as usize - 1].construction.as_ref().map(|c| c.building) else {
            return;
        };
        if self.controller(province, building) != Some(n) {
            return;
        }
        let Some(c) = self.buildings[province as usize - 1].construction.take() else { return };
        let e = &mut self.economy[n as usize - 1];
        e.gold += c.gold * CANCEL_REFUND_PCT / 100;
        e.month.buildings = e.month.buildings.saturating_sub(c.gold * CANCEL_REFUND_PCT / 100);
        e.supplies[Supply::EngineeringMaterials as usize] += c.materials * CANCEL_REFUND_PCT / 100;
    }

    /// Budynki prowincji dla interfejsu z punktu widzenia gracza `viewer` (`None` – nie ma takiej prowincji).
    pub fn province_report(&self, viewer: PlayerId, province: u16) -> Option<ProvinceReport> {
        if province == 0 || province as usize > self.map.provinces.len() {
            return None;
        }
        let b = self.buildings(province);
        let owner = self.owners.province_owner(province);
        let viewer_nation = self.owners.nation_of(viewer);
        let resource = self.map.provinces[province as usize - 1].resource;
        let yield_per_day = self.controller(province, Building::Extraction).map_or(0.0, |n| {
            let units = self.yield_per_day(province as usize - 1, &self.rates()[n as usize - 1]);
            if resource == GOLD { (units * GOLD_PER_UNIT / MILLI) as f64 / 10.0 } else { units as f64 / MILLI as f64 }
        });
        let options = BUILDINGS
            .iter()
            .map(|&building| {
                let level = b.level(building);
                let next = (level + 1).min(MAX_LEVEL[building as usize]);
                let (gold, materials) = self.build_cost(province, building, next);
                let controller = self.controller(province, building);
                BuildOption {
                    building,
                    level,
                    controller: controller.unwrap_or(0),
                    mine: controller.is_some() && controller == viewer_nation,
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
        // Wkład w limit populacji: oglądającego, jeśli ma tu kafle, inaczej największego udziałowca – jego
        // kafle w prowincji i miasto, jeśli je kontroluje.
        let shares = self.owners.shares(province);
        let holder = shares
            .iter()
            .find(|&&(n, _)| Some(n) == viewer_nation)
            .or_else(|| shares.iter().max_by_key(|&&(n, t)| (t, std::cmp::Reverse(n))));
        let (cap_tiles, cap_city) = holder.map_or((0, 0), |&(n, tiles)| {
            let city = if self.controller(province, Building::City) == Some(n) { b.level(Building::City) } else { 0 };
            (tiles, city)
        });
        let population_cap = cap_tiles as u64 * PEOPLE_PER_TILE + cap_city as u64 * CITY_POPULATION;
        Some(ProvinceReport {
            province,
            base_infrastructure: self.map.provinces[province as usize - 1].infrastructure,
            price: BASE_PRICE[resource as usize] as f64,
            population_cap: population_cap as f64,
            cap_tiles,
            cap_city,
            points: b.points(),
            used_points: b.used_points(),
            construction: b.construction.as_ref().map(|c| ConstructionReport {
                building: c.building,
                level: c.level,
                done: c.done,
                total: c.total,
                paused: self.controller(province, c.building).is_none(),
            }),
            options,
            mine: owner.is_some() && owner == viewer_nation,
            extraction: EXTRACTION[resource as usize].name().to_string(),
            yield_per_day,
            factory: self.factory_report(province, self.controller(province, Building::Factory)),
            attack: self.attack_report(viewer, province),
            capital: self.capital_report(viewer, province),
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
            ingredients: factories::ingredients(b.product, universities, owner.map(|n| (&b.inputs, self.economy(n)))),
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
            Intent::SetFactoryInput { province, ingredient, option } => {
                self.set_factory_input(player, *province, *ingredient, *option)
            }
            Intent::Attack { provinces, percent } => self.attack(player, provinces, *percent),
            Intent::CancelAttack { province } => self.cancel_attack(player, *province),
            Intent::MoveCapital { province } => self.move_capital(player, *province),
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
        self.buildings_revision = self.buildings_revision.wrapping_add(1);
        self.phase = Phase::Playing;
        self.playing_since = self.tick;
    }

    /// Czy gracz może przenieść stolicę do prowincji – koszt (złoto surowe) albo powód blokady.
    pub fn check_move_capital(&self, player: PlayerId, province: u16) -> Result<u64, CapitalBlock> {
        let n = self.owners.nation_of(player).filter(|n| self.phase == Phase::Playing && !self.annexed.contains_key(n));
        let Some(n) = n else { return Err(CapitalBlock::NotPlaying) };
        if province == 0 || province as usize > self.map.provinces.len() {
            return Err(CapitalBlock::NotOwner);
        }
        let capital = self.capital(n);
        if capital == Some(province) {
            return Err(CapitalBlock::AlreadyCapital);
        }
        if self.owners.province_owner(province) != Some(n)
            || self.controller(province, Building::CommandCenter) != Some(n)
        {
            return Err(CapitalBlock::NotOwner);
        }
        let building_command =
            |c: u16| self.buildings(c).construction.as_ref().is_some_and(|k| k.building == Building::CommandCenter);
        if capital.is_some_and(building_command) {
            return Err(CapitalBlock::Busy);
        }
        let (moves, last) = self.capital_moves.get(&player).copied().unwrap_or((0, 0));
        if moves > 0 && self.tick < last + CAPITAL_MOVE_TICKS {
            return Err(CapitalBlock::Cooldown);
        }
        let cost = CAPITAL_MOVE_GOLD * (moves as u64 + 1);
        if self.economy[n as usize - 1].gold < cost {
            return Err(CapitalBlock::NoGold);
        }
        Ok(cost)
    }

    /// Przeniesienie stolicy: centrum dowodzenia z poziomem do nowej prowincji, złoto, przerwa na rok.
    fn move_capital(&mut self, player: PlayerId, province: u16) {
        let Ok(cost) = self.check_move_capital(player, province) else { return };
        let n = self.owners.nation_of(player).unwrap();
        let level = match self.capital(n) {
            Some(old) => std::mem::take(&mut self.buildings[old as usize - 1].levels[Building::CommandCenter as usize]),
            None => 1,
        };
        self.buildings[province as usize - 1].levels[Building::CommandCenter as usize] = level.max(1);
        self.economy[n as usize - 1].gold -= cost;
        self.economy[n as usize - 1].month.capital += cost;
        self.starts.insert(player, province);
        let moves = self.capital_moves.entry(player).or_insert((0, 0));
        *moves = (moves.0 + 1, self.tick);
        self.buildings_revision = self.buildings_revision.wrapping_add(1);
    }

    /// Stolica w raporcie prowincji dla właściciela (`None` – nie jego).
    fn capital_report(&self, viewer: PlayerId, province: u16) -> Option<CapitalReport> {
        let n = self.owners.nation_of(viewer)?;
        if self.owners.province_owner(province) != Some(n) {
            return None;
        }
        let (moves, last) = self.capital_moves.get(&viewer).copied().unwrap_or((0, 0));
        let wait = if moves > 0 { (last + CAPITAL_MOVE_TICKS).saturating_sub(self.tick) } else { 0 };
        Some(CapitalReport {
            here: self.capital(n) == Some(province),
            cost: (CAPITAL_MOVE_GOLD * (moves as u64 + 1) / 10) as f64,
            blocked: self.check_move_capital(viewer, province).err(),
            cooldown_days: wait as f64 / TICKS_PER_DAY as f64,
        })
    }

    /// Stolica państwa: prowincja startowa jego gracza albo ta, do której ją przeniósł (`Intent::MoveCapital`).
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
        h.eat(self.natives.iter().flat_map(|n| n.to_le_bytes()));
        h.eat(self.annexed.iter().flat_map(|(&l, &w)| [l, w]));
        for (p, (count, tick)) in &self.capital_moves {
            h.eat(p.to_le_bytes().into_iter().chain(count.to_le_bytes()).chain(tick.to_le_bytes()));
        }
        h.0
    }
}

/// Zbiorniki wody (morze i jeziora; rzeki – przeprawy, nie zbiorniki) do desantów: numer zbiornika
/// kafla, czy to morze, kafle lądu prowincji nad każdym zbiornikiem i zbiorniki nad każdą prowincją.
/// Liczone z niezmiennej mapy przy `from_map` – poza hashem stanu.
#[derive(Default)]
struct Water {
    /// `body[kafel]` – zbiornik (od 1; 0 – ląd albo rzeka).
    body: Vec<u32>,
    /// `sea[b - 1]` – czy zbiornik `b` to morze (ma kafel oceanu).
    sea: Vec<bool>,
    /// `coast[b - 1]` – kafle lądu prowincji stykające się bokiem ze zbiornikiem.
    coast: Vec<Vec<u32>>,
    /// `province_bodies[id - 1]` – zbiorniki, z którymi styka się prowincja.
    province_bodies: Vec<Vec<u32>>,
}

impl Water {
    fn new(map: &MapData) -> Self {
        let (w, h) = (map.width as usize, map.height as usize);
        let wet = |i: usize| map.terrain[i] == Terrain::Ocean as u8 || map.terrain[i] == Terrain::Lake as u8;
        let mut water = Water { body: vec![0; w * h], ..Default::default() };
        let mut stack = Vec::new();
        for start in 0..w * h {
            if !wet(start) || water.body[start] != 0 {
                continue;
            }
            let id = water.sea.len() as u32 + 1;
            let mut sea = false;
            water.body[start] = id;
            stack.push(start);
            while let Some(i) = stack.pop() {
                sea |= map.terrain[i] == Terrain::Ocean as u8;
                let (x, y) = (i % w, i / w);
                for (ok, j) in
                    [(x > 0, i.wrapping_sub(1)), (x + 1 < w, i + 1), (y > 0, i.wrapping_sub(w)), (y + 1 < h, i + w)]
                {
                    if ok && wet(j) && water.body[j] == 0 {
                        water.body[j] = id;
                        stack.push(j);
                    }
                }
            }
            water.sea.push(sea);
        }
        water.coast = vec![Vec::new(); water.sea.len()];
        water.province_bodies = vec![Vec::new(); map.provinces.len()];
        for i in 0..w * h {
            let p = map.province[i];
            if p == 0 {
                continue;
            }
            let (x, y) = (i % w, i / w);
            let mut seen: [u32; 4] = [0; 4];
            for (k, (ok, j)) in
                [(x > 0, i.wrapping_sub(1)), (x + 1 < w, i + 1), (y > 0, i.wrapping_sub(w)), (y + 1 < h, i + w)]
                    .into_iter()
                    .enumerate()
            {
                let b = if ok { water.body[j] } else { 0 };
                if b == 0 || seen.contains(&b) {
                    continue;
                }
                seen[k] = b;
                water.coast[b as usize - 1].push(i as u32);
                let list = &mut water.province_bodies[p as usize - 1];
                if !list.contains(&b) {
                    list.push(b);
                }
            }
        }
        water
    }

    /// Morze (zbiornik z oceanem) stykające się bokiem z kaflem (np. port).
    fn sea_next_to(&self, tile: usize, w: usize, h: usize) -> Option<u32> {
        let (x, y) = (tile % w, tile / w);
        [(x > 0, tile.wrapping_sub(1)), (x + 1 < w, tile + 1), (y > 0, tile.wrapping_sub(w)), (y + 1 < h, tile + w)]
            .into_iter()
            .filter(|&(ok, _)| ok)
            .map(|(_, j)| self.body[j])
            .find(|&b| b != 0 && self.sea[b as usize - 1])
    }

    /// Czy państwo ma brzeg nad zbiornikiem `b` (choć jeden swój kafel lądu stykający się z nim).
    fn reaches(&self, b: u32, tiles: &[u8], nation: u8) -> bool {
        self.coast[b as usize - 1].iter().any(|&t| tiles[t as usize] == nation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        goods::id,
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
        assert_eq!((start.population() / crate::economy::MILLI, start.gold), (crate::economy::START_POPULATION, 0));
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
        assert_eq!(
            e.gold,
            e.workers / crate::economy::MILLI * crate::economy::TAX_PER_1000_WORKERS / 1000,
            "podatek od robotników (wojsko nie płaci)"
        );
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
        // Na start pula surowców jest pusta.
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
        const START_MATS: u64 = crate::goods::START_SUPPLIES[Supply::EngineeringMaterials as usize] * MILLI;
        play(&mut game, vec![vec![build(1, start, Building::Extraction)]]);
        let e = game.economy(n);
        assert!(e.gold < 1_000_000 - 100_000 + 1_000, "pobrane od razu (plus wydobycie/podatek tej tury)");
        assert_eq!(e.supplies[Supply::EngineeringMaterials as usize], START_MATS, "pierwszy poziom bez materiałów");
        assert_eq!(game.check_build(1, start, Building::City), Err(BuildBlock::Busy), "jedna budowa naraz");
        let c = game.buildings(start).construction.clone().unwrap();
        assert_eq!((c.building, c.level, c.total), (Building::Extraction, 1, 150));
        empty_turns(&mut game, 149);
        assert_eq!(game.buildings(start).level(Building::Extraction), 1);
        assert!(game.buildings(start).construction.is_none());

        // Drugi poziom kosztuje materiały (15; rolnictwo, hodowla, połowy – 0) – anulowanie oddaje połowę.
        let mats = game.build_cost(start, Building::Extraction, 2).1;
        let gold = game.economy(n).gold;
        play(&mut game, vec![vec![build(1, start, Building::Extraction)]]);
        assert_eq!(game.economy(n).supplies[Supply::EngineeringMaterials as usize], START_MATS - mats);
        play(&mut game, vec![vec![StampedIntent { player: 1, intent: Intent::CancelBuild { province: start } }]]);
        let e = game.economy(n);
        assert_eq!(e.supplies[Supply::EngineeringMaterials as usize], START_MATS - mats / 2, "zwrot 50% materiałów");
        assert!(e.gold < gold && e.gold > gold - 150_000 / 2 - 1_000, "zwrot 50% złota");
        assert_eq!(game.buildings(start).level(Building::Extraction), 1, "poziom zostaje");
    }

    #[test]
    fn university_needs_a_city_and_slots_match_extraction() {
        let (mut game, n, start) = rich_game(10_000_000);
        game.economy[n as usize - 1].supplies[Supply::EngineeringMaterials as usize] = 1000 * MILLI;
        assert_eq!(game.check_build(1, start, Building::University), Err(BuildBlock::NoCity));
        game.buildings[start as usize - 1].levels[Building::City as usize] = 1;
        assert!(game.check_build(1, start, Building::University).is_ok());
        // Pola, rancza, plantacje i łowiska – każdy poziom wydobycia tylko za złoto.
        let farm = (1..=game.map().provinces.len() as u16)
            .find(|&q| EXTRACTION[game.map().provinces[q as usize - 1].resource as usize].gold_only())
            .unwrap();
        game.paint_province(farm, n);
        game.buildings[farm as usize - 1].levels[Building::Extraction as usize] = 3;
        let (gold, materials) = game.check_build(1, farm, Building::Extraction).unwrap();
        assert_eq!((gold, materials), (buildings::cost(Building::Extraction, 4).0, 0));
        assert_eq!(buildings::slot(Building::University), buildings::slot(Building::City), "uniwersytet w mieście");
        // Teren miejsca wydobycia w generatorze zgadza się z budynkiem wydobycia.
        use mapgen::slots::{FIT, Fit};
        for (r, e) in EXTRACTION.iter().enumerate() {
            let fit = match e {
                Extraction::Mine => Fit::Highlands,
                Extraction::Sawmill | Extraction::Hunters => Fit::Forest,
                Extraction::Farm | Extraction::Plantation | Extraction::Ranch => Fit::Open,
                Extraction::OilWell | Extraction::Quarry => Fit::Any,
                Extraction::Fishery => Fit::Shore,
            };
            assert_eq!(FIT[r], fit, "surowiec {r}");
        }
    }

    #[test]
    fn buildings_belong_to_whoever_holds_their_tile() {
        let (mut game, n, start) = rich_game(10_000_000);
        let other = game.owners().nation_of(2).unwrap();
        play(&mut game, vec![vec![build(1, start, Building::Extraction)]]);
        empty_turns(&mut game, 150);
        assert_eq!(game.controller(start, Building::Factory), Some(n));
        // Drugie państwo zajmuje sam kafel fabryki (prowincja dalej w > 50% gracza 1).
        let [x, y] = game.map().provinces[start as usize - 1].slots[mapgen::slots::SLOT_FACTORY];
        let tile = y as usize * game.map().width as usize + x as usize;
        game.owners.set(tile, start, other);
        assert_eq!(game.owners().province_owner(start), Some(n));
        assert_eq!(game.controller(start, Building::Factory), Some(other));
        assert_eq!(game.controller(start, Building::Extraction), Some(n));
        assert_eq!(game.check_build(1, start, Building::Factory), Err(BuildBlock::NotOwner));
        assert_ne!(
            game.check_build(2, start, Building::Factory),
            Err(BuildBlock::NotOwner),
            "buduje ten, kto ma kafel"
        );
        let report = game.province_report(1, start).unwrap();
        let factory = report.options.iter().find(|o| o.building == Building::Factory).unwrap();
        assert!(!factory.mine && factory.controller == other);
        // Wydobycie (z obsadą) liczy się dalej państwu gracza 1.
        assert!(game.rates()[n as usize - 1].staff >= 10_000);
    }

    #[test]
    fn extraction_level_yields_with_staffing_and_cities_raise_the_cap() {
        let (mut game, n, start) = rich_game(10_000_000);
        let resource = game.map().provinces[start as usize - 1].resource;
        let cap = game.rates()[n as usize - 1].cap;
        play(&mut game, vec![vec![build(1, start, Building::Extraction)]]);
        game.economy[n as usize - 1].workers = 20_000 * MILLI;
        empty_turns(&mut game, 150);
        let r = &game.rates()[n as usize - 1];
        assert_eq!(r.staff, 10_000);
        assert_eq!(r.staff_ppm, PPM, "20 000 robotników obsadza 10 000");
        // 0,1 bazowo + 1,0 z poziomu (pola uprawne bez nawozów – bez premii).
        let units = 1100;
        // Wydobycie (złoto – do skarbca).
        let made = |r: &NationRates, units: u64| match resource {
            GOLD => r.production[resource as usize] == units * GOLD_PER_UNIT / MILLI,
            _ => r.production[resource as usize] == units,
        };
        assert!(made(r, units));
        // Mniej robotników – mniej wydobycia.
        game.economy[n as usize - 1].workers = 5_000 * MILLI;
        let r = &game.rates()[n as usize - 1];
        assert_eq!(r.staff_ppm, 500_000);
        assert!(made(r, 600));
        // Miasto: +25 000 do limitu populacji (infrastruktura startowa mieści 25 punktów miasta).
        game.economy[n as usize - 1].supplies[Supply::EngineeringMaterials as usize] = 100 * MILLI;
        play(&mut game, vec![vec![build(1, start, Building::City)]]);
        assert!(game.buildings(start).construction.is_some(), "{:?}", game.check_build(1, start, Building::City));
        empty_turns(&mut game, 150);
        assert_eq!(game.rates()[n as usize - 1].cap, cap + CITY_POPULATION);
        // Raport: jedyna prowincja państwa daje cały limit; infrastruktura bazowa jak na starcie.
        let report = game.province_report(1, start).unwrap();
        assert_eq!(report.population_cap, (cap + CITY_POPULATION) as f64);
        assert_eq!(report.base_infrastructure, game.buildings(start).level(Building::Infrastructure));
        assert_eq!(report.price, BASE_PRICE[resource as usize] as f64);
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
        for (r, units) in [(id::CEMENT, 1000), (id::WOOD, 600), (id::IRON, 400), (id::HORSES, 400)] {
            game.economy[k].stock[r as usize] = units;
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
        StampedIntent { player, intent: Intent::Attack { provinces: vec![province], percent } }
    }

    /// Czy kafle prowincji są spójne (bez wysp i kawałków za rzeką – te zajmuje się desantem).
    fn connected(game: &Game, province: u16) -> bool {
        let map = game.map();
        let w = map.width as usize;
        let tiles: Vec<usize> = (0..map.province.len()).filter(|&i| map.province[i] == province).collect();
        let mut seen = vec![false; map.province.len()];
        let mut stack = vec![tiles[0]];
        seen[tiles[0]] = true;
        let mut count = 0;
        while let Some(i) = stack.pop() {
            count += 1;
            for j in [i.wrapping_sub(1), i + 1, i.wrapping_sub(w), i + w] {
                if j < seen.len() && !seen[j] && map.province[j] == province && (j % w).abs_diff(i % w) <= 1 {
                    seen[j] = true;
                    stack.push(j);
                }
            }
        }
        count == tiles.len()
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
        let target = land_neighbour(&game, start, |q| game.owners().shares(q).is_empty() && connected(&game, q));
        let area = game.map().provinces[target as usize - 1].area;
        game.economy[n as usize - 1].troops = 100_000 * MILLI;
        // Bez paliwa ze startu – stała szybkość (paliwo przyspiesza, test niżej).
        game.economy[n as usize - 1].supplies[Supply::Fuel as usize] = 0;
        play(&mut game, vec![vec![attack(1, target, 50)]]);
        assert_eq!(game.economy(n).deployed, game.attacks[0].troops, "wojsko w ataku poza krajem");
        assert!(game.attacks[0].troops < 50_000 * MILLI, "pierwsze kafle już kosztowały");
        assert!(game.attacks[0].taken > 0 && game.attacks[0].taken <= 10, "kilka kafli niczyich na turę");
        assert_eq!(game.attacks[0].lost, 50_000 * MILLI - game.attacks[0].troops, "straty frontu");
        // Centrum dowodzenia 1: drugi front naraz – tak, ale jedna prowincja na front; dosłanie – tak.
        let other = land_neighbour(&game, start, |q| q != target && game.owners().shares(q).is_empty());
        assert_eq!(game.check_attack(1, &[other]), Ok(None));
        assert_eq!(game.check_attack(1, &[target, other]), Err(AttackBlock::Provinces));
        assert_eq!(game.check_attack(1, &[target]), Ok(None));
        // Tubylcy giną za każdy zajęty kafel.
        let natives = war::natives(area, game.map().provinces[target as usize - 1].infrastructure);
        assert!(game.natives[target as usize - 1] < natives, "tubylcy tracą ludzi");
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
        assert_eq!(game.check_attack(1, &[target]), Err(AttackBlock::Owned));
        // Tubylcy są słabsi niż dawny stały koszt: cała prowincja kosztuje mniej niż 20 wojska na kafel.
        let density = war::natives(area, 2) / area as u64;
        assert!(war::native_cost(density) < 20 * MILLI, "{}", war::native_cost(density));
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
        assert!(ours > 0 && ours <= 21 * 5, "cudze kafle wolniej (najwyżej ok. 5 na turę): {ours}");
        assert!(game.economy(n2).troops < calm.economy(n2).troops, "obrońca traci wojsko");
        let report = game.province_report(1, target).unwrap().attack;
        assert!(report.enemy_cost > report.neutral_cost && report.troops.is_some());
        // Odwołanie – wojsko wraca.
        play(&mut game, vec![vec![StampedIntent { player: 1, intent: Intent::CancelAttack { province: target } }]]);
        assert!(game.attacks.is_empty());
        assert_eq!(game.economy(n1).deployed, 0);
        // Gracz bez państwa i prowincja bez styku – odrzucone.
        assert_eq!(game.check_attack(9, &[target]), Err(AttackBlock::NotPlaying));
        let far = (1..=game.map().provinces.len() as u16)
            .find(|&q| {
                game.map().provinces[q as usize - 1].tunnel == 0
                    && game.frontier(n1, q, None).is_empty()
                    && !game.touches_sea(q)
                    && game.owners().shares(q).is_empty()
            })
            .unwrap();
        assert_eq!(game.check_attack(1, &[far]), Err(AttackBlock::NoContact));
    }

    /// Zdobycie kafla stolicy (centrum dowodzenia) przyłącza całe państwo: terytorium, połowa złota, 25%
    /// ludności, 30% magazynu; fronty pokonanego przepadają.
    #[test]
    fn taking_the_capital_annexes_the_whole_nation() {
        let (mut game, n1, start) = rich_game(0);
        let n2 = game.owners().nation_of(2).unwrap();
        let old_capital = *game.starts().get(&2).unwrap();
        // Stolica gracza 2 przeniesiona tuż obok gracza 1 (jego dawny start zostaje mu jako reszta państwa).
        let target = land_neighbour(&game, start, |q| connected(&game, q) && q != old_capital);
        game.paint_province(target, n2);
        game.starts.insert(2, target);
        game.buildings[target as usize - 1].levels[Building::CommandCenter as usize] = 1;
        let k2 = n2 as usize - 1;
        game.economy[k2].gold = 1000;
        game.economy[k2].troops = 1000 * MILLI;
        game.economy[k2].workers = 40_000 * MILLI;
        game.economy[k2].stock[id::IRON as usize] = 10 * MILLI;
        game.economy[n1 as usize - 1].troops = 400_000 * MILLI;
        play(&mut game, vec![vec![attack(1, target, 100)]]);
        for _ in 0..3000 {
            if game.annexed.contains_key(&n2) {
                break;
            }
            empty_turns(&mut game, 1);
        }
        assert_eq!(game.annexed.get(&n2), Some(&n1), "stolica zdobyta – aneksja");
        assert_eq!(game.owners().area(n2), 0, "całe terytorium u zdobywcy");
        assert_eq!(game.owners().province_owner(old_capital), Some(n1));
        assert_eq!(game.economy(n2).population(), 0);
        assert_eq!(game.buildings(target).level(Building::CommandCenter), 0, "centrum dowodzenia zniszczone");
        assert!(game.attacks.iter().all(|a| a.nation != n2));
        let report = &game.reports()[k2];
        assert_eq!(report.annexed_by, Some(1));
        assert_eq!(game.reports()[n1 as usize - 1].annexed_by, None);
        // Pokonany nie może już atakować.
        assert!(game.check_attack(2, &[start]).is_err());

        // Łupy: połowa złota, 25% wojska i robotników w kraju, 30% surowców i zaopatrzenia.
        let (mut game, n1, _) = rich_game(0);
        let n2 = game.owners().nation_of(2).unwrap();
        let before = game.economy(n1).clone();
        let l = &mut game.economy[n2 as usize - 1];
        (l.gold, l.troops, l.workers) = (1000, 8000 * MILLI, 40_000 * MILLI);
        l.stock[id::IRON as usize] = 10 * MILLI;
        l.supplies[Supply::Fuel as usize] = 20 * MILLI;
        game.annex(n1, n2);
        let e1 = game.economy(n1);
        assert_eq!(e1.gold - before.gold, 500);
        assert_eq!((e1.troops - before.troops, e1.workers - before.workers), (2000 * MILLI, 10_000 * MILLI));
        assert_eq!(e1.stock[id::IRON as usize] - before.stock[id::IRON as usize], 3 * MILLI);
        assert_eq!(e1.supplies[Supply::Fuel as usize] - before.supplies[Supply::Fuel as usize], 6 * MILLI);
        assert_eq!(game.owners().area(n2), 0);
    }

    /// Kocioł: prowincja gracza otoczona ze wszystkich stron przez wroga (bez morza i ziemi niczyjej) broni
    /// się słabiej; stolica i kafle przy ziemi niczyjej nie są w kotle.
    #[test]
    fn surrounded_tiles_form_a_pocket() {
        let (mut game, n1, _) = rich_game(0);
        let n2 = game.owners().nation_of(2).unwrap();
        let count = game.map().provinces.len() as u16;
        let pocket = (1..=count)
            .find(|&q| {
                let pr = &game.map().provinces[q as usize - 1];
                pr.tunnel == 0
                    && !pr.coastal
                    && !game.touches_sea(q)
                    && game.owners().shares(q).is_empty()
                    && game
                        .neighbours(q)
                        .iter()
                        .all(|l| game.owners().shares(l.province).is_empty() && !game.touches_sea(l.province))
                    && game.neighbours(q).len() >= 3
            })
            .expect("prowincja w głębi lądu");
        game.paint_province(pocket, n2);
        let ring: Vec<u16> = game.neighbours(pocket).iter().map(|l| l.province).collect();
        for &q in &ring {
            game.paint_province(q, n1);
        }
        game.update_pockets();
        let tiles: Vec<usize> = (0..game.map().province.len()).filter(|&i| game.map().province[i] == pocket).collect();
        assert!(tiles.iter().all(|&i| game.in_pocket(i)), "cała prowincja w kotle");
        assert!(!game.in_pocket(game.capital_tile(n2).unwrap()), "stolica nigdy w kotle");
        assert_eq!(game.pocket_tiles(pocket, n1), tiles.len() as u32);
        // Atak na kocioł – front widzi kafle kotła.
        game.economy[n1 as usize - 1].troops = 100_000 * MILLI;
        play(&mut game, vec![vec![attack(1, pocket, 50)]]);
        let fronts = game.fronts(1);
        assert!(fronts.iter().any(|f| f.pocket_tiles > 0), "front pokazuje kocioł");
        // Bez okrążenia (pierścień oddany niczyim) kotła nie ma.
        for &q in &ring {
            game.paint_province(q, 0);
        }
        game.update_pockets();
        assert!(tiles.iter().all(|&i| !game.in_pocket(i) || game.owners().tiles()[i] != n2));
    }

    #[test]
    fn factory_input_can_be_chosen_by_hand_and_back_to_auto() {
        let (mut game, n, start) = rich_game(0);
        game.buildings[start as usize - 1].levels[Building::Factory as usize] = 1;
        let set = |ingredient: u8, option: Option<u8>| StampedIntent {
            player: 1,
            intent: Intent::SetFactoryInput { province: start, ingredient, option },
        };
        let revision = game.buildings_revision();
        // Materiały: składnik 3 – paliwo | konie; ręcznie konie.
        play(&mut game, vec![vec![set(3, Some(1)), StampedIntent { player: 2, ..set(3, Some(0)) }]]);
        let report = game.province_report(1, start).unwrap().factory;
        assert_eq!((report.ingredients[3].used, report.ingredients[3].manual), (Some(1), true));
        assert!(!report.ingredients[0].manual);
        // Zły numer zamiennika – bez zmian; `None` – z powrotem automatycznie.
        play(&mut game, vec![vec![set(3, Some(9))], vec![set(3, None)]]);
        assert!(!game.province_report(1, start).unwrap().factory.ingredients[3].manual);
        assert_eq!(game.buildings_revision(), revision, "wybór wejścia nie zmienia poziomów");
        // Zmiana produktu zeruje wybory (i zmienia ikonę fabryki na mapie).
        play(&mut game, vec![vec![set(0, Some(0))]]);
        play(
            &mut game,
            vec![vec![StampedIntent {
                player: 1,
                intent: Intent::SetFactory { province: start, supply: Supply::Fuel },
            }]],
        );
        assert_eq!(game.buildings(start).inputs, Inputs::default());
        assert_eq!(game.economy(n).deployed, 0);
        assert_ne!(game.buildings_revision(), revision, "nowy produkt – nowa ikona");
        assert_eq!(game.factory_products()[start as usize - 1], Supply::Fuel as u8 + 1);
        assert_eq!(game.building_levels().len(), game.map().provinces.len() * buildings::BUILDING_COUNT);
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
        assert_eq!((MAX_FRONTS[2], MAX_FRONT_PROVINCES[2], MAX_SEA_LANDINGS[2]), (2, 2, 1));
        assert_eq!(game.reports()[n as usize - 1].command_level, 2);
    }

    /// Poziom centrum dowodzenia w stolicy gracza 1 (na potrzeby testu, bez budowy).
    fn set_command(game: &mut Game, start: u16, level: u8) {
        game.buildings[start as usize - 1].levels[Building::CommandCenter as usize] = level;
    }

    #[test]
    fn one_front_takes_several_provinces_within_command_limits() {
        let (mut game, n, start) = rich_game(0);
        game.economy[n as usize - 1].troops = 200_000 * MILLI;
        let free = |g: &Game, q: u16| g.owners().shares(q).is_empty();
        let a = land_neighbour(&game, start, |q| free(&game, q));
        let b = land_neighbour(&game, start, |q| q != a && free(&game, q));
        assert_eq!(game.check_attack(1, &[a, b]), Err(AttackBlock::Provinces), "CD 1: jedna prowincja na front");
        set_command(&mut game, start, 2);
        assert_eq!(game.check_attack(1, &[a, b]), Ok(None));
        play(
            &mut game,
            vec![vec![StampedIntent { player: 1, intent: Intent::Attack { provinces: vec![a, b], percent: 50 } }]],
        );
        assert_eq!(game.attacks.len(), 1, "jeden front");
        assert_eq!(game.attacks[0].provinces, vec![a, b]);
        empty_turns(&mut game, 30);
        let held = |q: u16| game.owners().shares(q).iter().find(|s| s.0 == n).map_or(0, |s| s.1);
        assert!(held(a) > 0 && held(b) > 0, "front idzie na obie prowincje naraz");
        // Dosłanie z trzecią prowincją – ponad limit CD 2 (2 na front).
        let c = land_neighbour(&game, a, |q| q != b && free(&game, q));
        assert_eq!(game.check_attack(1, &[a, c]), Err(AttackBlock::Provinces));
        set_command(&mut game, start, 3);
        assert_eq!(game.check_attack(1, &[a, c]), Ok(None), "CD 3: do 3 prowincji na front");
        assert_eq!(game.fronts(1)[0].provinces, vec![a, b]);
        // Tempo: ta sama prowincja, więcej wojska na tym samym froncie – szybciej.
        let front: Vec<_> =
            game.frontier(n, a, None).into_iter().map(|(t, around, water)| (t, around, water, a)).collect();
        assert!(!front.is_empty());
        assert!(
            game.pace(2_000 * MILLI, 100, &front, &[]) < game.pace(60_000 * MILLI, 100, &front, &[]),
            "zmasowany atak szybszy"
        );
    }

    #[test]
    fn fuel_use_depends_on_military_supply_and_command_level() {
        let light = MILITARY.iter().find(|m| m.supply == Supply::LightMilitary).unwrap();
        assert_eq!((light.strength(2), light.strength(3)), (60, 80), "lekkie mocniejsze od CD 3");
        assert_eq!((light.fuel(2), light.fuel(3)), (100, 60), "i oszczędniejsze w paliwie");
        let heavy = MILITARY.iter().find(|m| m.supply == Supply::HeavyMilitary).unwrap();
        assert_eq!((heavy.strength(5), heavy.strength(6)), (140, 160));
        // Atak na gracza: bez zaopatrzenia pali mniej paliwa niż z lekkim.
        let setup = |supplied: bool| {
            let (mut game, n1, start) = rich_game(0);
            let n2 = game.owners().nation_of(2).unwrap();
            let target = land_neighbour(&game, start, |_| true);
            game.paint_province(target, n2);
            let e = &mut game.economy[n1 as usize - 1];
            e.troops = 200_000 * MILLI;
            e.supplies[Supply::Fuel as usize] = 1000 * MILLI;
            e.supplies[Supply::LightMilitary as usize] = if supplied { 1000 * MILLI } else { 0 };
            play(&mut game, vec![vec![attack(1, target, 100)]]);
            empty_turns(&mut game, 10);
            1000 * MILLI - game.economy(n1).supplies[Supply::Fuel as usize]
        };
        let (bare, light_fuel) = (setup(false), setup(true));
        assert!(bare > 0 && light_fuel > bare, "paliwo: bez zaopatrzenia {bare}, z lekkim {light_fuel}");
    }

    #[test]
    fn sea_landing_from_a_port_short_at_command_level_one() {
        let (mut game, n, start) = rich_game(0);
        game.economy[n as usize - 1].troops = 200_000 * MILLI;
        // Morze z co najmniej dwiema niczyimi prowincjami bez wspólnej granicy: jedną przemalowujemy na
        // nasze (brzeg), na drugą – desant.
        let land = |g: &Game, p: u16, q: u16| g.neighbours(p).iter().any(|l| l.province == q);
        let pick = (0..game.water.sea.len())
            .filter(|&b| game.water.sea[b])
            .find_map(|b| {
                let body = b as u32 + 1;
                let on: Vec<u16> = (1..=game.map().provinces.len() as u16)
                    .filter(|&q| {
                        game.water.province_bodies[q as usize - 1].contains(&body)
                            && game.map().provinces[q as usize - 1].tunnel == 0
                            && game.owners().shares(q).is_empty()
                            && !game
                                .neighbours(q)
                                .iter()
                                .any(|l| game.owners().shares(l.province).iter().any(|s| s.0 == n))
                    })
                    .collect();
                on.iter().find_map(|&p| on.iter().find(|&&q| q != p && !land(&game, p, q)).map(|&q| (p, q)))
            })
            .expect("dwie prowincje nad tym samym morzem");
        let (shore, target) = pick;
        game.paint_province(shore, n);
        assert_eq!(game.command_level(n), 1);
        assert_eq!(game.check_attack(1, &[target]), Err(AttackBlock::NoPort), "desant tylko z portu");
        game.buildings[shore as usize - 1].levels[Building::Port as usize] = 1;
        game.economy[n as usize - 1].supplies[Supply::Fuel as usize] = 0;
        let route = game.landing_route(n, &[target]).expect("trasa z portu");
        // CD 1: tylko krótki rejs; dalej – od CD 2.
        let command = if route.distance > war::MAX_LANDING_DISTANCE[1] {
            assert_eq!(game.check_attack(1, &[target]), Err(AttackBlock::TooFar), "CD 1: za daleko");
            set_command(&mut game, start, 2);
            2
        } else {
            1
        };
        assert_eq!(game.check_attack(1, &[target]), Err(AttackBlock::NoFuel), "paliwo na rejs");
        game.economy[n as usize - 1].supplies[Supply::Fuel as usize] = 1000 * MILLI;
        let landing = game.check_attack(1, &[target]).unwrap().expect("desant");
        assert_eq!(landing.port, shore);
        play(&mut game, vec![vec![attack(1, target, 100)]]);
        let a = &game.attacks[0];
        assert!(a.landing.is_some() && a.sailing > 0, "płynie");
        assert_eq!(a.troops, war::LANDING_TROOPS[command] * MILLI, "limit wojska desantu według CD");
        let used = 1000 * MILLI - game.economy(n).supplies[Supply::Fuel as usize];
        assert!(used >= landing.fuel(a.troops), "paliwo pobrane z góry: {used}");
        // Kropka na mapie: od portu do brzegu celu, ticki rejsu stałe przez cały rejs.
        let sailing = game.landings();
        assert_eq!(sailing.len(), 1);
        let w = game.map().width;
        assert_eq!(
            (sailing[0].from_x + sailing[0].from_y * w, sailing[0].to_x + sailing[0].to_y * w),
            (landing.from, landing.to)
        );
        assert_eq!(game.map().province[landing.to as usize], target);
        assert_eq!(sailing[0].arrive - sailing[0].depart, landing.sailing() - war::EMBARK_TICKS, "po zaokrętowaniu");
        // W rejsie nic nie zajmuje, po dopłynięciu – przyczółek.
        empty_turns(&mut game, landing.sailing() - 2);
        assert_eq!(game.landings(), sailing, "raport bez zmian w rejsie");
        assert!(!game.owners().shares(target).iter().any(|s| s.0 == n), "jeszcze płynie");
        empty_turns(&mut game, 20);
        assert!(game.owners().shares(target).iter().any(|s| s.0 == n), "przyczółek zajęty");
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
        assert_eq!(mine[0].provinces, vec![target]);
        assert_eq!(mine[0].defenders[0].player, 2);
        assert!(mine[0].occupation > 0.0 && mine[0].attacker.troops > 0.0);
        assert!(game.fronts(9).is_empty());
    }

    /// Wspólny front: atak i kontratak w sąsiednich prowincjach – obie strony dalej zajmują kafle, armie
    /// ścierają się (słabsza traci więcej), a każdy gracz widzi jeden wiersz z przeciwnikiem.
    #[test]
    fn opposing_fronts_clash_and_report_as_one() {
        let (mut game, n1, start) = rich_game(0);
        let n2 = game.owners().nation_of(2).unwrap();
        let target = land_neighbour(&game, start, |_| true);
        game.paint_province(target, n2);
        game.economy[n1 as usize - 1].troops = 120_000 * MILLI;
        game.economy[n2 as usize - 1].troops = 60_000 * MILLI;
        play(&mut game, vec![vec![attack(1, target, 50), attack(2, start, 50)]]);
        assert!(game.opposed(0, 1), "atak i kontratak to wspólny front");
        empty_turns(&mut game, 30);
        let (a, b) = (&game.attacks[0], &game.attacks[1]);
        assert!(a.taken > 0 && b.taken > 0, "obie strony zajmują kafle: {} / {}", a.taken, b.taken);
        assert!(a.taken > b.taken, "silniejszy idzie szybciej: {} / {}", a.taken, b.taken);
        // Samo starcie: przy równym zaopatrzeniu słabsza armia traci więcej.
        let (before_a, before_b) = (game.attacks[0].troops, game.attacks[1].troops);
        game.clash(0, 1);
        let (lost_a, lost_b) = (before_a - game.attacks[0].troops, before_b - game.attacks[1].troops);
        assert!(lost_a > 0 && lost_b > lost_a, "starcie: {lost_a} / {lost_b}");
        let (mine, theirs) = (game.fronts(1), game.fronts(2));
        assert_eq!((mine.len(), theirs.len()), (1, 1), "jeden wiersz na wspólny front");
        let opponent = mine[0].opponent.as_ref().expect("przeciwnik w raporcie");
        assert_eq!(opponent.side.player, 2);
        assert!(opponent.side.troops > 0.0 && opponent.side.tiles > 0);
        assert!(mine[0].provinces.contains(&start) && mine[0].provinces.contains(&target));
        assert_eq!(theirs[0].opponent.as_ref().map(|o| o.side.player), Some(1));
        // Bez kontrataku – zwykły front z obroną.
        play(&mut game, vec![vec![StampedIntent { player: 2, intent: Intent::CancelAttack { province: start } }]]);
        assert!(game.fronts(1)[0].opponent.is_none());
    }

    /// Front kończy się sam: armia poniżej `AUTO_END_PCT`% wysłanej albo zastój bez zdobyczy i posiłków –
    /// na wspólnym froncie dopiero, gdy stoją obie strony. Resztka wraca do kraju.
    #[test]
    fn fronts_end_when_bled_out_or_stalled() {
        let (mut game, n1, start) = rich_game(0);
        let n2 = game.owners().nation_of(2).unwrap();
        let target = land_neighbour(&game, start, |_| true);
        game.paint_province(target, n2);
        game.economy[n1 as usize - 1].troops = 100_000 * MILLI;
        game.economy[n2 as usize - 1].troops = 100_000 * MILLI;
        play(&mut game, vec![vec![attack(1, target, 50), attack(2, start, 50)]]);
        empty_turns(&mut game, 3);
        assert_eq!(game.attacks.len(), 2);
        // Zastój tylko po jednej stronie – front trwa.
        game.attacks[0].idle = war::STALL_TICKS;
        game.attacks[1].idle = 0;
        game.attacks[1].taken = 0;
        empty_turns(&mut game, 1);
        assert_eq!(game.attacks.len(), 2, "druga strona jeszcze walczy");
        // Obie strony stoją – oba fronty się kończą, wojsko wraca.
        for a in &mut game.attacks {
            a.idle = war::STALL_TICKS + 5;
        }
        empty_turns(&mut game, 1);
        assert!(game.attacks.is_empty(), "zastój po obu stronach");
        assert_eq!((game.economy(n1).deployed, game.economy(n2).deployed), (0, 0));
        // Wykrwawiony front: poniżej 2% wysłanego wojska.
        play(&mut game, vec![vec![attack(1, target, 50)]]);
        let sent = game.attacks[0].sent;
        let left = sent * war::AUTO_END_PCT / 100 - MILLI;
        let cut = game.attacks[0].troops - left;
        game.attacks[0].troops = left;
        game.economy[n1 as usize - 1].deployed -= cut;
        empty_turns(&mut game, 1);
        assert!(game.attacks.is_empty(), "wykrwawiony front się kończy");
        assert_eq!(game.economy(n1).deployed, 0, "resztka wraca do kraju");
        // Posiłki zerują zastój.
        play(&mut game, vec![vec![attack(1, target, 10)]]);
        game.attacks[0].idle = 100;
        play(&mut game, vec![vec![attack(1, target, 10)]]);
        assert!(game.attacks[0].idle <= 1);
    }

    /// Umocnienia: kafle prowincji kosztują atakującego więcej i front idzie wolniej; działają dla tego, kto
    /// kontroluje prowincję (> 50% kafli).
    #[test]
    fn fortifications_slow_the_attacker_down() {
        let setup = |forts: u8| {
            let (mut game, n1, start) = rich_game(0);
            let n2 = game.owners().nation_of(2).unwrap();
            let target = land_neighbour(&game, start, |_| true);
            game.paint_province(target, n2);
            game.buildings[target as usize - 1].levels[Building::Fortifications as usize] = forts;
            game.economy[n1 as usize - 1].troops = 30_000 * MILLI;
            (game, n1, n2, target)
        };
        let (mut open, n1, n2, target) = setup(0);
        let (mut fort, ..) = setup(2);
        let (plain, walled) =
            (open.province_report(1, target).unwrap().attack, fort.province_report(1, target).unwrap().attack);
        assert_eq!((plain.fortifications, walled.fortifications), (0, 2));
        assert!(walled.enemy_cost > plain.enemy_cost, "umocnienia podnoszą koszt kafla");
        assert_eq!(fort.fort_level(target, n2), 2);
        assert_eq!(fort.fort_level(target, n1), 0, "cudze umocnienia nie bronią atakującego");
        for g in [&mut open, &mut fort] {
            play(g, vec![vec![attack(1, target, 50)]]);
            empty_turns(g, 30);
        }
        let taken = |g: &Game| g.owners().shares(target).iter().find(|s| s.0 == n1).map_or(0, |s| s.1);
        assert!(taken(&fort) < taken(&open), "front wolniej: {} / {}", taken(&fort), taken(&open));
        assert!(fort.attacks[0].lost > open.attacks[0].lost * taken(&fort) as u64 / taken(&open).max(1) as u64);
        // Zdobyta przewaga w prowincji – umocnienia bronią zdobywcy.
        let area = fort.map().provinces[target as usize - 1].area;
        fort.paint_province(target, n1);
        assert_eq!(fort.owners().shares(target), &[(n1, area)]);
        assert_eq!(fort.fort_level(target, n1), 2);
    }

    /// Zmiana stolicy: do własnej prowincji, za złoto rosnące z każdą zmianą, najwyżej raz na rok gry;
    /// centrum dowodzenia przenosi się z poziomem.
    #[test]
    fn capital_moves_with_its_command_center() {
        let (mut game, n, start) = rich_game(10_000_000);
        let target = land_neighbour(&game, start, |_| true);
        let other = land_neighbour(&game, start, |q| q != target);
        game.paint_province(target, n);
        game.paint_province(other, n);
        set_command(&mut game, start, 3);
        let free = (1..=game.map().provinces.len() as u16).find(|&q| game.owners().shares(q).is_empty()).unwrap();
        let mv = |province| vec![vec![StampedIntent { player: 1, intent: Intent::MoveCapital { province } }]];
        assert_eq!(game.check_move_capital(1, start), Err(CapitalBlock::AlreadyCapital));
        assert_eq!(game.check_move_capital(1, free), Err(CapitalBlock::NotOwner));
        assert_eq!(game.check_move_capital(9, target), Err(CapitalBlock::NotPlaying));
        let report = game.province_report(1, target).unwrap().capital.unwrap();
        assert!(!report.here && report.blocked.is_none() && report.cost == (CAPITAL_MOVE_GOLD / 10) as f64);
        let gold = game.economy(n).gold;
        play(&mut game, mv(target));
        assert_eq!(game.capital(n), Some(target), "stolica przeniesiona");
        assert_eq!(game.economy(n).gold, gold - CAPITAL_MOVE_GOLD);
        assert_eq!(game.buildings(target).level(Building::CommandCenter), 3, "centrum dowodzenia z poziomem");
        assert_eq!(game.buildings(start).level(Building::CommandCenter), 0);
        assert_eq!(game.command_level(n), 3);
        assert!(game.province_report(1, target).unwrap().capital.unwrap().here);
        // Przerwa na rok gry, potem drożej.
        assert_eq!(game.check_move_capital(1, other), Err(CapitalBlock::Cooldown));
        assert!(game.province_report(1, other).unwrap().capital.unwrap().cooldown_days > 300.0);
        empty_turns(&mut game, CAPITAL_MOVE_TICKS);
        assert_eq!(game.check_move_capital(1, other), Ok(2 * CAPITAL_MOVE_GOLD));
        // Bez złota – nie.
        game.economy[n as usize - 1].gold = 0;
        assert_eq!(game.check_move_capital(1, other), Err(CapitalBlock::NoGold));
    }

    /// Księga złota: budowa, podatek i wydobycie trafiają do bieżącego miesiąca, podatek zamyka miesiąc.
    #[test]
    fn ledger_tracks_the_month() {
        let (mut game, n, start) = rich_game(1_000_000);
        play(&mut game, vec![vec![build(1, start, Building::Extraction)]]);
        let cost = buildings::cost(Building::Extraction, 1).0;
        assert_eq!(game.economy(n).month.buildings, cost);
        let tax_at = game.next_tax_tick().unwrap();
        while game.tick() <= tax_at {
            empty_turns(&mut game, 1);
        }
        let e = game.economy(n);
        assert!(e.last_month.tax > 0 && e.last_month.buildings == cost, "zamknięty miesiąc");
        assert_eq!(e.month.buildings, 0, "nowy miesiąc od zera");
        let r = &game.reports()[n as usize - 1];
        assert_eq!(r.last_month.tax, (e.last_month.tax / 10) as f64);
        assert!(r.tax_base > 0.0 && r.cap_tiles > 0.0 && r.growth_food_pct > 0.0);
        // Front: siła zaopatrzenia i paliwo w raporcie.
        let target = land_neighbour(&game, start, |q| game.owners().shares(q).is_empty());
        game.economy[n as usize - 1].troops = 50_000 * MILLI;
        play(&mut game, vec![vec![attack(1, target, 50)]]);
        empty_turns(&mut game, 3);
        let f = &game.fronts(1)[0];
        assert!(f.strength > 0.0 && f.fuel > 0.0, "ze startowym paliwem: siła {} paliwo {}", f.strength, f.fuel);
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
