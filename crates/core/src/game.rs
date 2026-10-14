//! Stan gry i pętla ticków: mapa (niezmienna), licznik ticków, właściciele kafli, hash stanu.
//! Tu wejdą kolejne egzekucje intencji (walka, budynki, dyplomacja, boty).

use std::collections::BTreeMap;

use crate::{
    mapgen::{self, MapData, ProvinceLink},
    owners::Owners,
    protocol::{Catchup, GameConfig, Intent, PlayerId, SELECTION_TICKS, SERVER, Turn},
};

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
            #[cfg(feature = "debug")]
            Intent::Debug { .. } => {}
        }
    }

    fn begin_selection(&mut self, players: &[PlayerId], random: bool) {
        let mut players: Vec<PlayerId> = players.iter().copied().filter(|&p| p != SERVER).collect();
        players.sort_unstable();
        players.dedup();
        self.players = players;
        // Pierwszy tick po wyborze: ta tura + SELECTION_TICKS kolejnych.
        self.phase = Phase::Selection { end: self.tick + 1 + SELECTION_TICKS };
        if random {
            self.finish_selection();
        }
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
        self.phase = Phase::Playing;
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
