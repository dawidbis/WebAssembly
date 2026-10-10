# 0004. Dołączanie do pokoju przez podpisany bilet (JWT Ed25519)

**Status:** przyjęta (2026-10)

## Kontekst

Lobby (Lambda) i game-server (EC2) to osobne procesy. Game-server musi wiedzieć, do którego pokoju kierować połączenie i z jaką konfiguracją mapy go utworzyć, a gracz nie może wejść do pokoju z pominięciem lobby (limit miejsc, przyszłe konta).

## Decyzja

Meta-serwer wydaje przy `POST /api/rooms/{id}/join` krótki bilet: JWT podpisany Ed25519, ważny 60 s, z polami `room_id`, `player_name`, `config` (`GameConfig` pokoju – seed, parametry mapy, wersja generatora). Klient łączy się z `wss://<domena>/ws?ticket=<jwt>`. Game-server weryfikuje podpis kluczem publicznym i tworzy pokój z konfiguracji z biletu, jeśli jeszcze nie istnieje.

- Klucz prywatny: SSM Parameter Store (SecureString, standard tier – darmowy). Generuje go Terraform (`tls_private_key`, ED25519), więc jest też w stanie Terraform – prywatnym, szyfrowanym buckecie z wersjonowaniem (tak samo jak sekret originu). Świadomy kompromis na rzecz prostoty; rotacja: `terraform apply -replace=module.meta.tls_private_key.tickets`.
- Klucz publiczny: SSM (String), czytany przez game-server przy starcie.
- Typ `TicketClaims` w `game_core` – wspólny dla `meta` i `server`.
- Tryb lokalny (`--dev`, brak biletu): domyślny pokój jak dotąd, żeby `npm start` i narzędzia testowe działały bez lobby.

## Konsekwencje

- Game-server nie potrzebuje dostępu do bazy, żeby przyjąć gracza – mniej zależności, łatwiej skalować.
- Asymetryczny podpis: przejęcie game-servera nie pozwala wystawiać biletów.
- Bilet w URL trafia do logów dostępu – dlatego krótki czas życia i jednorazowe użycie w obrębie pokoju.
