//! Cienka warstwa wasm-bindgen: tylko konwersja danych JS ↔ Rust, zero logiki gry.

use game_core::{
    game::Game,
    mapgen::{self, MapData, MapGenParams},
    protocol::{Catchup, GameConfig, Turn},
};
use wasm_bindgen::prelude::*;

fn js_err(e: impl std::fmt::Display) -> JsError {
    JsError::new(&e.to_string())
}

/// Domyślne parametry generatora jako JSON (jedno źródło prawdy: Rust).
#[wasm_bindgen]
pub fn default_map_params() -> String {
    serde_json::to_string(&MapGenParams::default()).unwrap()
}

#[wasm_bindgen]
pub fn generator_version() -> u32 {
    mapgen::GENERATOR_VERSION
}

/// Generuje mapę z parametrów (JSON zgodny z typem TS `MapGenParams`) – faza 1, bez prowincji.
/// Prowincje dolicza potem `computeProvinces()` (osobno, żeby teren pojawił się wcześniej).
#[wasm_bindgen]
pub fn generate_map(params_json: &str) -> Result<GeneratedMap, JsError> {
    let params: MapGenParams = serde_json::from_str(params_json).map_err(js_err)?;
    let (map, input) = mapgen::generate_base(&params);
    Ok(GeneratedMap { map, params, pending: Some(input) })
}

/// Wygenerowana mapa. Zostaje w pamięci wasm po skopiowaniu buforów do JS, bo może z niej
/// powstać gra (`WasmGame.fromMap`) – bez generowania mapy drugi raz.
#[wasm_bindgen]
pub struct GeneratedMap {
    map: MapData,
    params: MapGenParams,
    /// Dane fazy 2 – `None`, gdy prowincje są już policzone.
    pending: Option<mapgen::ProvinceInput>,
}

#[wasm_bindgen]
impl GeneratedMap {
    #[wasm_bindgen(getter)]
    pub fn width(&self) -> u32 {
        self.map.width
    }
    #[wasm_bindgen(getter)]
    pub fn height(&self) -> u32 {
        self.map.height
    }
    #[wasm_bindgen(getter, js_name = chunkCols)]
    pub fn chunk_cols(&self) -> u32 {
        self.map.chunk_cols
    }
    #[wasm_bindgen(getter, js_name = chunkRows)]
    pub fn chunk_rows(&self) -> u32 {
        self.map.chunk_rows
    }
    /// Metody z nazwami pól zwracają kopię bufora (nowy Uint8Array/Uint16Array) – mapa zostaje w Ruście.
    pub fn terrain(&self) -> Vec<u8> {
        self.map.terrain.clone()
    }
    pub fn shade(&self) -> Vec<u8> {
        self.map.shade.clone()
    }
    pub fn biome(&self) -> Vec<u8> {
        self.map.biome.clone()
    }
    /// 6 bajtów na kafel: [typ, σ1, σ2] dwóch warstw typów (wagi rodzajów – `kindWeights` w TS).
    #[wasm_bindgen(js_name = biomeLayers)]
    pub fn biome_layers(&self) -> Vec<u8> {
        self.map.biome_layers.clone()
    }
    /// 1 = lód morski (zamarznięty kafel oceanu przy lądolodzie).
    #[wasm_bindgen(js_name = seaIce)]
    pub fn sea_ice(&self) -> Vec<u8> {
        self.map.sea_ice.clone()
    }
    #[wasm_bindgen(js_name = biomeMix)]
    pub fn biome_mix(&self) -> Vec<u8> {
        self.map.biome_mix.clone()
    }
    #[wasm_bindgen(js_name = riverFlow)]
    pub fn river_flow(&self) -> Vec<u16> {
        self.map.river_flow.clone()
    }
    pub fn forest(&self) -> Vec<u8> {
        self.map.forest.clone()
    }
    /// Numer prowincji kafla (od 1), 0 = brak.
    pub fn province(&self) -> Vec<u16> {
        self.map.province.clone()
    }
    /// Lista prowincji jako JSON (typ TS `Province[]`).
    #[wasm_bindgen(js_name = provincesJson)]
    pub fn provinces_json(&self) -> String {
        serde_json::to_string(&self.map.provinces).unwrap()
    }
    #[wasm_bindgen(js_name = waterChunks)]
    pub fn water_chunks(&self) -> Vec<u8> {
        self.map.water_chunks.clone()
    }
    /// Faza 2: liczy prowincje (potem `province`, `provincesJson`, `statsJson`).
    #[wasm_bindgen(js_name = computeProvinces)]
    pub fn compute_provinces(&mut self) {
        if let Some(input) = self.pending.take() {
            mapgen::generate_provinces(&input).apply(&mut self.map);
        }
    }
    #[wasm_bindgen(js_name = statsJson)]
    pub fn stats_json(&self) -> String {
        serde_json::to_string(&self.map.stats).unwrap()
    }
}

