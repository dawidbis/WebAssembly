# CLAUDE.md – wskazówki dla kolejnych sesji

Pełny opis projektu, parametrów, klawiszy, kontraktów i planu jest w [README.md](README.md) – przeczytaj najpierw sekcje „Architektura”, „Determinizm i hashe”, „Kontrakty utrzymywane ręcznie” i „Stan projektu i plan”.

## Współpraca

- Z użytkownikiem rozmawiamy **po polsku**; komentarze w kodzie i README też po polsku, commity po angielsku.
- Każda nowa funkcjonalność na **osobnym branchu** `claude/<nazwa>` od `main`; do `main` łączymy dopiero na prośbę (zwykle fast-forward). PR-ów nie otwieramy bez prośby.
- Usuwanie branchy na GitHubie z sesji kończy się 403 – użytkownik robi to sam w zakładce Branches.
- Użytkownik ma własną kopię repo (folder na pulpicie) i pobiera zmiany przez `git pull`; po zmianach w Ruście potrzebuje `npm run prep`.
- Zmiany wizualne pokazuj zrzutami z przeglądarki; użytkownik lubi stroić wartości sam – mów, w którym pliku/linii są.
- Wszystko, co dostraja wygląd/generator, powinno mieć suwak w panelu debugu.

## Środowisko w chmurze (pułapki)

- Node w kontenerze to 22.22.2, a Angular CLI 22 wymaga ≥ 22.22.3 → `source /opt/nvm/nvm.sh && nvm use 24` (w razie braku: `nvm install 24`).
- `rustup target add wasm32-unknown-unknown` i `cargo install wasm-pack`, jeśli ich brak.
- `wasm-pack` nie pobierze `wasm-opt` przez proxy – pobierz binaryen 117 przez `curl` i skopiuj `bin/wasm-opt` do `~/.cargo/bin`.
- Test w przeglądarce: `playwright-core` + Chromium z `/opt/pw-browsers/chromium`, flagi `--use-gl=swiftshader --enable-unsafe-swiftshader --ignore-gpu-blocklist`. Serwer dev: `npx ng serve` w `web/` (port 4200). Po teście zabij `ng serve`.
- Renderowanie jest programowe (bez GPU) – liczby FPS i płynność nagrań nie są miarodajne.
- **Nie zabijaj `ng serve` przez `pkill -f "ng serve"`** – wzorzec pasuje też do własnej powłoki i kończy komendę (exit 144). Zapisuj PID: `(npx ng serve > log 2>&1 & echo $! > ng.pid)`, potem `kill $(cat ng.pid)`.
- Czasem Bash chwilowo odmawia („classifier gave no verdict”) – edytuj wtedy narzędziami Edit/Write i spróbuj Bash później.
- Pomiary czasu wczytania: `tools/loadtest/` (serwer z dławieniem sieci + Chromium spowalniany SIGSTOP/SIGCONT; DevTools nie spowalnia workerów).

## Architektura w skrócie (szczegóły w README)

- Generator jest dwufazowy: `generate_base` (teren, biomy, woda, lasy) i `generate_provinces`; `generate` = obie fazy (test pilnuje identyczności). W przeglądarce prowincje przychodzą osobną wiadomością workera.
- Warstwy RGBA maluje `render/paint.worker.ts` (klient `render/painter.ts`); renderer pamięta 3 widoki i przenika je. Kod malowania (`render/terrain.ts`, `render/provinces.ts`) musi działać bez DOM.
- Góry (i rzeki w górach) są nieprzechodnie i niczyje: kafel lądu/rzeki z `province == 0`.
- Interfejs gracza: `web/src/app/ui/` (górny pasek, ramka prowincji, napis ładowania) – działa też w produkcji. Panel debugu (`debug/`) tylko w dev, otwierany Esc.
- Następna sesja: pierwsze mechaniki – zacznij od sekcji README „Gdzie wejdą mechaniki” (stan wyjściowy, braki, proponowana kolejność; kolejność uzgodnij z użytkownikiem).

## Weryfikacja przed commitem

```bash
cargo test --workspace --all-features              # 37 testów, zero ostrzeżeń (cargo build --all-features)
cargo run -p game-mapgen --release --features cli -- --seed 1 --out /tmp/m.png   # hashe z tabeli w README
cd web && npm run prep && npx ng build --configuration development && npx ng build
```

- Hashe seeda 1 w CLI i w panelu przeglądarki muszą być identyczne (determinizm native vs wasm). Po zmianie domyślnych parametrów lub algorytmu zaktualizuj tabelę w README; po zmianie algorytmu podbij `GENERATOR_VERSION`.
- Nowe etapy generatora dostają własny RNG (`seed ^ SALT`), żeby nie zmieniać terenu i biomów – dodaj test, że teren/biomy się nie zmieniają.
- Palety i wygląd są zdublowane w CLI (`crates/mapgen/src/bin/mapgen.rs`) i w `web/src/app/render/terrain.ts` / `render/provinces.ts` – zmieniaj oba.
- Shadery (`render/waves.ts`, `render/inland.ts`, `render/trees.ts`) Pixi kompiluje w starszym GLSL: brak `fwidth`, unikaj nazw zmiennych typu `patch`. Błędy shadera widać tylko w konsoli przeglądarki – zawsze sprawdź ją w teście.
- Oświetlenie: światło z lewego górnego rogu, cienie w prawo w dół (teren, dno oceanu, drzewa).
