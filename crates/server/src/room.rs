//! Aktor pokoju gry: jedyny właściciel stanu pokoju, komunikacja wyłącznie kanałami.
//!
//! Dwa etapy: **poczekalnia** (gracze dołączają, widzą się nawzajem – `ServerMsg::Lobby`, klienci
//! generują mapę w tle) i **gra** (od `ClientMsg::Start` gospodarza lecą tury). Gospodarz to gracz
//! obecny najdłużej (najniższe ID). Wyjście gospodarza z poczekalni zamyka pokój (`ServerMsg::Closed`
//! do wszystkich); w trakcie gry gospodarzem zostaje następny gracz. Po starcie wraca tylko ktoś,
//! kto był w poczekalni (ID karty z biletu), a gdy grę opuszczą wszyscy – pokój się kończy.
//! Gospodarz może w poczekalni zmienić ustawienia pokoju (`ClientMsg::Settings`): inna mapa = nowy
//! `Welcome` dla wszystkich. Pokój w trybie otwartym (`auto_start`) startuje od razu.

use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};

use game_core::{
    lobby::{MAX_CONTINENTS, MAX_PLAYERS, MapSettings, RoomSettings, clean_name},
    protocol::{Catchup, ClientMsg, GameConfig, Intent, LobbyPlayer, PlayerId, ServerMsg, StampedIntent, Turn},
};
use tokio::{
    sync::{mpsc, oneshot},
    time::Instant,
};
use tracing::{info, warn};

pub const TURN_MS: u64 = 100;

pub type RoomId = String;

/// Zasady pokoju (z biletu albo trybu otwartego).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoomRules {
    /// Czy serwer przyjmuje intencje debugowe (flaga --dev).
    pub dev: bool,
    /// Gra rusza od razu, bez poczekalni (tryb otwarty).
    pub auto_start: bool,
    pub max_players: u16,
}

pub enum RoomCmd {
    Join {
        out: mpsc::UnboundedSender<String>,
        name: String,
        /// ID karty z biletu (brak w trybie otwartym).
        client: Option<String>,
        /// ID gracza albo powód odmowy.
        reply: oneshot::Sender<Result<PlayerId, String>>,
    },
    Leave {
        player: PlayerId,
    },
    Client {
        player: PlayerId,
        msg: ClientMsg,
    },
    /// Stan do heartbeatu (lobby).
    Stats {
        reply: oneshot::Sender<RoomStats>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct RoomStats {
    pub players: u16,
    pub tick: u32,
    pub started: bool,
    /// Ustawienia i konfiguracja gry – lobby zapisuje je przy każdym raporcie (gospodarz mógł je zmienić).
    pub settings: RoomSettings,
    pub config: GameConfig,
}

pub type RoomHandle = mpsc::UnboundedSender<RoomCmd>;

/// Zdarzenia pokoju dla rejestru i heartbeatu do lobby – zmiany widać na liście lobby od razu,
/// bez czekania na cykliczny raport.
#[derive(Clone, Debug, PartialEq)]
pub enum RoomEvent {
    /// Dołączenie, wyjście, start – bieżący stan pokoju.
    Changed { id: RoomId, stats: RoomStats },
    /// Pokój się skończył (gospodarz zamknął poczekalnię albo minął czas bezczynności).
    Closed { id: RoomId },
}

pub type EventSender = mpsc::UnboundedSender<RoomEvent>;

struct Player {
    out: mpsc::UnboundedSender<String>,
    name: String,
}

struct Room {
    id: RoomId,
    rules: RoomRules,
    /// Ustawienia pokoju (nazwa, mapa, limit graczy); `config` to wynikająca z nich konfiguracja gry.
    settings: RoomSettings,
    config: GameConfig,
    players: BTreeMap<PlayerId, Player>,
    next_id: PlayerId,
    started: bool,
    /// Karty, które były w poczekalni – po starcie tylko one mogą (wrócić) do gry.
    members: BTreeSet<String>,
    tick: u32,
    pending: Vec<StampedIntent>,
    /// Log tur = replay. Nowy (albo wracający) gracz dostaje go w `Welcome` do nadrobienia.
    log: Vec<Turn>,
    /// Pierwszy zgłoszony hash dla ticka; kolejne muszą się z nim zgadzać.
    hashes: BTreeMap<u32, u32>,
    /// Od kiedy pokój jest pusty (`None` – ktoś jest).
    empty_since: Option<Instant>,
    /// Pokój się skończył (gospodarz zamknął poczekalnię, wszyscy wyszli z gry) – aktor kończy się
    /// przy najbliższym obiegu pętli.
    closed: bool,
    events: Option<EventSender>,
}

/// Uruchamia aktora pokoju. Pokój pusty dłużej niż `idle` kończy się sam – kanał się zamyka,
/// a rejestr pokoi (`rooms.rs`) zauważa to przez `RoomHandle::is_closed`.
pub fn spawn(
    id: RoomId,
    rules: RoomRules,
    name: String,
    config: GameConfig,
    idle: Duration,
    events: Option<EventSender>,
) -> RoomHandle {
    let (tx, mut rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        let mut room = Room::new(id, rules, name, config);
        room.events = events;
        info!(room = %room.id, seed = room.config.map.seed, "pokój utworzony");
        let mut interval = tokio::time::interval(Duration::from_millis(TURN_MS));
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    room.end_turn();
                    if room.closed {
                        info!(room = %room.id, "pokój zamknięty (gospodarz zamknął lobby albo wszyscy wyszli z gry)");
                        break;
                    }
                    if room.empty_since.is_some_and(|t| t.elapsed() >= idle) {
                        info!(room = %room.id, "pokój zamknięty (pusty)");
                        room.emit(RoomEvent::Closed { id: room.id.clone() });
                        break;
                    }
                }
                cmd = rx.recv() => match cmd {
                    Some(cmd) => room.handle(cmd),
                    None => break,
                },
            }
        }
    });
    tx
}

