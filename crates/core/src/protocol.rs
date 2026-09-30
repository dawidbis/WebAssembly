//! Wiadomości sieciowe i intencje. Jedyne źródło prawdy dla serwera, wasm i frontendu
//! (typy TS generuje `cargo types`).

use serde::{Deserialize, Serialize};

use crate::mapgen::MapGenParams;

pub type PlayerId = u16;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct GameConfig {
    pub generator_version: u32,
    pub map: MapGenParams,
}

/// Akcja gracza. Prawdziwe intencje (atak, budowa, sojusz…) dojdą razem z mechanikami.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Intent {
    /// Placeholder, żeby przetestować przepływ intencji end-to-end.
    Ping { nonce: u32 },
    #[cfg(feature = "debug")]
    Debug { cmd: DebugCmd },
}

#[cfg(feature = "debug")]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum DebugCmd {
    RegenerateMap { params: MapGenParams },
    SetPaused { paused: bool },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct StampedIntent {
    /// Nadaje serwer na podstawie połączenia – klient nie może się podszyć.
    pub player: PlayerId,
    pub intent: Intent,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct Turn {
    pub tick: u32,
    pub intents: Vec<StampedIntent>,
}

/// Przebieg gry przed dołączeniem gracza: rozegrano `tick` tur (ticki `0..tick`), z czego w `turns`
/// są tylko te z intencjami (rosnąco) – pozostałe były puste. Klient odtwarza je
/// (`Game::catch_up`), zanim wykona kolejne tury – pierwsza z nich ma tick równy `tick`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct Catchup {
    pub tick: u32,
    pub turns: Vec<Turn>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ClientMsg {
    Join { name: String },
    Intent { intent: Intent },
    /// Hash stanu (`Game::state_hash`) po wykonaniu tury `tick`. Klient wysyła go co kilka tur.
    Hash { tick: u32, hash: u32 },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ServerMsg {
    /// Pierwsza wiadomość po połączeniu: ID gracza, konfiguracja gry (z niej klient generuje mapę)
    /// i dotychczasowy przebieg gry do nadrobienia.
    Welcome { player: PlayerId, config: GameConfig, catchup: Catchup },
    Turn { turn: Turn },
    /// Hash gracza dla tego ticka różni się od hasha zgłoszonego wcześniej przez innego gracza.
    Desync { tick: u32 },
}
