//! Rejestr pokoi: aktor z mapą `RoomId → RoomHandle`. Tworzy pokój przy pierwszym bilecie
//! (z konfiguracją z biletu), pilnuje jednorazowości biletów i zapomina zamknięte pokoje.
//! Zbiera zdarzenia pokoi (`RoomEvent`) i przekazuje je obserwatorom (heartbeat do lobby);
//! ID zamkniętego pokoju z lobby pamięta przez `CLOSED_MEMORY_SECS` – spóźniony bilet go nie odtworzy.

use std::{collections::BTreeMap, time::Duration};

use game_core::protocol::GameConfig;
use tokio::sync::{mpsc, oneshot};
use tracing::warn;

use crate::room::{self, EventSender, RoomEvent, RoomHandle, RoomId, RoomRules};

/// Tyle sekund ID zamkniętego pokoju z lobby nie może być użyte ponownie (bilet żyje 60 s).
const CLOSED_MEMORY_SECS: u64 = 120;
/// Najwięcej pokoi naraz (pamięć serwera; lobby i tak ogranicza zakładanie na adres IP).
pub const MAX_ROOMS: usize = 200;

/// Bilet zużyty przy otwarciu pokoju: ID i czas wygaśnięcia (sekundy uniksowe).
pub struct TicketUse {
    pub jti: String,
    pub exp: u64,
}

#[derive(Debug, PartialEq)]
pub enum OpenError {
    /// Bilet o tym `jti` już raz wpuścił gracza.
    TicketReused,
    /// Pokój istnieje z inną konfiguracją gry niż w bilecie.
    ConfigMismatch,
    /// Pokój został niedawno zamknięty (gospodarz zamknął lobby, koniec gry).
    Closed,
    /// Serwer ma już `MAX_ROOMS` pokoi.
    TooManyRooms,
}

pub enum RegistryCmd {
    Open {
        id: RoomId,
        config: Box<GameConfig>,
        /// Zasady nowego pokoju (istniejący pokój zostaje przy swoich).
        rules: RoomRules,
        ticket: Option<TicketUse>,
        reply: oneshot::Sender<Result<RoomHandle, OpenError>>,
    },
    /// Liczba otwartych pokoi (do `/health`).
    Count { reply: oneshot::Sender<usize> },
    /// Otwarte pokoje (do heartbeatu).
    Snapshot { reply: oneshot::Sender<Vec<(RoomId, RoomHandle)>> },
    /// Subskrypcja zdarzeń pokoi.
    Watch { tx: EventSender },
}

pub type RegistryHandle = mpsc::UnboundedSender<RegistryCmd>;

struct Registry {
    idle: Duration,
    rooms: BTreeMap<RoomId, (GameConfig, RoomRules, RoomHandle)>,
    /// Zużyte bilety (`jti` → `exp`); po wygaśnięciu i tak nie przejdą weryfikacji, więc są usuwane.
    used: BTreeMap<String, u64>,
    /// Niedawno zamknięte pokoje z lobby (`id` → do kiedy pamiętać).
    closed: BTreeMap<RoomId, u64>,
    /// Kanał zdarzeń dawany każdemu nowemu pokojowi.
    events: EventSender,
    watchers: Vec<EventSender>,
}

impl Registry {
    fn new(idle: Duration, events: EventSender) -> Self {
        Registry {
            idle,
            rooms: BTreeMap::new(),
            used: BTreeMap::new(),
            closed: BTreeMap::new(),
            events,
            watchers: Vec::new(),
        }
    }
}

pub fn spawn(idle: Duration) -> RegistryHandle {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let (events_tx, mut events_rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        let mut registry = Registry::new(idle, events_tx);
        loop {
            tokio::select! {
                cmd = rx.recv() => match cmd {
                    Some(cmd) => registry.handle(cmd, game_ticket::now_secs()),
                    None => break,
                },
                Some(event) = events_rx.recv() => registry.event(event, game_ticket::now_secs()),
            }
        }
    });
    tx
}

impl Registry {
    fn handle(&mut self, cmd: RegistryCmd, now: u64) {
        // Pokoje kończą się same (pusty pokój po czasie `idle`) – zamknięty kanał = pokoju nie ma.
        self.rooms.retain(|_, (_, _, handle)| !handle.is_closed());
        match cmd {
            RegistryCmd::Open { id, config, rules, ticket, reply } => {
                let _ = reply.send(self.open(id, *config, rules, ticket, now));
            }
            RegistryCmd::Count { reply } => {
                let _ = reply.send(self.rooms.len());
            }
            RegistryCmd::Snapshot { reply } => {
                let _ =
                    reply.send(self.rooms.iter().map(|(id, (_, _, handle))| (id.clone(), handle.clone())).collect());
            }
            RegistryCmd::Watch { tx } => self.watchers.push(tx),
        }
    }

