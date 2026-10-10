//! Rejestr pokoi: aktor z mapą `RoomId → RoomHandle`. Tworzy pokój przy pierwszym bilecie
//! (z konfiguracją z biletu), pilnuje jednorazowości biletów i zapomina zamknięte pokoje.

use std::{collections::BTreeMap, time::Duration};

use game_core::protocol::GameConfig;
use tokio::sync::{mpsc, oneshot};
use tracing::warn;

use crate::room::{self, RoomHandle, RoomId};

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
}

pub enum RegistryCmd {
    Open {
        id: RoomId,
        config: Box<GameConfig>,
        ticket: Option<TicketUse>,
        reply: oneshot::Sender<Result<RoomHandle, OpenError>>,
    },
    /// Liczba otwartych pokoi (do `/health`).
    Count { reply: oneshot::Sender<usize> },
    /// Otwarte pokoje (do heartbeatu).
    Snapshot { reply: oneshot::Sender<Vec<(RoomId, RoomHandle)>> },
}

pub type RegistryHandle = mpsc::UnboundedSender<RegistryCmd>;

struct Registry {
    dev: bool,
    idle: Duration,
    rooms: BTreeMap<RoomId, (GameConfig, RoomHandle)>,
    /// Zużyte bilety (`jti` → `exp`); po wygaśnięciu i tak nie przejdą weryfikacji, więc są usuwane.
    used: BTreeMap<String, u64>,
}

pub fn spawn(dev: bool, idle: Duration) -> RegistryHandle {
    let (tx, mut rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        let mut registry = Registry { dev, idle, rooms: BTreeMap::new(), used: BTreeMap::new() };
        while let Some(cmd) = rx.recv().await {
            registry.handle(cmd, game_ticket::now_secs());
        }
    });
    tx
}

impl Registry {
    fn handle(&mut self, cmd: RegistryCmd, now: u64) {
        // Pokoje kończą się same (pusty pokój po czasie `idle`) – zamknięty kanał = pokoju nie ma.
        self.rooms.retain(|_, (_, handle)| !handle.is_closed());
        match cmd {
            RegistryCmd::Open { id, config, ticket, reply } => {
                let _ = reply.send(self.open(id, *config, ticket, now));
            }
            RegistryCmd::Count { reply } => {
                let _ = reply.send(self.rooms.len());
            }
            RegistryCmd::Snapshot { reply } => {
                let _ = reply.send(self.rooms.iter().map(|(id, (_, handle))| (id.clone(), handle.clone())).collect());
            }
        }
    }

    fn open(
        &mut self,
        id: RoomId,
        config: GameConfig,
        ticket: Option<TicketUse>,
        now: u64,
    ) -> Result<RoomHandle, OpenError> {
        if let Some(ticket) = ticket {
            self.used.retain(|_, exp| *exp >= now);
            if self.used.contains_key(&ticket.jti) {
                warn!(room = %id, jti = %ticket.jti, "bilet użyty drugi raz");
                return Err(OpenError::TicketReused);
            }
            self.used.insert(ticket.jti, ticket.exp);
        }
        if let Some((existing, handle)) = self.rooms.get(&id) {
            if *existing != config {
                warn!(room = %id, "bilet z inną konfiguracją niż pokój");
                return Err(OpenError::ConfigMismatch);
            }
            return Ok(handle.clone());
        }
        let handle = room::spawn(id.clone(), self.dev, config.clone(), self.idle);
        self.rooms.insert(id, (config, handle.clone()));
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

    fn ticket(jti: &str, exp: u64) -> Option<TicketUse> {
        Some(TicketUse { jti: jti.into(), exp })
    }

    #[tokio::test]
    async fn rooms_are_separate_and_reused_by_id() {
        let mut reg =
            Registry { dev: false, idle: Duration::from_secs(60), rooms: BTreeMap::new(), used: BTreeMap::new() };
        let a1 = reg.open("a".into(), config(1), ticket("t1", 100), 0).unwrap();
        let a2 = reg.open("a".into(), config(1), ticket("t2", 100), 0).unwrap();
        let b = reg.open("b".into(), config(2), ticket("t3", 100), 0).unwrap();
        assert!(a1.same_channel(&a2));
        assert!(!a1.same_channel(&b));
        assert_eq!(reg.open("a".into(), config(9), ticket("t4", 100), 0).unwrap_err(), OpenError::ConfigMismatch);
    }

    #[tokio::test]
    async fn ticket_is_single_use_until_it_expires() {
        let mut reg =
            Registry { dev: false, idle: Duration::from_secs(60), rooms: BTreeMap::new(), used: BTreeMap::new() };
        reg.open("a".into(), config(1), ticket("t1", 100), 50).unwrap();
        assert_eq!(reg.open("a".into(), config(1), ticket("t1", 100), 60).unwrap_err(), OpenError::TicketReused);
        // Po wygaśnięciu wpis znika (sam bilet i tak nie przejdzie wtedy weryfikacji podpisu/exp).
        reg.open("a".into(), config(1), ticket("t2", 100), 101).unwrap();
        assert!(!reg.used.contains_key("t1"));
        // Bez biletu (tryb lokalny) nie ma czego pilnować.
        reg.open("a".into(), config(1), None, 101).unwrap();
    }
}
