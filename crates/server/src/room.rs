//! Aktor pokoju gry: jedyny właściciel stanu pokoju, komunikacja wyłącznie kanałami.
//!
//! Dwa etapy: **poczekalnia** (gracze dołączają, widzą się nawzajem – `ServerMsg::Lobby`, klienci
//! generują mapę w tle) i **gra** (od `ClientMsg::Start` gospodarza lecą tury). Gospodarz to gracz
//! obecny najdłużej (najniższe ID). Po starcie wraca tylko ktoś, kto był w poczekalni (ID karty
//! z biletu). Pokój w trybie otwartym (`auto_start`) startuje od razu – jak dawniej.

use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};

use game_core::protocol::{
    Catchup, ClientMsg, GameConfig, Intent, LobbyPlayer, PlayerId, ServerMsg, StampedIntent, Turn,
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoomStats {
    pub players: u16,
    pub tick: u32,
    pub started: bool,
}

pub type RoomHandle = mpsc::UnboundedSender<RoomCmd>;

struct Player {
    out: mpsc::UnboundedSender<String>,
    name: String,
}

struct Room {
    id: RoomId,
    rules: RoomRules,
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
}

/// Uruchamia aktora pokoju. Pokój pusty dłużej niż `idle` kończy się sam – kanał się zamyka,
/// a rejestr pokoi (`rooms.rs`) zauważa to przez `RoomHandle::is_closed`.
pub fn spawn(id: RoomId, rules: RoomRules, config: GameConfig, idle: Duration) -> RoomHandle {
    let (tx, mut rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        let mut room = Room::new(id, rules, config);
        info!(room = %room.id, seed = room.config.map.seed, "pokój utworzony");
        let mut interval = tokio::time::interval(Duration::from_millis(TURN_MS));
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    room.end_turn();
                    if room.empty_since.is_some_and(|t| t.elapsed() >= idle) {
                        info!(room = %room.id, "pokój zamknięty (pusty)");
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
    fn new(id: RoomId, rules: RoomRules, config: GameConfig) -> Self {
        Room {
            id,
            rules,
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
        }
    }

    fn handle(&mut self, cmd: RoomCmd) {
        match cmd {
            RoomCmd::Stats { reply } => {
                let _ = reply.send(RoomStats {
                    players: self.players.len() as u16,
                    tick: self.tick,
                    started: self.started,
                });
            }
            RoomCmd::Join { out, name, client, reply } => {
                let result = self.join(out, name, client);
                let _ = reply.send(result);
            }
            RoomCmd::Leave { player } => {
                self.players.remove(&player);
                info!(room = %self.id, player, online = self.players.len(), "gracz wyszedł");
                if self.players.is_empty() {
                    // Pusty pokój: od nowa (docelowo: zapis replayu), a po czasie `idle` – zamknięcie.
                    self.tick = 0;
                    self.pending.clear();
                    self.log.clear();
                    self.hashes.clear();
                    self.members.clear();
                    self.started = self.rules.auto_start;
                    self.empty_since = Some(Instant::now());
                } else {
                    self.broadcast_lobby();
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
        let member = client.as_ref().is_some_and(|c| self.members.contains(c));
        if self.started && client.is_some() && !member {
            return Err("gra w tym pokoju już trwa".into());
        }
        if self.players.len() >= self.rules.max_players as usize {
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
            }
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

    /// Gospodarz: gracz obecny najdłużej.
    fn host(&self) -> Option<PlayerId> {
        self.players.keys().next().copied()
    }

    fn broadcast_lobby(&self) {
        let lobby = ServerMsg::Lobby {
            players: self.players.iter().map(|(&id, p)| LobbyPlayer { id, name: p.name.clone() }).collect(),
            host: self.host(),
            started: self.started,
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
        let mut room = Room::new("test".into(), OPEN, config());
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
        let mut room = Room::new("r".into(), LOBBY, config());
        let (host, mut host_rx) = join(&mut room, "Ala", Some("karta-ali"));
        let (guest, mut guest_rx) = join(&mut room, "Ola", Some("karta-oli"));
        let (host, guest) = (host.unwrap(), guest.unwrap());

        // Poczekalnia: brak tur, obaj widzą skład i gospodarza.
        room.end_turn();
        assert!(room.log.is_empty());
        let lobby = messages(&mut guest_rx).into_iter().last().unwrap();
        let ServerMsg::Lobby { players, host: h, started } = lobby else { panic!("{lobby:?}") };
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
    fn full_room_and_strangers_after_start_are_refused() {
        let mut room = Room::new("r".into(), LOBBY, config());
        let (a, _a_rx) = join(&mut room, "A", Some("karta-a"));
        let (_b, _b_rx) = join(&mut room, "B", Some("karta-b"));
        assert_eq!(join(&mut room, "C", Some("karta-c")).0, Err("pokój jest pełny".into()));

        room.client(a.unwrap(), ClientMsg::Start);
        room.handle(RoomCmd::Leave { player: 1 });
        // Po starcie: obcy nie wejdzie nawet na wolne miejsce, uczestnik poczekalni wraca.
        assert_eq!(join(&mut room, "C", Some("karta-c")).0, Err("gra w tym pokoju już trwa".into()));
        assert!(join(&mut room, "B", Some("karta-b")).0.is_ok());
    }

    #[tokio::test(start_paused = true)]
    async fn empty_room_closes_after_idle_and_busy_room_does_not() {
        let idle = Duration::from_secs(60);
        let empty = spawn("empty".into(), OPEN, config(), idle);
        let busy = spawn("busy".into(), OPEN, config(), idle);
        let (out, _rx) = mpsc::unbounded_channel();
        let (reply, player) = oneshot::channel();
        busy.send(RoomCmd::Join { out, name: "Ala".into(), client: None, reply }).unwrap();
        player.await.unwrap().unwrap();

        tokio::time::sleep(idle + Duration::from_secs(1)).await;
        assert!(empty.is_closed());
        assert!(!busy.is_closed());
    }
}
