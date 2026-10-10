//! Heartbeat pokoi do lobby: zapis stanu pokoi w tabeli meta-serwera (DynamoDB).
//!
//! - **od razu** przy każdym zdarzeniu pokoju (`RoomEvent`: dołączenie, wyjście, start, zamknięcie) –
//!   zamknięte lobby znika z listy natychmiast, liczba graczy jest aktualna,
//! - **co `HEARTBEAT_SECS`** – pełny raport (odświeża `lastSeen`; pokoje, które zniknęły bez
//!   zdarzenia, dostają status `closed`).
//!
//! Zapis do DynamoDB jest za cechą `aws` (build na EC2); lokalnie i w testach heartbeatu nie ma.

use std::{collections::BTreeSet, time::Duration};

use tokio::sync::{mpsc, oneshot};

use crate::{
    room::{RoomCmd, RoomEvent, RoomId, RoomStats},
    rooms::{RegistryCmd, RegistryHandle},
};

#[cfg_attr(not(feature = "aws"), allow(dead_code))]
pub const HEARTBEAT_SECS: u64 = 15;

#[derive(Clone, Debug, PartialEq)]
pub struct RoomReport {
    pub id: RoomId,
    pub stats: RoomStats,
}

/// Dokąd trafiają raporty (DynamoDB w produkcji, wektor w testach).
pub(crate) trait Sink {
    /// `Send` – heartbeat działa w osobnym zadaniu tokio.
    fn report(&self, open: &[RoomReport], closed: &[RoomId], now: u64) -> impl Future<Output = ()> + Send;
}

/// Stan wszystkich otwartych pokoi.
pub async fn collect(registry: &RegistryHandle) -> Vec<RoomReport> {
    let (reply, rx) = oneshot::channel();
    if registry.send(RegistryCmd::Snapshot { reply }).is_err() {
        return Vec::new();
    }
    let mut reports = Vec::new();
    for (id, handle) in rx.await.unwrap_or_default() {
        let (reply, rx) = oneshot::channel();
        if handle.send(RoomCmd::Stats { reply }).is_ok()
            && let Ok(stats) = rx.await
        {
            reports.push(RoomReport { id, stats });
        }
    }
    reports
}

/// Jeden krok heartbeatu: raport i lista pokoi do porównania w następnym kroku.
pub(crate) async fn tick(
    registry: &RegistryHandle,
    sink: &impl Sink,
    previous: &BTreeSet<RoomId>,
    now: u64,
) -> BTreeSet<RoomId> {
    let open = collect(registry).await;
    let current: BTreeSet<RoomId> = open.iter().map(|r| r.id.clone()).collect();
    let closed: Vec<RoomId> = previous.difference(&current).cloned().collect();
    sink.report(&open, &closed, now).await;
    current
}

/// Zdarzenie pokoju – zapis od razu (lista lobby nie czeka na cykliczny raport).
pub(crate) async fn on_event(sink: &impl Sink, event: RoomEvent, previous: &mut BTreeSet<RoomId>, now: u64) {
    match event {
        RoomEvent::Changed { id, stats } => {
            previous.insert(id.clone());
            sink.report(&[RoomReport { id, stats }], &[], now).await;
        }
        RoomEvent::Closed { id } => {
            previous.remove(&id);
            sink.report(&[], &[id], now).await;
        }
    }
}

#[cfg_attr(not(feature = "aws"), allow(dead_code))]
pub(crate) fn spawn(registry: RegistryHandle, sink: impl Sink + Send + Sync + 'static) {
    let (tx, mut events) = mpsc::unbounded_channel();
    let _ = registry.send(RegistryCmd::Watch { tx });
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(HEARTBEAT_SECS));
        let mut previous = BTreeSet::new();
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    previous = tick(&registry, &sink, &previous, game_ticket::now_secs()).await;
                }
                Some(event) = events.recv() => {
                    on_event(&sink, event, &mut previous, game_ticket::now_secs()).await;
                }
            }
        }
    });
}

#[cfg(feature = "aws")]
pub mod dynamo {
    //! Zapis heartbeatu w tabeli pokoi (ten sam układ co `crates/meta/src/store.rs`).

    use aws_sdk_dynamodb::{Client, error::DisplayErrorContext, types::AttributeValue};
    use tracing::{debug, warn};

    use super::{RoomReport, Sink};
    use crate::room::RoomId;

    /// Pokój bez heartbeatu znika z tabeli po tym czasie (TTL).
    const TTL_SECS: u64 = 3600;

    pub struct DynamoSink {
        pub client: Client,
        pub table: String,
    }

