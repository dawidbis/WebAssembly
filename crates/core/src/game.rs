//! Stan gry i pętla ticków: mapa (niezmienna), licznik ticków, właściciele kafli, hash stanu.
//! Tu wejdą kolejne egzekucje intencji (walka, budynki, dyplomacja, boty).

use crate::{
    mapgen::{self, MapData, ProvinceLink},
    owners::Owners,
    protocol::{Catchup, GameConfig, Intent, PlayerId, Turn},
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
        Self { config, map, map_hash: h.0, graph, bounds, tick: 0, owners }
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

    /// Wykonuje jedną turę z serwera. Tury muszą przychodzić po kolei. Intencje w kolejności
    /// z serwera; niepoprawna (np. zajęcie cudzej prowincji) przepada bez śladu w stanie gry.
    pub fn apply_turn(&mut self, turn: &Turn) {
        assert_eq!(turn.tick, self.tick, "tura spoza kolejki");
        for stamped in &turn.intents {
            self.execute(stamped.player, &stamped.intent);
        }
        self.tick += 1;
    }

    /// Walidacja i skutek intencji – w rdzeniu, nie na serwerze (serwer tylko stempluje gracza).
    fn execute(&mut self, player: PlayerId, intent: &Intent) {
        match *intent {
            Intent::Ping { .. } => {}
            Intent::Claim { province } => {
                self.claim(player, province);
            }
            #[cfg(feature = "debug")]
            Intent::Debug { .. } => {}
        }
    }

    /// Zajmuje całą niczyją prowincję dla gracza. `false` – prowincja nie istnieje albo ktoś ma
    /// w niej choć kafel (albo zabrakło numerów państw).
    fn claim(&mut self, player: PlayerId, province: u16) -> bool {
        if province == 0 || province as usize > self.map.provinces.len() || !self.owners.shares(province).is_empty() {
            return false;
        }
        let Some(nation) = self.owners.nation_for(player) else {
            return false;
        };
        let [x0, y0, x1, y1] = self.bounds[province as usize - 1];
        let w = self.map.width as usize;
        for y in y0 as usize..=y1 as usize {
            for i in y * w + x0 as usize..=y * w + x1 as usize {
                if self.map.province[i] == province {
                    self.owners.set(i, province, nation);
                }
            }
        }
        true
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
        h.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        mapgen::{GENERATOR_VERSION, MapGenParams},
        protocol::{Intent, PlayerId, StampedIntent},
    };

    fn config() -> GameConfig {
        GameConfig {
            generator_version: GENERATOR_VERSION,
            map: MapGenParams { width: 320, height: 200, ..Default::default() },
        }
    }

    fn turn(tick: u32) -> Turn {
        // Co siódma tura z intencjami – reszta pusta, jak zwykle w logu serwera. Zajęcia prowincji
        // zmieniają stan (część przepada – prowincja już zajęta).
        let intents = if tick % 7 == 3 {
            let player = (tick % 2) as u16;
            vec![
                StampedIntent { player, intent: Intent::Ping { nonce: tick } },
                StampedIntent { player, intent: Intent::Claim { province: (1 + tick / 7 % 4) as u16 } },
            ]
        } else {
            Vec::new()
        };
        Turn { tick, intents }
    }

    fn claim(tick: u32, player: PlayerId, province: u16) -> Turn {
        Turn { tick, intents: vec![StampedIntent { player, intent: Intent::Claim { province } }] }
    }

    #[test]
    fn claim_takes_a_whole_unowned_province() {
        let mut game = Game::new(config());
        let area = game.map().provinces[1].area;
        game.apply_turn(&claim(0, 7, 2));
        assert_eq!(game.owners().shares(2), &[(1, area)]);
        assert_eq!(game.owners().nations(), &[7]);
        let tiles = game.map().province.iter().enumerate().filter(|&(_, &p)| p == 2);
        assert!(tiles.clone().all(|(i, _)| game.owners().owner(i) == Some(7)));
        let owned = game.owners().tiles().iter().filter(|&&n| n != 0).count();
        assert_eq!(owned, tiles.count(), "poza prowincją nic się nie zmienia");

        // Cudza prowincja – intencja przepada; niczyja – to samo państwo gracza.
        let before = game.state_hash();
        game.apply_turn(&claim(1, 3, 2));
        assert_eq!((game.owners().shares(2), game.owners().nations()), (&[(1, area)][..], &[7][..]));
        assert_ne!(game.state_hash(), before, "tick i tak idzie dalej");
        game.apply_turn(&claim(2, 7, 1));
        assert_eq!(game.owners().shares(1), &[(1, game.map().provinces[0].area)]);
        game.apply_turn(&claim(3, 3, 3));
        assert_eq!(game.owners().shares(3)[0].0, 2, "nowy gracz – nowe państwo");
        assert_eq!(game.owners().nation_of(3), Some(2));
        // Prowincja spoza mapy.
        let count = game.map().provinces.len() as u16;
        game.apply_turn(&claim(4, 3, count + 1));
        game.apply_turn(&claim(5, 3, 0));
        assert_eq!(game.owners().nations(), &[7, 3]);
    }

    #[test]
    fn owners_are_in_the_state_hash() {
        let (mut a, mut b) = (Game::new(config()), Game::new(config()));
        a.apply_turn(&claim(0, 1, 5));
        b.apply_turn(&Turn { tick: 0, intents: Vec::new() });
        assert_ne!(a.state_hash(), b.state_hash());
        // Ta sama prowincja, inny gracz – inny hash.
        let mut c = Game::new(config());
        c.apply_turn(&claim(0, 2, 5));
        assert_ne!(a.state_hash(), c.state_hash());
        // Te same zajęcia w tej samej kolejności – ten sam hash.
        let mut d = Game::new(config());
        d.apply_turn(&claim(0, 1, 5));
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
