# 0007. Ochrona przed spamem żądań i DDoS – warstwami, w darmowym progu

**Status:** przyjęta (2026-10)

## Kontekst

Gra jest publiczna: lobby (API Gateway + Lambda + DynamoDB) i game-server (WebSocket na EC2) za CloudFront. Zagrożenia: zalewanie API (koszty, wyczerpanie przepustowości DynamoDB), zakładanie tysięcy lobby, otwieranie tysięcy połączeń WebSocket, zalewanie wiadomościami w grze, wołanie originów z pominięciem CloudFront. Projekt hobbystyczny – ochrona ma się mieścić w darmowym progu.

## Decyzja – warstwy

| Warstwa | Co chroni | Koszt |
|---|---|---|
| **AWS Shield Standard** (automatycznie na CloudFront) | ataki sieciowe L3/L4 (SYN flood, UDP reflection) | 0 |
| **Originy tylko przez CloudFront** – sekretny nagłówek `X-Origin-Verify` (API: sprawdza Lambda, game-server: sprawdza serwer + Security Group z listą adresów CloudFront) | ominięcie limitów i podrabianie adresu klienta | 0 |
| **API Gateway – throttling** (cały etap 10 req/s, burst 20; `POST /api/rooms` 1 req/s, burst 5) | koszt Lambdy/DynamoDB przy zalewie – sufit liczby wywołań | 0 |
| **Lambda – limity na adres IP** (`CloudFront-Viewer-Address`; okna minutowe, liczniki `RATE#…` w DynamoDB z TTL): zakładanie lobby 5/min, bilety 30/min → `429` | jeden klient nie zajmie wszystkich zasobów | ~0 (zapis tylko przy zakładaniu/dołączaniu) |
| **DynamoDB w trybie provisioned** (5/5) | sufit kosztów – zalew spowalnia zapisy (throttling), zamiast nabijać rachunek | 0 (always free) |
| **Game-server** (`server/src/limits.rs`): ≤ 12 połączeń z adresu IP, ≤ 2000 łącznie (`429` przy otwarciu), ≤ 20 wiadomości/s na połączenie (burst 60 – potem rozłączenie), wiadomość ≤ 16 KB, ≤ 200 pokoi | wyczerpanie pamięci/CPU instancji | 0 |
| **Walidacja wejścia** (nazwy, limity graczy, ID kart, rozmiar treści ≤ 4 KB, bilety jednorazowe 60 s) | śmieciowe dane, powtórki biletów | 0 |
| **AWS Budgets** (1 i 10 USD, przed kredytami) | wykrycie nieoczekiwanych kosztów | 0 |

## Rozważane, odłożone

- **AWS WAF z regułą rate-based na CloudFront** – limity na IP na brzegu (przed Lambdą), listy znanych botów. Koszt ~5 USD/mies. za web ACL + 1 USD/regułę + opłata za żądania. Do włączenia przy realnym ruchu albo ataku; ewentualnie w ramach planów stałej opłaty CloudFront (sprawdzić aktualną ofertę).
- **Limit współbieżności Lambdy (reserved concurrency)** – na nowych kontach limit konta bywa niski (10), a rezerwacja wymaga zapasu nierezerwowanego; throttling API Gateway pełni tę rolę.

## Konsekwencje

- Gracze za wspólnym NAT (akademik, firma) dzielą limity na IP – progi są dobrane z zapasem (kilka osób na adres).
- Licznik limitu w Lambdzie jest „miękki”: błąd DynamoDB przepuszcza żądanie (ochrona nie może wyłączyć gry).
- Po zmianie sekretu originu (np. `terraform apply -replace`) CloudFront i Lambda/serwer muszą dostać nową wartość – przez kilka minut propagacji CloudFront żądania mogą dostawać 403.
