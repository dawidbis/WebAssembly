# Symulacja czasu wczytania

Mierzy czas od wejścia na stronę do gotowej mapy (`performance.mark('map-rendered')`)
dla buildu produkcyjnego, z dławieniem sieci i procesora.

```bash
cd web && npx ng build && cd ..
# serwer: port, katalog, przepustowość kbit/s (0 = bez limitu), RTT ms, gzip|none
node tools/loadtest/throttle-server.mjs 5100 web/dist/web/browser 1600 150 gzip &
# pomiar: adres, spowolnienie CPU (1 = bez, 2 = dwa razy wolniej…)
node tools/loadtest/sim.mjs http://127.0.0.1:5100/ 1
```

- Sieć dławi serwer (wspólny limit przepustowości, 1 RTT na żądanie, 3 RTT na nowe połączenie).
- CPU spowalnia wstrzymywanie całego procesu przeglądarki (SIGSTOP/SIGCONT, jak `cpulimit`) –
  DevTools nie potrafi spowolnić workera, w którym działa generator.
- Wynik (JSON): `ready` – teren na ekranie (ms od startu), `provinces` – prowincje na ekranie,
  `gen` / `provincesMs` – generowanie terenu / prowincji w workerze,
  `dom` – DOMContentLoaded, `bytes` – przesłane bajty (bez wasm pobieranego przez worker).
- Ścieżka do Chromium jest w `sim.mjs` (`/opt/pw-browsers/chromium` w kontenerze).