    impl DynamoSink {
        async fn update(&self, id: &str, set: &str, values: &[(&str, AttributeValue)]) {
            let mut req = self
                .client
                .update_item()
                .table_name(&self.table)
                .key("pk", AttributeValue::S(format!("ROOM#{id}")))
                .key("sk", AttributeValue::S("META".into()))
                .update_expression(set)
                // Tylko pokoje z lobby – pokój `default` (tryb otwarty) ani obce ID nie tworzą wpisów.
                .condition_expression("attribute_exists(pk)")
                .expression_attribute_names("#ttl", "ttl")
                .expression_attribute_names("#s", "status");
            for (name, value) in values {
                req = req.expression_attribute_values(*name, value.clone());
            }
            match req.send().await {
                Ok(_) => {}
                Err(e) if e.as_service_error().is_some_and(|s| s.is_conditional_check_failed_exception()) => {
                    debug!(room = %id, "pokoju nie ma w lobby – pomijam");
                }
                Err(e) => warn!(room = %id, error = %DisplayErrorContext(e), "heartbeat"),
            }
        }
    }

    impl Sink for DynamoSink {
        async fn report(&self, open: &[RoomReport], closed: &[RoomId], now: u64) {
            let n = |v: u64| AttributeValue::N(v.to_string());
            for r in open {
                self.update(
                    &r.id,
                    // Lobby pokazuje tylko pokoje w poczekalni (`open`); po starcie – `playing`.
                    "SET players = :p, tick = :t, lastSeen = :now, #ttl = :ttl, #s = :status",
                    &[
                        (":p", n(r.stats.players.into())),
                        (":t", n(r.stats.tick.into())),
                        (":now", n(now)),
                        (":ttl", n(now + TTL_SECS)),
                        (":status", AttributeValue::S(if r.stats.started { "playing" } else { "open" }.into())),
                    ],
                )
                .await;
            }
            for id in closed {
                self.update(
                    id,
                    "SET players = :zero, lastSeen = :now, #ttl = :ttl, #s = :closed",
                    &[
                        (":zero", n(0)),
                        (":now", n(now)),
                        (":ttl", n(now + TTL_SECS)),
                        (":closed", AttributeValue::S("closed".into())),
                    ],
                )
                .await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use game_core::{
        mapgen::{GENERATOR_VERSION, MapGenParams},
        protocol::GameConfig,
    };
    use tokio::sync::mpsc;

    use super::*;
    use crate::rooms;

    #[derive(Default)]
    struct VecSink(Mutex<Vec<(Vec<RoomReport>, Vec<RoomId>)>>);

    impl Sink for VecSink {
        async fn report(&self, open: &[RoomReport], closed: &[RoomId], _now: u64) {
            self.0.lock().unwrap().push((open.to_vec(), closed.to_vec()));
        }
    }

    fn config() -> GameConfig {
        GameConfig {
            generator_version: GENERATOR_VERSION,
            map: MapGenParams { width: 200, height: 160, ..Default::default() },
        }
    }

    async fn open(registry: &RegistryHandle, id: &str) -> crate::room::RoomHandle {
        let (reply, rx) = oneshot::channel();
        let rules = crate::room::RoomRules { dev: false, auto_start: false, max_players: 8 };
        registry
            .send(RegistryCmd::Open { id: id.into(), config: Box::new(config()), rules, ticket: None, reply })
            .unwrap();
        rx.await.unwrap().unwrap()
    }

    #[tokio::test]
    async fn events_are_reported_at_once() {
        let sink = VecSink::default();
        let mut previous = BTreeSet::new();
        let stats = RoomStats { players: 2, tick: 0, started: false };
        on_event(&sink, RoomEvent::Changed { id: "a".into(), stats }, &mut previous, 0).await;
        on_event(&sink, RoomEvent::Closed { id: "a".into() }, &mut previous, 1).await;
        let reports = sink.0.lock().unwrap().clone();
        assert_eq!(reports[0].0, vec![RoomReport { id: "a".into(), stats }]);
        assert_eq!(reports[1].1, vec!["a".to_string()]);
        assert!(previous.is_empty(), "zamknięty pokój nie wróci jako `closed` w kolejnym raporcie");
    }

    #[tokio::test(start_paused = true)]
    async fn reports_players_and_closed_rooms() {
        let registry = rooms::spawn(Duration::from_secs(60));
        let a = open(&registry, "a").await;
        open(&registry, "b").await;
        let (out, _rx) = mpsc::unbounded_channel();
        let (reply, joined) = oneshot::channel();
        a.send(RoomCmd::Join { out, name: "Ala".into(), client: None, reply }).unwrap();
        joined.await.unwrap().unwrap();

        let sink = VecSink::default();
        let seen = tick(&registry, &sink, &BTreeSet::new(), 0).await;
        assert_eq!(seen, BTreeSet::from(["a".to_string(), "b".to_string()]));
        let (open_rooms, closed) = sink.0.lock().unwrap()[0].clone();
        let players: Vec<(String, u16)> = open_rooms.iter().map(|r| (r.id.clone(), r.stats.players)).collect();
        assert_eq!(players, vec![("a".into(), 1), ("b".into(), 0)]);
        assert!(closed.is_empty());

        // Pusty pokój `b` zamyka się po czasie bezczynności – kolejny raport zgłasza go jako zamknięty.
        tokio::time::sleep(Duration::from_secs(61)).await;
        let seen = tick(&registry, &sink, &seen, 61).await;
        assert_eq!(seen, BTreeSet::from(["a".to_string()]));
        assert_eq!(sink.0.lock().unwrap()[1].1, vec!["b".to_string()]);
    }
}