impl Room {
    fn new(id: RoomId, rules: RoomRules, name: String, config: GameConfig) -> Self {
        let settings = RoomSettings {
            name,
            map: MapSettings::from_params(&config.map),
            seed: config.map.seed,
            max_players: rules.max_players,
        };
        Room {
            id,
            rules,
            settings,
            config,
            players: BTreeMap::new(),
            next_id: 0,
            started: rules.auto_start,
            members: BTreeSet::new(),
            tick: 0,
            pending: Vec::new(),
            log: Vec::new(),
            hashes: BTreeMap::new(),
            empty_since: Some(Instant::now()),
            closed: false,
            events: None,
        }
    }

    fn stats(&self) -> RoomStats {
        RoomStats {
            players: self.players.len() as u16,
            tick: self.tick,
            started: self.started,
            settings: self.settings.clone(),
            config: self.config.clone(),
        }
    }

    fn emit(&self, event: RoomEvent) {
        if let Some(events) = &self.events {
            let _ = events.send(event);
        }
    }

    fn changed(&self) {
        self.emit(RoomEvent::Changed { id: self.id.clone(), stats: self.stats() });
    }

    fn handle(&mut self, cmd: RoomCmd) {
        match cmd {
            RoomCmd::Stats { reply } => {
                let _ = reply.send(self.stats());
            }
            RoomCmd::Join { out, name, client, reply } => {
                let result = self.join(out, name, client);
                let _ = reply.send(result);
            }
            RoomCmd::Leave { player } => {
                let was_host = self.host() == Some(player);
                self.players.remove(&player);
                info!(room = %self.id, player, online = self.players.len(), "gracz wyszedł");
                if was_host && !self.started && !self.rules.auto_start {
                    // Gospodarz opuścił poczekalnię: koniec lobby dla wszystkich – od razu, także na liście.
                    let closed = ServerMsg::Closed { reason: "Gospodarz zamknął lobby.".into() };
                    self.broadcast(&serde_json::to_string(&closed).unwrap());
                    self.players.clear();
                    self.closed = true;
                    self.emit(RoomEvent::Closed { id: self.id.clone() });
                } else if self.players.is_empty() && !self.rules.auto_start {
                    // Wszyscy wyszli z trwającej gry: koniec gry (docelowo: zapis replayu).
                    self.closed = true;
                    self.emit(RoomEvent::Closed { id: self.id.clone() });
                } else if self.players.is_empty() {
                    // Pusty pokój trybu otwartego: od nowa (docelowo: zapis replayu), po czasie `idle` – zamknięcie.
                    self.tick = 0;
                    self.pending.clear();
                    self.log.clear();
                    self.hashes.clear();
                    self.members.clear();
                    self.started = self.rules.auto_start;
                    self.empty_since = Some(Instant::now());
                } else {
                    self.broadcast_lobby();
                    self.changed();
                }
            }
            RoomCmd::Client { player, msg } => self.client(player, msg),
        }
    }

