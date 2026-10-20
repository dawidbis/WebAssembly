//! Cienka warstwa wasm-bindgen: tylko konwersja danych JS ↔ Rust, zero logiki gry.

use game_core::{
    game::{Game, Phase},
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

/// Receptury wszystkich rodzajów zaopatrzenia jako JSON (typ TS `SupplyRule[]`) – drzewko w oknie zapasów.
#[wasm_bindgen]
pub fn supply_rules() -> String {
    serde_json::to_string(&game_core::factories::rules()).unwrap()
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
    /// 1 = kafel lądu będący lodowcem (z `glacier`).
    pub fn glacier(&self) -> Vec<u8> {
        self.map.glacier.clone()
    }
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
    /// Faza 3 (ostatnie szlify): tunele i enklawy (potem `provincesJson`, `enclavesJson`,
    /// `statsJson`). `true` = enklawy zmieniły numerację prowincji (trzeba odczytać `province`).
    pub fn polish(&mut self) -> bool {
        mapgen::polish(&mut self.map, &self.params)
    }
    #[wasm_bindgen(js_name = enclavesJson)]
    pub fn enclaves_json(&self) -> String {
        serde_json::to_string(&self.map.enclaves).unwrap()
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
    /// Etap gry: 0 – czekanie na gotowość graczy, 1 – wybór prowincji startowych, 2 – gra.
    #[wasm_bindgen(getter)]
    pub fn phase(&self) -> u8 {
        match self.0.phase() {
            Phase::Waiting => 0,
            Phase::Selection { .. } => 1,
            Phase::Playing => 2,
        }
    }
    /// Pierwszy tick po czasie wyboru (0 poza wyborem) – odliczanie na ekranie to `selectionEnd - tick`.
    #[wasm_bindgen(getter, js_name = selectionEnd)]
    pub fn selection_end(&self) -> u32 {
        match self.0.phase() {
            Phase::Selection { end } => end,
            _ => 0,
        }
    }
    /// Gospodarka państw jako JSON (typ TS `NationReport[]`, `[n - 1]` – państwo `n`).
    #[wasm_bindgen(js_name = reportsJson)]
    pub fn reports_json(&self) -> String {
        serde_json::to_string(&self.0.reports()).unwrap()
    }
    /// Budynki prowincji z punktu widzenia gracza `viewer` jako JSON (typ TS `ProvinceReport`, `null` –
    /// nie ma takiej prowincji).
    #[wasm_bindgen(js_name = provinceReportJson)]
    pub fn province_report_json(&self, viewer: u16, province: u16) -> String {
        serde_json::to_string(&self.0.province_report(viewer, province)).unwrap()
    }
    /// Poziomy budynków wszystkich prowincji (`[(id - 1) * 8 + Building as usize]`).
    #[wasm_bindgen(js_name = buildingLevels)]
    pub fn building_levels(&self) -> Vec<u8> {
        self.0.building_levels()
    }
    /// Produkt fabryki każdej prowincji (`[id - 1]`: `Supply` + 1, 0 – bez fabryki).
    #[wasm_bindgen(js_name = factoryProducts)]
    pub fn factory_products(&self) -> Vec<u8> {
        self.0.factory_products()
    }
    /// Trwające budowy (`[(id - 1) * 3]`: rodzaj + 1 (0 – brak), poziom, postęp 0–255).
    #[wasm_bindgen(js_name = buildingConstructions)]
    pub fn building_constructions(&self) -> Vec<u8> {
        self.0.constructions()
    }
    #[wasm_bindgen(getter, js_name = buildingsRevision)]
    pub fn buildings_revision(&self) -> u32 {
        self.0.buildings_revision()
    }
    /// Zmiany właścicieli kafli od poprzedniego wywołania (`kafel << 8 | państwo`) – płynny front.
    #[wasm_bindgen(js_name = ownerChanges)]
    pub fn owner_changes(&mut self) -> Vec<u32> {
        self.0.take_owner_changes()
    }
    /// Czy gracz może zaatakować prowincje jednym frontem – JSON `{ blocked, landing, crossing }` (`blocked`:
    /// typ TS `AttackBlock` albo `null`; `landing`: `LandingReport` albo `null` – desant morski; `crossing`:
    /// `CrossingReport` albo `null` – przeprawa przez rzekę albo góry).
    #[wasm_bindgen(js_name = checkAttackJson)]
    pub fn check_attack_json(&self, player: u16, provinces: Vec<u16>) -> String {
        let result = self.0.check_attack(player, &provinces);
        let landing = match (&result, self.0.owners().nation_of(player)) {
            (Ok(Some(l)), Some(n)) => Some(self.0.landing_report(n, l)),
            _ => None,
        };
        let crossing = self.0.crossing_report(player, &provinces);
        serde_json::json!({ "blocked": result.err(), "landing": landing, "crossing": crossing }).to_string()
    }
    /// Rynek z punktu widzenia gracza `viewer` (JSON `MarketReport | null`).
    #[wasm_bindgen(js_name = marketJson)]
    pub fn market_json(&self, viewer: u16) -> String {
        serde_json::to_string(&self.0.market_report(viewer)).unwrap()
    }
    /// Port w prowincji (JSON `PortReport | null`): partnerzy handlu morskiego.
    #[wasm_bindgen(js_name = portJson)]
    pub fn port_json(&self, province: u16) -> String {
        serde_json::to_string(&self.0.port_report(province)).unwrap()
    }
    /// Kafle odciętych przyczółków wszystkich państw (rosnąco) – kreskowanie na mapie politycznej.
    #[wasm_bindgen(js_name = bridgeheadTiles)]
    pub fn bridgehead_tiles(&self) -> Vec<u32> {
        self.0.bridgehead_tiles()
    }
    /// Walczące fronty wszystkich państw (JSON `FrontMark[]`) – „ile vs ile” na mapie.
    #[wasm_bindgen(js_name = frontMarksJson)]
    pub fn front_marks_json(&self) -> String {
        serde_json::to_string(&self.0.front_marks()).unwrap()
    }
    /// Garnizony przyczółków gracza (JSON `GarrisonReport[]`).
    #[wasm_bindgen(js_name = garrisonsJson)]
    pub fn garrisons_json(&self, viewer: u16) -> String {
        serde_json::to_string(&self.0.garrisons(viewer)).unwrap()
    }
    /// Płynące desanty wszystkich państw (JSON `SailingReport[]`) – kropki na mapie.
    #[wasm_bindgen(js_name = landingsJson)]
    pub fn landings_json(&self) -> String {
        serde_json::to_string(&self.0.landings()).unwrap()
    }
    /// Aktywne fronty gracza `viewer` jako JSON (typ TS `FrontReport[]`).
    #[wasm_bindgen(js_name = frontsJson)]
    pub fn fronts_json(&self, viewer: u16) -> String {
        serde_json::to_string(&self.0.fronts(viewer)).unwrap()
    }
    /// Tick początku gry właściwej (0 przed nią – gra właściwa nigdy nie zaczyna się w ticku 0).
    #[wasm_bindgen(getter, js_name = playingSince)]
    pub fn playing_since(&self) -> u32 {
        self.0.playing_since().unwrap_or(0)
    }
    /// Tick najbliższego podatku (0 przed grą właściwą).
    #[wasm_bindgen(getter, js_name = nextTaxTick)]
    pub fn next_tax_tick(&self) -> u32 {
        self.0.next_tax_tick().unwrap_or(0)
    }
    /// Rośnie przy każdej zmianie właścicieli – worker wysyła je na ekran tylko po zmianie.
    #[wasm_bindgen(getter, js_name = ownersRevision)]
    pub fn owners_revision(&self) -> u32 {
        self.0.owners().revision()
    }
    /// Państwo na każdym kaflu (0 = niczyje) – kopia (nowy Uint8Array).
    pub fn owners(&self) -> Vec<u8> {
        self.0.owners().tiles().to_vec()
    }
    /// Gracze kierujący państwami: `nations[n - 1]` dla państwa `n`.
    pub fn nations(&self) -> Vec<u16> {
        self.0.owners().nations().to_vec()
    }
    /// Udziały w prowincjach jako JSON: `[id - 1]` → lista `[państwo, liczba kafli]`.
    #[wasm_bindgen(js_name = provinceSharesJson)]
    pub fn province_shares_json(&self) -> String {
        serde_json::to_string(self.0.owners().all_shares()).unwrap()
    }
}
