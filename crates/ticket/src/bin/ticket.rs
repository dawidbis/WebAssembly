//! Bilety i klucze ręcznie – do testów game-servera bez lobby.
//!
//!   cargo run -p game-ticket -- keygen --out katalog      # ticket.pem (prywatny) + ticket.pub.pem
//!   cargo run -p game-ticket -- --key ticket.pem --room r1 --name Ala [--client karta] [--max 8] [--seed 7] [--params p.json] [--ttl 60]
//!
//! Bilet (JWT) idzie na stdout; połączenie: ws://127.0.0.1:3000/ws?ticket=<bilet>
//! (serwer z `--ticket-key ticket.pub.pem`).

use std::path::Path;

use game_core::{
    mapgen::{GENERATOR_VERSION, MapGenParams},
    protocol::GameConfig,
};
use game_ticket::{Signer, TTL_SECS, TicketClaims, generate_key_pair, now_secs};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();

    if args.get(1).map(String::as_str) == Some("keygen") {
        let dir = arg("--out").unwrap_or_else(|| ".".into());
        let keys = generate_key_pair();
        let write = |name: &str, pem: &str| {
            let path = Path::new(&dir).join(name);
            std::fs::write(&path, pem).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            println!("{}", path.display());
        };
        write("ticket.pem", &keys.private);
        write("ticket.pub.pem", &keys.public);
        return;
    }

    let read = |path: String| std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let key = read(arg("--key").expect("--key: plik z kluczem prywatnym PEM (albo: ticket keygen)"));
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
        // Bez --client każdy bilet to inna karta (po starcie gry serwer wpuszcza tylko znane karty).
        client: arg("--client").unwrap_or_else(|| format!("cli-{nanos:x}")),
        max_players: arg("--max").map_or(8, |m| m.parse().expect("--max: liczba graczy")),
        config: GameConfig { generator_version: GENERATOR_VERSION, map },
        jti: format!("{nanos:x}-{}", std::process::id()),
        iat: now,
        exp: now + ttl,
    };
    let signer = Signer::from_pem(&key).expect("niepoprawny klucz prywatny PEM (Ed25519)");
    println!("{}", signer.sign(&claims).expect("podpis"));
}