    fn join(
        &mut self,
        out: mpsc::UnboundedSender<String>,
        name: String,
        client: Option<String>,
    ) -> Result<PlayerId, String> {
        if self.closed {
            return Err("lobby zostało zamknięte".into());
        }
        let member = client.as_ref().is_some_and(|c| self.members.contains(c));
        if self.started && client.is_some() && !member {
            return Err("gra w tym pokoju już trwa".into());
        }
        if self.players.len() >= self.settings.max_players as usize {
            return Err("pokój jest pełny".into());
        }
        if let Some(client) = client {
            self.members.insert(client);
        }
        let player = self.next_id;
        self.next_id += 1;
        let welcome = ServerMsg::Welcome { player, config: self.config.clone(), catchup: self.catchup() };
        let _ = out.send(serde_json::to_string(&welcome).unwrap());
        info!(room = %self.id, player, name, online = self.players.len() + 1, "gracz dołączył");
        self.players.insert(player, Player { out, name });
        self.empty_since = None;
        self.changed();
        self.broadcast_lobby();
        Ok(player)
    }

    fn client(&mut self, player: PlayerId, msg: ClientMsg) {
        match msg {
            ClientMsg::Join { name } => info!(room = %self.id, player, name, "gracz się przedstawił"),
            ClientMsg::Start => {
                if self.started || self.host() != Some(player) {
                    warn!(room = %self.id, player, "start odrzucony (nie gospodarz albo gra trwa)");
                    return;
                }
                self.started = true;
                info!(room = %self.id, player, players = self.players.len(), "start gry");
                self.broadcast_lobby();
                self.changed();
            }
            ClientMsg::Settings { settings } => self.change_settings(player, settings),
            ClientMsg::Intent { intent } => {
                if self.started && self.allowed(&intent) {
                    // Serwer tylko stempluje – logikę gry waliduje rdzeń u klientów.
                    self.pending.push(StampedIntent { player, intent });
                }
            }
            ClientMsg::Hash { tick, hash } => {
                let expected = *self.hashes.entry(tick).or_insert(hash);
                if expected != hash {
                    warn!(room = %self.id, player, tick, "desync");
                    self.send(player, &ServerMsg::Desync { tick });
                }
            }
        }
    }

    /// Nowe ustawienia od gospodarza w poczekalni. Limit graczy nie spada poniżej liczby obecnych;
    /// inna mapa (ustawienia albo seed) = nowa konfiguracja i nowy `Welcome` dla każdego gracza.
    fn change_settings(&mut self, player: PlayerId, s: RoomSettings) {
        if self.started || self.rules.auto_start || self.host() != Some(player) {
            warn!(room = %self.id, player, "zmiana ustawień odrzucona (nie gospodarz albo gra trwa)");
            return;
        }
        let Some(name) = clean_name(&s.name) else { return };
        let map = MapSettings { continents: s.map.continents.clamp(1, MAX_CONTINENTS), ..s.map };
        let max_players = s.max_players.clamp((self.players.len() as u16).max(1), MAX_PLAYERS);
        let new_map = map != self.settings.map || s.seed != self.settings.seed;
        self.settings = RoomSettings { name, map, seed: s.seed, max_players };
        if new_map {
            self.config = GameConfig { generator_version: self.config.generator_version, map: map.params(s.seed) };
            self.hashes.clear();
            for (&id, p) in &self.players {
                let welcome =
                    ServerMsg::Welcome { player: id, config: self.config.clone(), catchup: Catchup::default() };
                let _ = p.out.send(serde_json::to_string(&welcome).unwrap());
            }
        }
        info!(room = %self.id, player, seed = s.seed, new_map, "nowe ustawienia pokoju");
        self.broadcast_lobby();
        self.changed();
    }

    /// Gospodarz: gracz obecny najdłużej.
    fn host(&self) -> Option<PlayerId> {
        self.players.keys().next().copied()
    }

