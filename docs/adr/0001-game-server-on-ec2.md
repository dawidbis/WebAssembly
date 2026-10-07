# 0001. Game-server jako proces na EC2, nie Lambda / API Gateway WebSocket

**Status:** przyjęta (2026-10)

## Kontekst

Pokój gry (`crates/server/src/room.rs`) to stanowy aktor: zegar tur co 100 ms, log tur do nadrabiania, porównanie hashy stanu, a docelowo symulacja-cień (ADR 0006). Projekt ma działać tanio (Free plan AWS), ale być gotowy do skalowania.

Rozważane opcje:

1. **API Gateway WebSocket + Lambda** – w pełni serverless, ale Lambda nie trzyma stanu ani zegara między wywołaniami (stan musiałby żyć w DynamoDB, zegar w Step Functions/EventBridge – minimalny interwał 1 min, za wolno). Płatność za każdą wiadomość: 10 graczy × 10 tur/s ≈ 8,6 mln wiadomości na dobę na pokój.
2. **ECS Fargate (zadanie na mecz)** – dobra izolacja i skalowanie do zera, ale brak darmowego progu, dynamiczne IP utrudnia TLS/CloudFront, zimny start zadania ~30–60 s.
3. **Jedna mała instancja EC2 (Graviton) z wieloma pokojami w jednym procesie** – proces Rust/tokio obsługuje wiele pokoi tanio; koszt stały z kredytów Free plan.
4. **Amazon GameLift** – zarządzana flota serwerów gier; przerost formy przy obecnej skali.

## Decyzja

Opcja 3: binarka `game-server` jako usługa systemd na EC2 t4g.micro (Amazon Linux 2023, arm64), za CloudFront (ADR 0003). Bez SSH – wdrożenia przez SSM Run Command.

## Konsekwencje

- Koszt ok. 10 USD/mies. (instancja + IPv4 + dysk) – z kredytów; instancję można zatrzymać, frontend działa wtedy offline.
- Jeden punkt awarii: restart instancji kończy trwające gry. Akceptowalne w v1; alarm EC2 z akcją `recover`.
- Ścieżka skalowania: wiele instancji (ASG) rejestrujących się w DynamoDB (`SERVER#<id>`), a meta-serwer przydziela pokoje najmniej obciążonej. Kod pokoju się nie zmienia.
