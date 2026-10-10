//! Meta-serwer (lobby) jako AWS Lambda za API Gateway HTTP API (docs/adr/0002).
//!
//! Zmienne środowiskowe (ustawia Terraform, `infra/modules/meta`):
//! - `ROOMS_TABLE` – tabela DynamoDB z pokojami,
//! - `TICKET_KEY_PARAM` – parametr SSM (SecureString) z kluczem prywatnym biletów (PEM Ed25519),
//! - `ORIGIN_VERIFY_SECRET` – wymagany nagłówek `X-Origin-Verify` (dokłada go CloudFront): API działa
//!   tylko przez CloudFront, więc limitów i adresu klienta (`CloudFront-Viewer-Address`) nie da się obejść.
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

    let origin_secret: Option<Arc<str>> =
        std::env::var("ORIGIN_VERIFY_SECRET").ok().filter(|s| !s.is_empty()).map(Arc::from);
    let app = Arc::new(App {
        store: DynamoStore { client: aws_sdk_dynamodb::Client::new(&config), table: env("ROOMS_TABLE")? },
        signer: game_ticket::Signer::from_pem(&pem)?,
        random: random_u64,
    });

    run(service_fn(move |req: Request| {
        let app = app.clone();
        let origin_secret = origin_secret.clone();
        async move {
            let header = |name: &str| req.headers().get(name).and_then(|v| v.to_str().ok());
            if let Some(secret) = &origin_secret
                && !header("x-origin-verify").is_some_and(|v| constant_time_eq(v.as_bytes(), secret.as_bytes()))
            {
                return Response::builder()
                    .status(403)
                    .body(Body::from(r#"{"error":"tylko przez CloudFront"}"#))
                    .map_err(Error::from);
            }
            let ip = header("cloudfront-viewer-address").map_or("nieznany", viewer_ip);
            let reply =
                app.handle(req.method(), req.uri().path(), req.body().as_ref(), game_ticket::now_secs(), ip).await;
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

/// Adres IP z `CloudFront-Viewer-Address` (`adres:port`, także IPv6 – port po ostatnim `:`).
fn viewer_ip(value: &str) -> &str {
    value.rsplit_once(':').map_or(value, |(ip, _port)| ip)
}

/// Porównanie sekretu bez wczesnego wyjścia (czas nie zdradza, ile znaków się zgadza).
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Losowość z systemu (ID pokoi i biletów muszą być nieprzewidywalne).
fn random_u64() -> u64 {
    let mut bytes = [0u8; 8];
    getrandom::fill(&mut bytes).expect("getrandom");
    u64::from_le_bytes(bytes)
}