    fn broadcast_lobby(&self) {
        let lobby = ServerMsg::Lobby {
            players: self.players.iter().map(|(&id, p)| LobbyPlayer { id, name: p.name.clone() }).collect(),
            host: self.host(),
            started: self.started,
            settings: self.settings.clone(),
        };
        self.broadcast(&serde_json::to_string(&lobby).unwrap());
    }

    fn broadcast(&self, text: &str) {
        for p in self.players.values() {
            let _ = p.out.send(text.to_string());
        }
    }

    /// Przebieg gry do nadrobienia: numer bieżącej tury i tylko tury z intencjami (reszta jest pusta).
    fn catchup(&self) -> Catchup {
        Catchup { tick: self.tick, turns: self.log.iter().filter(|t| !t.intents.is_empty()).cloned().collect() }
    }

    fn allowed(&self, intent: &Intent) -> bool {
        match intent {
            #[cfg(feature = "debug")]
            Intent::Debug { .. } => self.rules.dev,
            _ => true,
        }
    }

    fn end_turn(&mut self) {
        if self.players.is_empty() || !self.started {
            return;
        }
        let turn = Turn { tick: self.tick, intents: std::mem::take(&mut self.pending) };
        self.broadcast(&serde_json::to_string(&ServerMsg::Turn { turn: turn.clone() }).unwrap());
        self.log.push(turn);
        self.tick += 1;
        // Stare hashe nie są już potrzebne.
        let keep_from = self.tick.saturating_sub(600);
        self.hashes = self.hashes.split_off(&keep_from);
    }

    fn send(&self, player: PlayerId, msg: &ServerMsg) {
        if let Some(p) = self.players.get(&player) {
            let _ = p.out.send(serde_json::to_string(msg).unwrap());
        }
    }
}

#[cfg(test)]
mod tests {
    use game_core::{game::Game, mapgen::MapGenParams, protocol::Intent};

    use super::*;

    const OPEN: RoomRules = RoomRules { dev: false, auto_start: true, max_players: u16::MAX };
    const LOBBY: RoomRules = RoomRules { dev: false, auto_start: false, max_players: 2 };

    fn config() -> GameConfig {
        let map = MapGenParams { width: 200, height: 160, ..Default::default() };
        GameConfig { generator_version: game_core::mapgen::GENERATOR_VERSION, map }
    }

    /// Dołącza gracza; zwraca wynik i kanał jego wiadomości.
    fn join(
        room: &mut Room,
        name: &str,
        client: Option<&str>,
    ) -> (Result<PlayerId, String>, mpsc::UnboundedReceiver<String>) {
        let (out, rx) = mpsc::unbounded_channel();
        (room.join(out, name.into(), client.map(String::from)), rx)
    }

    fn messages(rx: &mut mpsc::UnboundedReceiver<String>) -> Vec<ServerMsg> {
        std::iter::from_fn(|| rx.try_recv().ok()).map(|t| serde_json::from_str(&t).unwrap()).collect()
    }

    #[test]
    fn late_player_catches_up_to_the_same_state() {
        let mut room = Room::new("test".into(), OPEN, "Pokój".into(), config());
        let (_, _rx) = join(&mut room, "a", None);
        let mut early = Game::new(room.config.clone());
        for tick in 0..30u32 {
            if tick % 4 == 1 {
                room.pending.push(StampedIntent { player: 0, intent: Intent::Ping { nonce: tick } });
            }
            room.end_turn();
            early.apply_turn(room.log.last().unwrap());
        }
        let catchup = room.catchup();
        assert_eq!(catchup.tick, 30);
        assert!(catchup.turns.iter().all(|t| !t.intents.is_empty()));
        assert_eq!(catchup.turns.len(), room.log.iter().filter(|t| !t.intents.is_empty()).count());

        let mut late = Game::new(room.config.clone());
        late.catch_up(&catchup);
        assert_eq!(late.state_hash(), early.state_hash());
    }

