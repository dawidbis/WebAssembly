//! Aktor pokoju gry: jedyny właściciel stanu pokoju, komunikacja wyłącznie kanałami.

use std::{collections::BTreeMap, time::Duration};

use game_core::protocol::{Catchup, ClientMsg, GameConfig, Intent, PlayerId, ServerMsg, StampedIntent, Turn};
use tokio::{
    sync::{mpsc, oneshot},
    time::Instant,
};
use tracing::{info, warn};

pub const TURN_MS: u64 = 100;

pub type RoomId = String;

pub enum RoomCmd {
    Join { out: mpsc::UnboundedSender<String>, name: String, reply: oneshot::Sender<PlayerId> },
    Leave { player: PlayerId },
    Client { player: PlayerId, msg: ClientMsg },
}

pub type RoomHandle = mpsc::UnboundedSender<RoomCmd>;

struct Room {
    id: RoomId,
    /// Czy serwer przyjmuje intencje debugowe (flaga --dev).
    #[cfg_attr(not(feature = "debug"), allow(dead_code))]
    dev: bool,
    config: GameConfig,
    players: BTreeMap<PlayerId, mpsc::UnboundedSender<String>>,
    next_id: PlayerId,
    tick: u32,
    pending: Vec<StampedIntent>,
    /// Log tur = replay. Nowy (albo wracający) gracz dostaje go w `Welcome` do nadrobienia.
    log: Vec<Turn>,
    /// Pierwszy zgłoszony hash dla ticka; kolejne muszą się z nim zgadzać.
    hashes: BTreeMap<u32, u32>,
    /// Od kiedy pokój jest pusty (`None` – ktoś gra).
    empty_since: Option<Instant>,
}

/// Uruchamia aktora pokoju. Pokój pusty dłużej niż `idle` kończy się sam – kanał się zamyka,
/// a rejestr pokoi (`rooms.rs`) zauważa to przez `RoomHandle::is_closed`.
pub fn spawn(id: RoomId, dev: bool, config: GameConfig, idle: Duration) -> RoomHandle {
    let (tx, mut rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        let mut room = Room::new(id, dev, config);
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
    fn new(id: RoomId, dev: bool, config: GameConfig) -> Self {
        Room {
            id,
            dev,
            config,
            players: BTreeMap::new(),
            next_id: 0,
            tick: 0,
            pending: Vec::new(),
            log: Vec::new(),
            hashes: BTreeMap::new(),
            empty_since: Some(Instant::now()),
        }
    }

    fn handle(&mut self, cmd: RoomCmd) {
        match cmd {
            RoomCmd::Join { out, name, reply } => {
                let player = self.next_id;
                self.next_id += 1;
                let welcome = ServerMsg::Welcome { player, config: self.config.clone(), catchup: self.catchup() };
                let _ = out.send(serde_json::to_string(&welcome).unwrap());
                self.players.insert(player, out);
                self.empty_since = None;
                let _ = reply.send(player);
                info!(room = %self.id, player, name, online = self.players.len(), "gracz dołączył");
            }
            RoomCmd::Leave { player } => {
                self.players.remove(&player);
                info!(room = %self.id, player, online = self.players.len(), "gracz wyszedł");
                if self.players.is_empty() {
                    // Pusty pokój: reset (docelowo: zapis replayu), a po czasie `idle` – zamknięcie.
                    self.tick = 0;
                    self.pending.clear();
                    self.log.clear();
                    self.hashes.clear();
                    self.empty_since = Some(Instant::now());
                }
            }
            RoomCmd::Client { player, msg } => match msg {
                ClientMsg::Join { name } => info!(room = %self.id, player, name, "gracz się przedstawił"),
                ClientMsg::Intent { intent } => {
                    if self.allowed(&intent) {
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
            },
        }
    }

    /// Przebieg gry do nadrobienia: numer bieżącej tury i tylko tury z intencjami (reszta jest pusta).
    fn catchup(&self) -> Catchup {
        Catchup { tick: self.tick, turns: self.log.iter().filter(|t| !t.intents.is_empty()).cloned().collect() }
    }

    fn allowed(&self, intent: &Intent) -> bool {
        match intent {
            #[cfg(feature = "debug")]
            Intent::Debug { .. } => self.dev,
            _ => true,
        }
    }

    fn end_turn(&mut self) {
        if self.players.is_empty() {
            return;
        }
        let turn = Turn { tick: self.tick, intents: std::mem::take(&mut self.pending) };
        let text = serde_json::to_string(&ServerMsg::Turn { turn: turn.clone() }).unwrap();
        for out in self.players.values() {
            let _ = out.send(text.clone());
        }
        self.log.push(turn);
        self.tick += 1;
        // Stare hashe nie są już potrzebne.
        let keep_from = self.tick.saturating_sub(600);
        self.hashes = self.hashes.split_off(&keep_from);
    }

    fn send(&self, player: PlayerId, msg: &ServerMsg) {
        if let Some(out) = self.players.get(&player) {
            let _ = out.send(serde_json::to_string(msg).unwrap());
        }
    }
}

#[cfg(test)]
mod tests {
    use game_core::{game::Game, mapgen::MapGenParams, protocol::Intent};

    use super::*;

    fn room() -> Room {
        let map = MapGenParams { width: 200, height: 160, ..Default::default() };
        Room::new("test".into(), false, GameConfig { generator_version: game_core::mapgen::GENERATOR_VERSION, map })
    }

    #[test]
    fn late_player_catches_up_to_the_same_state() {
        let mut room = room();
        let (out, _rx) = mpsc::unbounded_channel();
        room.players.insert(0, out);
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

    #[tokio::test(start_paused = true)]
    async fn empty_room_closes_after_idle_and_busy_room_does_not() {
        let config = room().config;
        let idle = Duration::from_secs(60);

        let empty = spawn("empty".into(), false, config.clone(), idle);
        let busy = spawn("busy".into(), false, config, idle);
        let (out, _rx) = mpsc::unbounded_channel();
        let (reply, player) = oneshot::channel();
        busy.send(RoomCmd::Join { out, name: "Ala".into(), reply }).unwrap();
        player.await.unwrap();

        tokio::time::sleep(idle + Duration::from_secs(1)).await;
        assert!(empty.is_closed());
        assert!(!busy.is_closed());
    }
}
