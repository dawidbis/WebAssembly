//! Wiadomości sieciowe i intencje. Jedyne źródło prawdy dla serwera, wasm i frontendu
//! (typy TS generuje `cargo types`).

use serde::{Deserialize, Serialize};

use crate::{buildings::Building, goods::Supply, lobby::RoomSettings, mapgen::MapGenParams};

pub type PlayerId = u16;

/// „Gracz” stemplujący intencje samego serwera (np. `Intent::BeginSelection`). Serwer nie przyjmuje
/// takich intencji od klientów, a rdzeń wykonuje je tylko z tym stemplem.
pub const SERVER: PlayerId = PlayerId::MAX;

/// Czas wyboru prowincji startowej w turach (tura = 100 ms → 20 s).
pub const SELECTION_TICKS: u32 = 200;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
    /// Wybór prowincji startowej (numer od 1) – tylko w czasie wyboru (`Phase::Selection`), tylko
    /// prowincja bez innego gracza i bez tunelu. Ponowny wybór przenosi start (poprzednia prowincja
    /// wraca do niczyich). Niepoprawny – przepada (walidacja w `Game::apply_turn`).
    ChooseStart { province: u16 },
    /// Tylko od serwera (stempel `SERVER`): wszyscy gracze mają gotową mapę – zaczyna się wybór
    /// prowincji startowych (`SELECTION_TICKS` tur), a przy `random` – od razu losowe starty.
    /// `players` – uczestnicy gry: kto nie wybierze, dostaje start losowy na koniec wyboru.
    BeginSelection { players: Vec<PlayerId>, random: bool },
    /// Suwak podziału nowej populacji: docelowy udział wojska w % (0–100, reszta – robotnicy).
    /// Dotyczy całej populacji: ludzie przechodzą między wojskiem a robotnikami z bezwładnością
    /// (`economy::SHIFT_PPM` populacji na turę).
    SetTroopShare { percent: u8 },
    /// Budowa kolejnego poziomu budynku w prowincji (docs/gdd/budynki.md): tylko właściciel prowincji
    /// (> 50% kafli), w grze właściwej, gdy prowincja nic nie buduje, są wolne punkty infrastruktury,
    /// złoto i materiały inżynieryjne (pobierane od razu). Walidacja: `Game::check_build`.
    Build { province: u16, building: Building },
    /// Anulowanie budowy w prowincji – właściciel odzyskuje `buildings::CANCEL_REFUND_PCT`% kosztu.
    CancelBuild { province: u16 },
    /// Co produkuje fabryka w prowincji (tylko właściciel; rodzaj odblokowany centrum dowodzenia). Zmiana
    /// w działającej fabryce – przezbrojenie (`factories::RETOOL_TICKS` tur bez produkcji).
    SetFactory { province: u16, supply: Supply },
    /// Wejście składnika `ingredient` receptury fabryki: `Some(k)` – ręcznie zamiennik `k` (zostaje,
    /// dopóki gracz go nie zmieni), `None` – automatycznie (najwięcej sztuk, do wyczerpania zapasu).
    SetFactoryInput { province: u16, ingredient: u8, option: Option<u8> },
    /// Atak na prowincje (jeden front): `percent` wojska z kraju (1–100) idzie na front, który zajmuje kafle prowincji
    /// stykające się z naszymi (niczyje i innych graczy). Trwający front z którąś z nich – dosłanie
    /// wojska (reszta dołącza). Walidacja: `Game::check_attack` (styk, tunel, limity centrum dowodzenia).
    Attack { provinces: Vec<u16>, percent: u8 },
    /// Odwołanie frontu z prowincją – wojsko wraca do kraju.
    CancelAttack { province: u16 },
    /// Przeniesienie stolicy (z centrum dowodzenia i jego poziomem) do własnej prowincji – za złoto, najwyżej
    /// raz na rok gry. Walidacja: `Game::check_move_capital`.
    MoveCapital { province: u16 },
    /// Rynek: bilans grupy surowców (`market::GROUP_NAMES`), −100 (import do nasycenia) … 0 … +100
    /// (eksport całej nadwyżki).
    SetTradeGroup { group: u8, percent: i8 },
    /// Rynek: bilans pojedynczego surowca (`None` – jak jego grupa).
    SetTradeResource { resource: u8, percent: Option<i8> },
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
    Join {
        name: String,
    },
    /// Start gry – tylko gospodarz pokoju, tylko w poczekalni.
    Start,
    /// Nowe ustawienia pokoju – tylko gospodarz, tylko w poczekalni. Inna mapa = nowy `Welcome` dla wszystkich.
    Settings {
        settings: RoomSettings,
    },
    Intent {
        intent: Intent,
    },
    /// Gra u klienta gotowa (mapa wygenerowana, przebieg nadrobiony). Gdy zgłoszą się wszyscy
    /// gracze, serwer zaczyna wybór prowincji startowych (`Intent::BeginSelection`).
    Ready,
    /// Hash stanu (`Game::state_hash`) po wykonaniu tury `tick`. Klient wysyła go co kilka tur.
    Hash {
        tick: u32,
        hash: u32,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ServerMsg {
    /// Pierwsza wiadomość po połączeniu: ID gracza, konfiguracja gry (z niej klient generuje mapę)
    /// i dotychczasowy przebieg gry do nadrobienia.
    Welcome {
        player: PlayerId,
        config: GameConfig,
        catchup: Catchup,
    },
    Turn {
        turn: Turn,
    },
    /// Hash gracza dla tego ticka różni się od hasha zgłoszonego wcześniej przez innego gracza.
    Desync {
        tick: u32,
    },
    /// Stan poczekalni po każdej zmianie: gracze, gospodarz, czy gra ruszyła. Tury lecą dopiero po starcie.
    Lobby {
        players: Vec<LobbyPlayer>,
        host: Option<PlayerId>,
        started: bool,
        /// Bieżące ustawienia pokoju (gospodarz może je zmieniać w poczekalni).
        settings: RoomSettings,
    },
    /// Serwer nie wpuścił gracza (pokój pełny, gra trwa bez niego) – połączenie zostanie zamknięte.
    Refused {
        reason: String,
    },
    /// Pokój się skończył (gospodarz opuścił poczekalnię) – wszyscy wracają do listy lobby.
    Closed {
        reason: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct LobbyPlayer {
    pub id: PlayerId,
    pub name: String,
}