    fn event(&mut self, event: RoomEvent, now: u64) {
        if let RoomEvent::Closed { id } = &event {
            // Pokój z lobby nie wróci pod tym samym ID (tryb otwarty – `default` – może powstać od nowa).
            if self.rooms.get(id).is_some_and(|(_, rules, _)| !rules.auto_start) {
                self.closed.insert(id.clone(), now + CLOSED_MEMORY_SECS);
            }
            self.rooms.remove(id);
        }
        self.watchers.retain(|w| w.send(event.clone()).is_ok());
    }

    fn open(
        &mut self,
        id: RoomId,
        config: GameConfig,
        rules: RoomRules,
        ticket: Option<TicketUse>,
        now: u64,
    ) -> Result<RoomHandle, OpenError> {
        self.closed.retain(|_, until| *until >= now);
        if self.closed.contains_key(&id) {
            return Err(OpenError::Closed);
        }
        if let Some(ticket) = ticket {
            self.used.retain(|_, exp| *exp >= now);
            if self.used.contains_key(&ticket.jti) {
                warn!(room = %id, jti = %ticket.jti, "bilet użyty drugi raz");
                return Err(OpenError::TicketReused);
            }
            self.used.insert(ticket.jti, ticket.exp);
        }
        if let Some((existing, _, handle)) = self.rooms.get(&id) {
            if *existing != config {
                warn!(room = %id, "bilet z inną konfiguracją niż pokój");
                return Err(OpenError::ConfigMismatch);
            }
            return Ok(handle.clone());
        }
        if self.rooms.len() >= MAX_ROOMS {
            warn!(room = %id, "limit pokoi");
            return Err(OpenError::TooManyRooms);
        }
        let handle = room::spawn(id.clone(), rules, config.clone(), self.idle, Some(self.events.clone()));
        self.rooms.insert(id, (config, rules, handle.clone()));
        Ok(handle)
    }
}

#[cfg(test)]
mod tests {
    use game_core::mapgen::{GENERATOR_VERSION, MapGenParams};

    use super::*;

    fn config(seed: u32) -> GameConfig {
        GameConfig { generator_version: GENERATOR_VERSION, map: MapGenParams { seed, ..Default::default() } }
    }

    const RULES: RoomRules = RoomRules { dev: false, auto_start: false, max_players: 8 };

    fn registry() -> Registry {
        Registry::new(Duration::from_secs(60), mpsc::unbounded_channel().0)
    }

    #[tokio::test]
    async fn closed_lobby_room_cannot_be_reopened_for_a_while() {
        let mut reg = registry();
        let (watch, mut seen) = mpsc::unbounded_channel();
        reg.handle(RegistryCmd::Watch { tx: watch }, 0);
        reg.open("a".into(), config(1), RULES, None, 0).unwrap();
        reg.event(RoomEvent::Closed { id: "a".into() }, 10);
        assert_eq!(seen.try_recv().unwrap(), RoomEvent::Closed { id: "a".into() }, "obserwator dostaje zdarzenie");
        assert_eq!(reg.open("a".into(), config(1), RULES, None, 20).unwrap_err(), OpenError::Closed);
        // Po czasie pamięci ID wolne (bilety i tak wygasły).
        reg.open("a".into(), config(1), RULES, None, 10 + CLOSED_MEMORY_SECS + 1).unwrap();

        // Pokój trybu otwartego zamknięty z bezczynności powstaje od nowa od razu.
        let open = RoomRules { dev: false, auto_start: true, max_players: u16::MAX };
        reg.open("default".into(), config(1), open, None, 0).unwrap();
        reg.event(RoomEvent::Closed { id: "default".into() }, 10);
        reg.open("default".into(), config(1), open, None, 11).unwrap();
    }

    fn ticket(jti: &str, exp: u64) -> Option<TicketUse> {
        Some(TicketUse { jti: jti.into(), exp })
    }

    #[tokio::test]
    async fn rooms_are_separate_and_reused_by_id() {
        let mut reg = registry();
        let a1 = reg.open("a".into(), config(1), RULES, ticket("t1", 100), 0).unwrap();
        let a2 = reg.open("a".into(), config(1), RULES, ticket("t2", 100), 0).unwrap();
        let b = reg.open("b".into(), config(2), RULES, ticket("t3", 100), 0).unwrap();
        assert!(a1.same_channel(&a2));
        assert!(!a1.same_channel(&b));
        assert_eq!(
            reg.open("a".into(), config(9), RULES, ticket("t4", 100), 0).unwrap_err(),
            OpenError::ConfigMismatch
        );
    }

    #[tokio::test]
    async fn ticket_is_single_use_until_it_expires() {
        let mut reg = registry();
        reg.open("a".into(), config(1), RULES, ticket("t1", 100), 50).unwrap();
        assert_eq!(reg.open("a".into(), config(1), RULES, ticket("t1", 100), 60).unwrap_err(), OpenError::TicketReused);
        // Po wygaśnięciu wpis znika (sam bilet i tak nie przejdzie wtedy weryfikacji podpisu/exp).
        reg.open("a".into(), config(1), RULES, ticket("t2", 100), 101).unwrap();
        assert!(!reg.used.contains_key("t1"));
        // Bez biletu (tryb lokalny) nie ma czego pilnować.
        reg.open("a".into(), config(1), RULES, None, 101).unwrap();
    }
}