/// Symulacja gry w workerze: wykonuje tury z serwera i liczy hash stanu.
#[wasm_bindgen]
pub struct WasmGame(Game);

#[wasm_bindgen]
impl WasmGame {
    /// Gra na mapie, która już jest (ta sama, co na ekranie) – bez generowania jej drugi raz.
    /// Przejmuje mapę: obiekt `map` jest potem w JS nieużywalny (także przy błędzie).
    /// Mapa musi być wygenerowana z `config.map` i mieć policzone prowincje.
    #[wasm_bindgen(js_name = fromMap)]
    pub fn from_map(config_json: &str, map: GeneratedMap) -> Result<WasmGame, JsError> {
        let config: GameConfig = serde_json::from_str(config_json).map_err(js_err)?;
        if config.generator_version != mapgen::GENERATOR_VERSION {
            return Err(JsError::new(&format!(
                "wersja generatora serwera ({}) inna niż klienta ({}) – odśwież stronę",
                config.generator_version,
                mapgen::GENERATOR_VERSION
            )));
        }
        if map.params != config.map {
            return Err(JsError::new("mapa nie pochodzi z konfiguracji gry"));
        }
        if map.pending.is_some() {
            return Err(JsError::new("prowincje mapy nie są jeszcze policzone"));
        }
        Ok(WasmGame(Game::from_map(config, map.map)))
    }
    /// Zaczyna grę od nowa na tej samej mapie (serwer przysłał nowe `Welcome` z tą samą konfiguracją).
    pub fn restart(&mut self) {
        self.0.restart();
    }
    /// Nadrabia przebieg gry sprzed dołączenia (JSON typu TS `Catchup`).
    #[wasm_bindgen(js_name = catchUp)]
    pub fn catch_up(&mut self, catchup_json: &str) -> Result<(), JsError> {
        let catchup: Catchup = serde_json::from_str(catchup_json).map_err(js_err)?;
        self.0.catch_up(&catchup);
        Ok(())
    }
    /// Wykonuje turę (JSON typu TS `Turn`). Tura spoza kolejki to błąd, a nie panika – gra zostaje cała.
    #[wasm_bindgen(js_name = applyTurn)]
    pub fn apply_turn(&mut self, turn_json: &str) -> Result<(), JsError> {
        let turn: Turn = serde_json::from_str(turn_json).map_err(js_err)?;
        if turn.tick != self.0.tick() {
            return Err(JsError::new(&format!("tura {} spoza kolejki (oczekiwana {})", turn.tick, self.0.tick())));
        }
        self.0.apply_turn(&turn);
        Ok(())
    }
    #[wasm_bindgen(getter)]
    pub fn tick(&self) -> u32 {
        self.0.tick()
    }
    #[wasm_bindgen(js_name = stateHash)]
    pub fn state_hash(&self) -> u32 {
        self.0.state_hash()
    }
}
