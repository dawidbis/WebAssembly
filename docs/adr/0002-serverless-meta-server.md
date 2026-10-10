# 0002. Meta-serwer (lobby) serverless: HTTP API + Lambda w Ruście + DynamoDB

**Status:** przyjęta (2026-10)

## Kontekst

Meta-serwer obsługuje rzeczy poza samą rozgrywką: lista pokoi, tworzenie pokoju, dołączanie (bilet – ADR 0004), później konta, historia i rankingi. Ruch jest rzadki i nieregularny (żądania HTTP, nie strumień).

## Decyzja

- **Lambda w Ruście** (`crates/meta`, `lambda_http`, runtime `provided.al2023`, architektura arm64; budowana `cargo zigbuild` jako statyczna binarka `bootstrap` – tym samym narzędziem co game-server, zip robi Terraform `archive_file`): ten sam język i te same typy co rdzeń (`game_core` – `GameConfig`, `MapGenParams::sanitized()`), typy TS generowane przez `ts-rs`. Zimny start Rust na arm64 to kilkadziesiąt ms.
- **API Gateway HTTP API** jako wejście (zamiast Lambda Function URL): trasy, throttling, a później authorizer JWT (Cognito) bez zmian w kodzie. Koszt 1 USD / 1 mln żądań – pomijalny.
- **DynamoDB**, jedna tabela (single-table design): `PK=ROOM#<id>, SK=META`, GSI po statusie do listy otwartych pokoi, TTL na martwe pokoje; miejsce na `SERVER#<id>` pod wiele serwerów gry. Tryb provisioned 5/5 RCU/WCU (always-free do 25/25); on-demand przy większym ruchu.

## Konsekwencje

- Zero kosztu przy braku ruchu; skaluje się bez zmian.
- Lokalnie: logika API (`meta/src/app.rs`) nie zależy od Lambdy i jest testowana na magazynie w pamięci; przepływ z przeglądarką – `tools/lobby/e2e.mjs` (atrapa `/api`).
- Stan pokoju (liczba graczy, tick) dopisuje game-server heartbeatem – meta czyta, nie liczy.
