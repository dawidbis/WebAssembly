//! Stan gry i pętla ticków. Na razie tylko szkielet: mapa + licznik ticków + hash stanu.
//! Tu wejdą egzekucje intencji (walka, budynki, dyplomacja, boty).

use crate::{
    mapgen::{self, MapData},
    protocol::{GameConfig, Turn},
};

pub struct Game {
    config: GameConfig,
    map: MapData,
    tick: u32,
}

impl Game {
    pub fn new(config: GameConfig) -> Self {
        let map = mapgen::generate(&config.map);
        Self { config, map, tick: 0 }
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

    /// Hash stanu do wykrywania desynców (FNV-1a). Rozszerzaj o każde nowe pole stanu.
    pub fn state_hash(&self) -> u32 {
        let mut h: u32 = 0x811C_9DC5;
        let mut eat = |b: u8| h = (h ^ b as u32).wrapping_mul(0x0100_0193);
        self.tick.to_le_bytes().into_iter().for_each(&mut eat);
        self.map.terrain.iter().copied().for_each(&mut eat);
        h
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mapgen::{MapGenParams, GENERATOR_VERSION};

    fn config() -> GameConfig {
        GameConfig {
            generator_version: GENERATOR_VERSION,
            map: MapGenParams { width: 320, height: 200, ..Default::default() },
        }
    }

    #[test]
    fn two_games_stay_in_sync() {
        let (mut a, mut b) = (Game::new(config()), Game::new(config()));
        for tick in 0..50 {
            let turn = Turn { tick, intents: vec![] };
            a.apply_turn(&turn);
            b.apply_turn(&turn);
            assert_eq!(a.state_hash(), b.state_hash());
        }
    }

    #[test]
    fn protocol_json_roundtrip() {
        let msg = crate::protocol::ClientMsg::Hash { tick: 3, hash: 42 };
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(json, r#"{"type":"hash","tick":3,"hash":42}"#);
    }
}
