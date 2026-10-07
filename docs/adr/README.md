# Decyzje architektoniczne (ADR)

Krótkie zapisy decyzji: kontekst, decyzja, konsekwencje. Nowa decyzja = nowy plik z kolejnym numerem; zmiana decyzji = nowy ADR, który zastępuje stary (stary dostaje status „zastąpiony przez …”).

| Nr | Decyzja | Status |
|---|---|---|
| [0001](0001-game-server-on-ec2.md) | Game-server jako proces na EC2, nie Lambda / API Gateway WebSocket | przyjęta |
| [0002](0002-serverless-meta-server.md) | Meta-serwer (lobby) serverless: HTTP API + Lambda w Ruście + DynamoDB | przyjęta |
| [0003](0003-single-domain-cloudfront.md) | Jedna domena za CloudFront dla frontendu, API i WebSocketu | przyjęta |
| [0004](0004-join-tickets.md) | Dołączanie do pokoju przez podpisany bilet (JWT Ed25519) | przyjęta |
| [0005](0005-terraform-deploy-from-ide.md) | Terraform w repo, wdrażanie z IDE przez SSO | przyjęta |
| [0006](0006-shadow-simulation.md) | Symulacja-cień na serwerze jako anticheat lockstepu | przyjęta |