    #[test]
    fn waiting_room_until_host_starts() {
        let mut room = Room::new("r".into(), LOBBY, "Pokój".into(), config());
        let (host, mut host_rx) = join(&mut room, "Ala", Some("karta-ali"));
        let (guest, mut guest_rx) = join(&mut room, "Ola", Some("karta-oli"));
        let (host, guest) = (host.unwrap(), guest.unwrap());

        // Poczekalnia: brak tur, obaj widzą skład i gospodarza.
        room.end_turn();
        assert!(room.log.is_empty());
        let lobby = messages(&mut guest_rx).into_iter().last().unwrap();
        let ServerMsg::Lobby { players, host: h, started, .. } = lobby else { panic!("{lobby:?}") };
        assert_eq!(players.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Ala", "Ola"]);
        assert_eq!((h, started), (Some(host), false));

        // Start może dać tylko gospodarz.
        room.client(guest, ClientMsg::Start);
        assert!(!room.started);
        room.client(host, ClientMsg::Start);
        assert!(room.started);
        room.end_turn();
        assert_eq!(room.log.len(), 1);
        assert!(messages(&mut host_rx).iter().any(|m| matches!(m, ServerMsg::Lobby { started: true, .. })));

        // Gospodarz wychodzi – gospodarzem zostaje następny.
        room.handle(RoomCmd::Leave { player: host });
        assert_eq!(room.host(), Some(guest));
    }

    #[test]
    fn host_leaving_the_waiting_room_closes_it_for_everyone() {
        let mut room = Room::new("r".into(), LOBBY, "Pokój".into(), config());
        let (host, _host_rx) = join(&mut room, "Ala", Some("karta-ali"));
        let (_guest, mut guest_rx) = join(&mut room, "Ola", Some("karta-oli"));
        messages(&mut guest_rx);
        room.handle(RoomCmd::Leave { player: host.unwrap() });
        let closed = messages(&mut guest_rx);
        assert!(matches!(closed.as_slice(), [ServerMsg::Closed { .. }]), "{closed:?}");
        assert!(room.closed && room.players.is_empty());
        assert_eq!(join(&mut room, "Ewa", Some("karta-ewy")).0, Err("lobby zostało zamknięte".into()));

        // Gość wychodzący z poczekalni niczego nie zamyka.
        let mut room = Room::new("r2".into(), LOBBY, "Pokój".into(), config());
        let (_host, _h) = join(&mut room, "Ala", Some("karta-ali"));
        let (guest, _g) = join(&mut room, "Ola", Some("karta-oli"));
        room.handle(RoomCmd::Leave { player: guest.unwrap() });
        assert!(!room.closed && room.players.len() == 1);
    }

    #[test]
    fn host_changes_settings_in_the_waiting_room() {
        let (tx, mut events) = mpsc::unbounded_channel();
        let mut room = Room::new("r".into(), LOBBY, "Pokój".into(), config());
        room.events = Some(tx);
        let (host, mut host_rx) = join(&mut room, "Ala", Some("karta-ali"));
        let (guest, mut guest_rx) = join(&mut room, "Ola", Some("karta-oli"));
        let (host, guest) = (host.unwrap(), guest.unwrap());
        messages(&mut host_rx);
        messages(&mut guest_rx);
        let small = MapSettings { size: game_core::lobby::MapSize::Small, ..MapSettings::default() };
        let wanted = RoomSettings { name: "  Nowa nazwa ".into(), map: small, seed: 77, max_players: 1 };

        // Gość nie może.
        room.client(guest, ClientMsg::Settings { settings: wanted.clone() });
        assert_eq!(room.settings.name, "Pokój");

        // Gospodarz: nazwa oczyszczona, limit nie mniejszy niż liczba graczy, nowa mapa = nowy Welcome.
        room.client(host, ClientMsg::Settings { settings: wanted.clone() });
        assert_eq!((room.settings.name.as_str(), room.settings.max_players), ("Nowa nazwa", 2));
        assert_eq!((room.config.map.seed, room.config.map.width), (77, 1000));
        let msgs = messages(&mut guest_rx);
        let ServerMsg::Welcome { player, config, .. } = &msgs[0] else { panic!("{msgs:?}") };
        assert_eq!((*player, config.map.seed), (guest, 77));
        assert!(matches!(&msgs[1], ServerMsg::Lobby { settings, .. } if settings.seed == 77));
        let last = std::iter::from_fn(|| events.try_recv().ok()).last();
        let Some(RoomEvent::Changed { stats, .. }) = last else { panic!("{last:?}") };
        assert_eq!((stats.settings.name.as_str(), stats.config.map.seed), ("Nowa nazwa", 77));

        // Sama nazwa – bez nowej mapy (bez Welcome).
        room.client(
            host,
            ClientMsg::Settings { settings: RoomSettings { name: "Inna".into(), ..room.settings.clone() } },
        );
        assert!(matches!(messages(&mut guest_rx).as_slice(), [ServerMsg::Lobby { .. }]));

        // Po starcie ustawienia są zamrożone.
        room.client(host, ClientMsg::Start);
        room.client(host, ClientMsg::Settings { settings: wanted });
        assert_eq!(room.settings.name, "Inna");
    }

    #[test]
    fn full_room_and_strangers_after_start_are_refused() {
        let mut room = Room::new("r".into(), LOBBY, "Pokój".into(), config());
        let (a, _a_rx) = join(&mut room, "A", Some("karta-a"));
        let (_b, _b_rx) = join(&mut room, "B", Some("karta-b"));
        assert_eq!(join(&mut room, "C", Some("karta-c")).0, Err("pokój jest pełny".into()));

        room.client(a.unwrap(), ClientMsg::Start);
        room.handle(RoomCmd::Leave { player: 1 });
        // Po starcie: obcy nie wejdzie nawet na wolne miejsce, uczestnik poczekalni wraca.
        assert_eq!(join(&mut room, "C", Some("karta-c")).0, Err("gra w tym pokoju już trwa".into()));
        assert!(join(&mut room, "B", Some("karta-b")).0.is_ok());
    }

    #[test]
    fn changes_are_reported_at_once_and_empty_game_closes() {
        let (tx, mut events) = mpsc::unbounded_channel();
        let mut room = Room::new("r".into(), LOBBY, "Pokój".into(), config());
        room.events = Some(tx);
        let mut next = || events.try_recv().ok();

        let (host, _h) = join(&mut room, "Ala", Some("karta-ali"));
        let (guest, _g) = join(&mut room, "Ola", Some("karta-oli"));
        let (host, guest) = (host.unwrap(), guest.unwrap());
        assert!(matches!(next(), Some(RoomEvent::Changed { stats: RoomStats { players: 1, .. }, .. })));
        assert!(matches!(next(), Some(RoomEvent::Changed { stats: RoomStats { players: 2, .. }, .. })));

        room.client(host, ClientMsg::Start);
        assert!(matches!(next(), Some(RoomEvent::Changed { stats: RoomStats { started: true, .. }, .. })));

        // Jeden gracz wychodzi z gry – gra trwa; wychodzą wszyscy – koniec gry.
        room.handle(RoomCmd::Leave { player: host });
        assert!(!room.closed);
        room.handle(RoomCmd::Leave { player: guest });
        assert!(room.closed && room.players.is_empty());
        let last = std::iter::from_fn(&mut next).last();
        assert_eq!(last, Some(RoomEvent::Closed { id: "r".into() }));
        assert!(join(&mut room, "Ola", Some("karta-oli")).0.is_err(), "zakończona gra nikogo nie wpuszcza");

        // Wyjście gospodarza z poczekalni – zdarzenie zamknięcia od razu.
        let (tx, mut events) = mpsc::unbounded_channel();
        let mut room = Room::new("r2".into(), LOBBY, "Pokój".into(), config());
        room.events = Some(tx);
        let (host, _h) = join(&mut room, "Ala", Some("karta-ali"));
        room.handle(RoomCmd::Leave { player: host.unwrap() });
        let last = std::iter::from_fn(|| events.try_recv().ok()).last();
        assert_eq!(last, Some(RoomEvent::Closed { id: "r2".into() }));
    }

    #[tokio::test(start_paused = true)]
    async fn empty_room_closes_after_idle_and_busy_room_does_not() {
        let idle = Duration::from_secs(60);
        let empty = spawn("empty".into(), OPEN, "Pokój".into(), config(), idle, None);
        let busy = spawn("busy".into(), OPEN, "Pokój".into(), config(), idle, None);
        let (out, _rx) = mpsc::unbounded_channel();
        let (reply, player) = oneshot::channel();
        busy.send(RoomCmd::Join { out, name: "Ala".into(), client: None, reply }).unwrap();
        player.await.unwrap().unwrap();

        tokio::time::sleep(idle + Duration::from_secs(1)).await;
        assert!(empty.is_closed());
        assert!(!busy.is_closed());
    }
}
