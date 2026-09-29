//! Aktor pokoju gry: jedyny właściciel stanu pokoju, komunikacja wyłącznie kanałami.

use std::{collections::BTreeMap, time::Duration};

use game_core::{
    mapgen::{MapGenParams, GENERATOR_VERSION},
    protocol::{ClientMsg, GameConfig, Intent, PlayerId, ServerMsg, StampedIntent, Turn},
};
use tokio::sync::{mpsc, oneshot};

pub const TURN_MS: u64 = 100;

pub enum RoomCmd {
    Join { out: mpsc::UnboundedSender<String>, reply: oneshot::Sender<PlayerId> },
    Leave { player: PlayerId },
    Client { player: PlayerId, msg: ClientMsg },
}

pub type RoomHandle = mpsc::UnboundedSender<RoomCmd>;

struct Room {
    /// Czy serwer przyjmuje intencje debugowe (flaga --dev).
    #[cfg_attr(not(feature = "debug"), allow(dead_code))]
    dev: bool,
    config: GameConfig,
    players: BTreeMap<PlayerId, mpsc::UnboundedSender<String>>,
    next_id: PlayerId,
    tick: u32,
    pending: Vec<StampedIntent>,
    /// Log tur = replay. Nowy/wracający gracz dostanie go do nadrobienia (TODO: Catchup).
    log: Vec<Turn>,
    /// Pierwszy zgłoszony hash dla ticka; kolejne muszą się z nim zgadzać.
    hashes: BTreeMap<u32, u32>,
}

pub fn spawn(dev: bool) -> RoomHandle {
    let (tx, mut rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        let mut room = Room {
            dev,
            config: GameConfig { generator_version: GENERATOR_VERSION, map: MapGenParams::default() },
            players: BTreeMap::new(),
            next_id: 0,
            tick: 0,
            pending: Vec::new(),
            log: Vec::new(),
            hashes: BTreeMap::new(),
        };
        let mut interval = tokio::time::interval(Duration::from_millis(TURN_MS));
        loop {
            tokio::select! {
                _ = interval.tick() => room.end_turn(),
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
    fn handle(&mut self, cmd: RoomCmd) {
        match cmd {
            RoomCmd::Join { out, reply } => {
                let player = self.next_id;
                self.next_id += 1;
                let welcome = ServerMsg::Welcome { player, config: self.config.clone() };
                let _ = out.send(serde_json::to_string(&welcome).unwrap());
                self.players.insert(player, out);
                let _ = reply.send(player);
                println!("gracz {player} dołączył ({} online)", self.players.len());
            }
            RoomCmd::Leave { player } => {
                self.players.remove(&player);
                println!("gracz {player} wyszedł ({} online)", self.players.len());
                if self.players.is_empty() {
                    // Pusty pokój: reset (docelowo: zapis replayu i zamknięcie gry).
                    self.tick = 0;
                    self.pending.clear();
                    self.log.clear();
                    self.hashes.clear();
                }
            }
            RoomCmd::Client { player, msg } => match msg {
                ClientMsg::Join { name } => println!("gracz {player} to {name}"),
                ClientMsg::Intent { intent } => {
                    if self.allowed(&intent) {
                        // Serwer tylko stempluje – logikę gry waliduje rdzeń u klientów.
                        self.pending.push(StampedIntent { player, intent });
                    }
                }
                ClientMsg::Hash { tick, hash } => {
                    let expected = *self.hashes.entry(tick).or_insert(hash);
                    if expected != hash {
                        eprintln!("DESYNC: gracz {player}, tick {tick}");
                        self.send(player, &ServerMsg::Desync { tick });
                    }
                }
            },
        }
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
