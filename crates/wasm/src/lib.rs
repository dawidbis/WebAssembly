//! Cienka warstwa wasm-bindgen: tylko konwersja danych JS ↔ Rust, zero logiki gry.

use game_core::{
    game::Game,
    mapgen::{self, MapData, MapGenParams},
    protocol::{GameConfig, Turn},
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

/// Generuje mapę z parametrów (JSON zgodny z typem TS `MapGenParams`).
#[wasm_bindgen]
pub fn generate_map(params_json: &str) -> Result<GeneratedMap, JsError> {
    let params: MapGenParams = serde_json::from_str(params_json).map_err(js_err)?;
    Ok(GeneratedMap(mapgen::generate(&params)))
}

#[wasm_bindgen]
pub struct GeneratedMap(MapData);

#[wasm_bindgen]
impl GeneratedMap {
    #[wasm_bindgen(getter)]
    pub fn width(&self) -> u32 {
        self.0.width
    }
    #[wasm_bindgen(getter)]
    pub fn height(&self) -> u32 {
        self.0.height
    }
    #[wasm_bindgen(getter, js_name = chunkCols)]
    pub fn chunk_cols(&self) -> u32 {
        self.0.chunk_cols
    }
    #[wasm_bindgen(getter, js_name = chunkRows)]
    pub fn chunk_rows(&self) -> u32 {
        self.0.chunk_rows
    }
    /// Metody `take_*` przenoszą bufor do JS (Uint8Array) bez dodatkowej kopii po stronie Rusta.
    #[wasm_bindgen(js_name = takeTerrain)]
    pub fn take_terrain(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.0.terrain)
    }
    #[wasm_bindgen(js_name = takeShade)]
    pub fn take_shade(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.0.shade)
    }
    #[wasm_bindgen(js_name = takeBiome)]
    pub fn take_biome(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.0.biome)
    }
    #[wasm_bindgen(js_name = takeBiomeOther)]
    pub fn take_biome_other(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.0.biome_other)
    }
    #[wasm_bindgen(js_name = takeBiomeMix)]
    pub fn take_biome_mix(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.0.biome_mix)
    }
    #[wasm_bindgen(js_name = takeRiverFlow)]
    pub fn take_river_flow(&mut self) -> Vec<u16> {
        std::mem::take(&mut self.0.river_flow)
    }
    #[wasm_bindgen(js_name = takeForest)]
    pub fn take_forest(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.0.forest)
    }
    #[wasm_bindgen(js_name = takeFertility)]
    pub fn take_fertility(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.0.fertility)
    }
    /// Numer prowincji kafla (od 1), 0 = brak.
    #[wasm_bindgen(js_name = takeProvince)]
    pub fn take_province(&mut self) -> Vec<u16> {
        std::mem::take(&mut self.0.province)
    }
    /// Lista prowincji jako JSON (typ TS `Province[]`).
    #[wasm_bindgen(js_name = provincesJson)]
    pub fn provinces_json(&self) -> String {
        serde_json::to_string(&self.0.provinces).unwrap()
    }
    #[wasm_bindgen(js_name = takeWaterChunks)]
    pub fn take_water_chunks(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.0.water_chunks)
    }
    #[wasm_bindgen(js_name = statsJson)]
    pub fn stats_json(&self) -> String {
        serde_json::to_string(&self.0.stats).unwrap()
    }
}

/// Uchwyt do symulacji dla workera. Na razie nieużywany przez UI – gotowy na pętlę tur.
#[wasm_bindgen]
pub struct WasmGame(Game);

#[wasm_bindgen]
impl WasmGame {
    #[wasm_bindgen(constructor)]
    pub fn new(config_json: &str) -> Result<WasmGame, JsError> {
        let config: GameConfig = serde_json::from_str(config_json).map_err(js_err)?;
        Ok(WasmGame(Game::new(config)))
    }
    #[wasm_bindgen(js_name = applyTurn)]
    pub fn apply_turn(&mut self, turn_json: &str) -> Result<(), JsError> {
        let turn: Turn = serde_json::from_str(turn_json).map_err(js_err)?;
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
