# 0003. Jedna domena za CloudFront dla frontendu, API i WebSocketu

**Status:** przyjęta (2026-10)

## Kontekst

Są trzy źródła treści: statyczny frontend (Angular + wasm), API lobby i WebSocket game-servera. Strona jest serwowana przez HTTPS, więc WebSocket musi być `wss://`. Frontend łączy się dziś z `/ws` na tym samym hoście (`web/src/app/game/transport.ts`).

## Decyzja

Jedna dystrybucja CloudFront na własnej domenie (certyfikat ACM z us-east-1):

| Ścieżka | Origin | Cache |
|---|---|---|
| `/*` | S3 (prywatny bucket, Origin Access Control) + CloudFront Function z fallbackiem SPA | pliki z hashem – rok, `index.html` – bez cache |
| `/api/*` | API Gateway HTTP API | wyłączony |
| `/ws*` | EC2 (HTTP, port 3000), WebSocket | wyłączony |

Origin EC2 jest chroniony: Security Group wpuszcza tylko managed prefix list `com.amazonaws.global.cloudfront.origin-facing`, a serwer wymaga sekretnego nagłówka `X-Origin-Verify` dokładanego przez CloudFront.

## Konsekwencje

- Brak CORS, jeden certyfikat, TLS za darmo (CloudFront always free: 1 TB / 10 mln żądań).
- EC2 nie potrzebuje certyfikatu ani własnej domeny.
- Fallback SPA w CloudFront Function, a nie w „custom error responses” – te podmieniałyby też błędy `/api`.
- Przy wielu serwerach gry potrzebny będzie routing do konkretnego serwera (NLB albo subdomeny `gs-<n>`) – decyzja odłożona.
