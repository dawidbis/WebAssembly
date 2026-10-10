//! Meta-serwer (lobby) jako AWS Lambda za API Gateway HTTP API (docs/adr/0002).
//!
//! Zmienne środowiskowe (ustawia Terraform, `infra/modules/meta`):
//! - `ROOMS_TABLE` – tabela DynamoDB z pokojami,
//! - `TICKET_KEY_PARAM` – parametr SSM (SecureString) z kluczem prywatnym biletów (PEM Ed25519).
//!
//! Build (Linux arm64, statycznie): `cargo zigbuild --release -p game-meta --target aarch64-unknown-linux-musl`
//! – binarka `bootstrap` dla runtime `provided.al2023`; zip robi Terraform.

mod app;
mod store;

use std::sync::Arc;

use lambda_http::{Body, Error, Request, Response, run, service_fn};

use app::App;
use store::DynamoStore;

#[tokio::main]
async fn main() -> Result<(), Error> {
    lambda_http::tracing::init_default_subscriber();
    let env = |name: &str| std::env::var(name).map_err(|_| format!("brak zmiennej {name}"));

    let config = aws_config::load_from_env().await;
    let key_param = env("TICKET_KEY_PARAM")?;
    // Klucz czytany raz, przy zimnym starcie – kolejne wywołania używają go z pamięci.
    let pem = aws_sdk_ssm::Client::new(&config)
        .get_parameter()
        .name(&key_param)
        .with_decryption(true)
        .send()
        .await?
        .parameter
        .and_then(|p| p.value)
        .ok_or_else(|| format!("pusty parametr {key_param}"))?;

    let app = Arc::new(App {
        store: DynamoStore { client: aws_sdk_dynamodb::Client::new(&config), table: env("ROOMS_TABLE")? },
        signer: game_ticket::Signer::from_pem(&pem)?,
        random: random_u64,
    });

    run(service_fn(move |req: Request| {
        let app = app.clone();
        async move {
            let reply = app.handle(req.method(), req.uri().path(), req.body().as_ref(), game_ticket::now_secs()).await;
            Response::builder()
                .status(reply.status)
                .header("content-type", "application/json; charset=utf-8")
                .header("cache-control", "no-store")
                .body(Body::from(reply.body))
                .map_err(Error::from)
        }
    }))
    .await
}

/// Losowość z systemu (ID pokoi i biletów muszą być nieprzewidywalne).
fn random_u64() -> u64 {
    let mut bytes = [0u8; 8];
    getrandom::fill(&mut bytes).expect("getrandom");
    u64::from_le_bytes(bytes)
}
