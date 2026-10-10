//! Stan gry i pętla ticków. Na razie tylko szkielet: mapa + licznik ticków + hash stanu.
//! Tu wejdą egzekucje intencji (walka, budynki, dyplomacja, boty).

use crate::{
    mapgen::{self, MapData},
    protocol::{Catchup, GameConfig, Turn},
};

pub struct Game {
    config: GameConfig,
    /// Mapa jest niezmienna – zmienny stan gry (właściciele, wykarczowany las…) trzymaj w osobnych
    /// polach inicjalizowanych w `from_map`, bo `restart` odtwarza grę z tej samej mapy.
    map: MapData,
    /// FNV-1a mapy liczony raz – `state_hash` go kontynuuje, więc nie przechodzi co turę przez całą mapę.
    map_hash: u32,
    tick: u32,
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

    /// Gra na gotowej mapie. `map` musi być wynikiem `mapgen::generate(&config.map)` (albo dwufazowo
    /// `generate_base` + `generate_provinces` – wynik jest ten sam), inaczej stan rozjedzie się z innymi graczami.
    pub fn from_map(config: GameConfig, map: MapData) -> Self {
        let mut h = Fnv::default();
        h.eat(map.terrain.iter().copied());
        h.eat(map.biome.iter().copied());
        h.eat(map.forest.iter().map(|&f| (f >= 128) as u8));
        h.eat(map.province.iter().flat_map(|p| p.to_le_bytes()));
        Self { config, map, map_hash: h.0, tick: 0 }
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

    /// Wykonuje jedną turę z serwera. Tury muszą przychodzić po kolei.
    pub fn apply_turn(&mut self, turn: &Turn) {
        assert_eq!(turn.tick, self.tick, "tura spoza kolejki");
        for _stamped in &turn.intents {
            // TODO: intencja → egzekucja (walidacja w rdzeniu, nie na serwerze).
        }
        self.tick += 1;
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
    /// prowincje), potem stan gry. Rozszerzaj o każde nowe pole stanu.
    pub fn state_hash(&self) -> u32 {
        let mut h = Fnv(self.map_hash);
        h.eat(self.tick.to_le_bytes());
        h.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        mapgen::{GENERATOR_VERSION, MapGenParams},
        protocol::{Intent, StampedIntent},
    };

    fn config() -> GameConfig {
        GameConfig {
            generator_version: GENERATOR_VERSION,
            map: MapGenParams { width: 320, height: 200, ..Default::default() },
        }
    }

    fn turn(tick: u32) -> Turn {
        // Co siódma tura z intencją – reszta pusta, jak zwykle w logu serwera.
        let intents = if tick % 7 == 3 {
            vec![StampedIntent { player: (tick % 2) as u16, intent: Intent::Ping { nonce: tick } }]
        } else {
            Vec::new()
        };
        Turn { tick, intents }
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
    fn two_phase_map_gives_the_same_game() {
        let config = config();
        let (mut map, input) = mapgen::generate_base(&config.map);
        mapgen::generate_provinces(&input).apply(&mut map);
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
        game.restart();
        assert_eq!(game.tick(), 0);
        assert_eq!(game.state_hash(), initial);
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
