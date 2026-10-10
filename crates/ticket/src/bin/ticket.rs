//! Wystawianie biletów ręcznie – do testów game-servera bez lobby.
//!
//!   cargo run -p game-ticket -- --key ticket.pem --room r1 --name Ala [--seed 7] [--params p.json] [--ttl 60]
//!
//! Wypisuje bilet (JWT); połączenie: ws://127.0.0.1:3000/ws?ticket=<bilet>.

use game_core::{
    mapgen::{GENERATOR_VERSION, MapGenParams},
    protocol::GameConfig,
};
use game_ticket::{Signer, TTL_SECS, TicketClaims, now_secs};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let read = |path: String| std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));

    let key = read(arg("--key").expect("--key: plik z kluczem prywatnym PEM"));
    let mut map: MapGenParams = match arg("--params") {
        Some(path) => serde_json::from_str(&read(path)).expect("--params: JSON z polami MapGenParams"),
        None => MapGenParams::default(),
    };
    if let Some(seed) = arg("--seed") {
        map.seed = seed.parse().expect("--seed: liczba u32");
    }
    let ttl: u64 = arg("--ttl").map_or(TTL_SECS, |t| t.parse().expect("--ttl: sekundy"));
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let now = now_secs();

    let claims = TicketClaims {
        room: arg("--room").unwrap_or_else(|| "test".into()),
        name: arg("--name").unwrap_or_else(|| "gracz".into()),
        config: GameConfig { generator_version: GENERATOR_VERSION, map },
        jti: format!("{nanos:x}-{}", std::process::id()),
        iat: now,
        exp: now + ttl,
    };
    let signer = Signer::from_pem(&key).expect("niepoprawny klucz prywatny PEM (Ed25519)");
    println!("{}", signer.sign(&claims).expect("podpis"));
}
