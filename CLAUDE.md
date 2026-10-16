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
- **Po każdej zmianie `MapGenParams` uruchom test pętli tur** (niżej): parametry porównuje `sameParams` w `worker/protocol.ts` – pole-tablica (np. `biomeVariants`) porównywane przez `===` po cichu blokowało start gry (worker czekał na „mapę z konfiguracji”, tryb lokalny działał normalnie).
- Test pętli tur na kilku kartach: `node tools/lockstep/two-tabs.mjs http://127.0.0.1:3000/ --tamper` przy działającym `game-server` (build produkcyjny) albo z adresem `ng serve` (4200). Serwer: `--seed N` / `--params p.json` wybiera mapę gry.

## Lokalnie na Windows i AWS (pułapki)

- Sesje bywają też lokalne (VS Code na Windows, Git Bash + PowerShell). W Git Bash nie ma Pythona – większe przeróbki plików rób Edit/Write.
- AWS: profil CLI `wieczko` (domyślny wszędzie: Terraform, `tools/deploy/`, zadania VS Code), region `eu-central-1`. Z sesji w chmurze nie ma dostępu do konta – `terraform apply` i wdrożenia robi użytkownik albo lokalna sesja **po jego zgodzie** (to zmiany na koncie i koszty).
- PowerShell dzieli argumenty `-flag=plik.ext` na kropce: `terraform init "-backend-config=backend.hcl"` w cudzysłowie; zadania VS Code są typu `process` (bez powłoki). `curl` w PowerShell to alias – używaj `curl.exe`. Plik dla Terraform z PowerShell: `Out-File -Encoding ascii` (nie `>`).
- Skrypty uruchamiane na Linuksie (`*.sh`, `*.tftpl` – user-data EC2) muszą mieć LF – pilnuje `.gitattributes`; z CRLF cloud-init się wysypuje.
- Game-server na EC2 budujemy `cargo zigbuild --release -p game-server --target aarch64-unknown-linux-musl` (Zig z winget – po instalacji nowy PATH dopiero w nowym terminalu). Wdrożenie: `node tools/deploy/game-server.mjs`.
- Testy w Chrome na Windows: zmienna `CHROME` ze ścieżką do `chrome.exe`.
- Game-server ma stały adres (Elastic IP) – stop/start instancji nie wymaga `terraform apply`. Wdrożenie wszystkiego: `node tools/deploy/all.mjs`; sprawdzenie produkcji (tylko odczyt): `node tools/deploy/smoke.mjs`. Alarmy CloudWatch: moduł `monitoring` (`alert_email` w `terraform.tfvars`).
- Serwer działa w trybie otwartym, dopóki w SSM nie ma `/mapa/prod/ticket-public-key` (tworzy go moduł `meta`). Kolejność wdrożenia lobby: frontend → `tools/deploy/infra.mjs` → game-server (infra/README.md, krok 5) – inaczej stary frontend bez biletów nie połączy się z serwerem.
- `terraform plan/apply` w `envs/prod` wymaga zbudowanej Lambdy (`archive_file`) – używaj `node tools/deploy/infra.mjs [--plan]` (build `cargo zigbuild -p game-meta` + Terraform).
- Lobby testujesz bez AWS: `node tools/lobby/e2e.mjs` (atrapa `/api` przez CDP `Fetch` + game-server z biletami + Chrome; ekran powitalny, modal, poczekalnia, start, restart serwera); lobby w przeglądarce pokazuje się tylko, gdy `/api/rooms` zwraca JSON.
- Pokój z biletu ma poczekalnię (tury dopiero po `ClientMsg::Start` gospodarza, wyjście gospodarza z poczekalni zamyka pokój – `ServerMsg::Closed`; gospodarz zmienia ustawienia `ClientMsg::Settings` – game-server jest ich źródłem prawdy, lobby dostaje je heartbeatem; gra kończy się, gdy wyjdą wszyscy); pokój `default` (tryb otwarty) startuje sam – dlatego `two-tabs.mjs` działa bez zmian.
- Ochrona przed spamem (docs/adr/0007): limity na IP w Lambdzie (`app.rs`: `CREATE_PER_MINUTE`, `JOIN_PER_MINUTE`) i w game-serverze (`limits.rs`); adres klienta to `CloudFront-Viewer-Address` – ufamy mu tylko po sprawdzeniu nagłówka originu. Lokalnie (bez sekretu originu) limitu na IP nie ma.
- Ustawienia mapy w lobby to `MapSettings` (`core/src/lobby.rs`) – nowe ustawienie dodawaj tam (`params`), w oknie „Nowe lobby” (`ui/lobby-panel.*`) i w atrapie `tools/lobby/e2e.mjs` (CLI `ticket --settings`).
- Testowe klucze biletów generuje `game_ticket::test_keys::pair(seed)` (cecha `test-keys`) – nie wpisuj kluczy PEM do repo (skanery sekretów).

## Architektura w skrócie (szczegóły w README)

- Generator ma trzy fazy: `generate_base` (teren, biomy, woda, lasy), `generate_provinces` i `polish` (dostęp: tunele do dolin zamkniętych górami/lodowcem, za daleko – enklawa z yeti); `generate` = wszystkie (test pilnuje identyczności). W przeglądarce prowincje przychodzą osobną wiadomością workera.
- Warstwy RGBA maluje `render/paint.worker.ts` (klient `render/painter.ts`); renderer pamięta 3 widoki i przenika je. Kod malowania (`render/terrain.ts`, `render/provinces.ts`) musi działać bez DOM.
- Przechodni jest tylko kafel z `province > 0`: woda (ocean, jeziora, **rzeki**) i góry są nieprzechodnie i niczyje. Lądolód jest przechodni, chyba że mapa ma `glacier` (lodowiec: nieprzechodni, niczyj, rysowany z wyraźną krawędzią i cieniem). `Province` ma stałe właściwości z GDD (`area`, `biome`, `coastal`, `river`, `lake`, `mountains`) i `tunnel` (dostęp tylko tunelem). Do każdej prowincji da się dojść; wąska rzeka (≤ 4 kafle) łączy prowincje jak przyszły most.
- Zasoby: `Province.resource` (35 zasobów, złoto gwarantowane co najmniej raz) losuje `resources.rs` na końcu `polish` z tabeli szans `resource_table.rs` – generuje ją `node tools/resources/chances.mjs` (nie edytuj ręcznie); kolory i nazwy w `render/resources.ts` (ta sama kolejność). Mapa zasobów [4] to widok płaski jak polityczna (`isFlatView`).
- Biomy: 5 typów klimatu na kontynent, 13 rodzajów z pól chłodu i suchości, warianty typów (zbiory rodzajów, wagi `biomeVariants`), lód morski `seaIce` przy lądolodzie (README „Biomy”). Kafel ma `biome` (dominujący, do rozgrywki) i warstwy `biomeLayers` + `biomeMix` – wagi rodzajów liczy `kind_weights` (Rust) / `kindWeights` (TS), ten sam wzór w obu.
- Interfejs gracza: `web/src/app/ui/` (górny pasek, ramka prowincji, napis ładowania, komunikat o grze) – działa też w produkcji. Panel debugu (`debug/`) tylko w dev, otwierany Esc.
- Pętla lockstep (README „Pętla tur w przeglądarce”): `GameSession` generuje mapę dopiero z `GameConfig` z `Welcome` (bez serwera – z domyślnych), worker buduje z tej samej mapy `WasmGame.fromMap` (mapa nigdy nie jest generowana drugi raz), wykonuje tury i co 10 tur odsyła hash. Mapa z panelu debugu w trakcie gry to tylko lokalny podgląd.
- Stan gry w `Game` to pola inicjalizowane w `from_map` i dopisane do `state_hash`; mapa jest niezmienna (`restart` odtwarza grę z tej samej mapy).
- Serwer (README „Serwer i protokół”): rejestr pokoi (`server/src/rooms.rs`, aktor) + pokój (`room.rs`, aktor) + heartbeat do lobby (`heartbeat.rs`, cecha `aws`); tryb otwarty (pokój `default`) albo bilety JWT Ed25519 (`crates/ticket`). Lobby: Lambda `crates/meta` (logika w `app.rs` niezależna od Lambdy, typy w `core/src/lobby.rs`), frontend `game/lobby.ts` + `ui/lobby-panel.ts`. Wdrożenie na AWS: `infra/README.md` (tabela etapów – następne: symulacja-cień, etap 2b), decyzje w `docs/adr/`.
- Właściciele (`core/src/owners.rs`): właściciel **na kaflu** (decyzja użytkownika, GDD), bajt = numer państwa (0 = niczyj, `nations()[n-1]` = gracz), udziały w prowincjach, hash przyrostowy. Graf sąsiedztwa prowincji: `mapgen::province_graph` (w `Game::neighbours`). Tożsamość: `PlayerId` należy do karty (`client` z biletu albo `/ws?client=` w trybie otwartym), po zerwanym połączeniu ten sam. Start: `ClientMsg::Ready` → serwer wstawia `Intent::BeginSelection` (stempel `SERVER`) → 200 tur wyboru (`ChooseStart`) liczy rdzeń, potem starty losowe; odliczanie `ui/start-timer.ts`. Mapa polityczna: niczyje szare, państwa w kolorach; mapa terenu: granice państw w kolorze państwa i tło (kolor albo flaga), nazwy jak w EU4 – warstwy maluje `paintTerritories` (`render/owners.ts`), flagi `render/flags.ts`, kolory `render/nations.ts`, nazwy `render/territory-names.ts`, opcje pod zębatką (`MapStore.territory`).
- Gospodarka (`core/src/economy.rs`): populacja (wojsko + robotnicy, w tysięcznych osoby) i złoto na państwo, limit 100 × kafle, przyrost ze szczytem przy 55% limitu, suwak `SetTroopShare` – cel udziału wojska w całej populacji, ludzie przechodzą z bezwładnością (`SHIFT_PPM`), podatek od robotników co 300 tur (= miesiąc gry w kalendarzu `ui/calendar.ts`); nazwy prowincji `mapgen/src/names.rs` (`Province::name`, wagi słów według cech prowincji); stałe do strojenia na górze pliku (zasady gry – bez suwaków w panelu debugu, bo muszą być takie same u wszystkich). UI: `ui/resource-bar.*` na środku HUD (`app.html`: wyjście | zasoby + odliczanie | kalendarz `ui/calendar.ts`), rodzaje mapy jako pionowy dok ikon w lewym górnym rogu pod „Menu” (`.dock-top`), ramka prowincji w lewym dolnym tylko przed grą właściwą (zaznaczona prowincja ma pierwszeństwo przed najechaną), w grze okno prowincji `ui/buildings-window.ts` (+ `ui/province-summary.ts`); liczby skrócone k / m (`ui/format.ts`).
- GDD gospodarki (surowce, zaopatrzenie, budynki, centrum dowodzenia, rynek): `docs/gdd/` – zasady i liczby do strojenia; kolejność wdrażania w `docs/gdd/plan.md`.
- Surowce (etap 1 GDD – zrobiony): `core/src/goods.rs`, zapas `Economy::stock`, właściciel prowincji `Owners::province_owner` (> 50% kafli), okno zapasów `ui/stock-window.*` (przeciągane, stan w `MapStore.stockWindow`); suwak podziału przed startem – `Game::pending_share`; kategorie legendy według zaopatrzenia – `SUPPLY_CATEGORIES` (`render/resources.ts`).
- Budynki (etap 2 GDD – zrobiony): `core/src/buildings.rs` (tabele), `Game::buildings` / `check_build` / `rates` (`NationRates`: limit z miastami, obsada, nawozy, wydobycie) / `province_report`; intencje `Build`, `CancelBuild`; zaopatrzenie `Economy::supplies` (`goods::Supply`, start: `START_SUPPLIES` – 50 materiałów, 10 paliwa, 30 podstawowego cywilnego); UI `ui/province-buildings.*` w ramce prowincji, raport przez `watch` w workerze.
- Fabryki (etap 3 GDD – zrobiony): receptury `factories.rs` (`RECIPES`, `produce`, `recipe_text`), produkt i przezbrojenie w `ProvinceBuildings`, `Intent::SetFactory`, `Game::factory_tick` / `command_level` (do etapu 5 zawsze 1) / `factory_report`; UI – sekcja fabryki w `ui/province-buildings.*`, bilans w oknie zapasów.
- Ekspansja (etap 6 bez aneksji – zrobiony): `core/src/war.rs` (stałe, `Attack`, raporty), `Game::attacks` / `war_tick` / `attack_tick` / `frontier` / `check_attack`, `Economy::deployed`; intencje `Attack`, `CancelAttack`; kolejność frontu `war::front_score` (teren, otoczenie, zawirowanie); terytoria na ekranie co turę – `Owners::take_changes` → `changes` w zdarzeniu → `render/territory.ts` (CPU liczy krawędzie i granicę wokół zmian, GPU koloruje); pełne `owners` (flagi, nazwy) najwyżej co `OWNERS_EVERY` tur. UI: `ui/attack-panel.*` (dół, przypinany), `ui/radial-menu.ts` (prawy przycisk), `ui/fronts-panel.*` (+ `MapRenderer::flyTo`), okno budynków `ui/buildings-window.ts`, wspólne okno `ui/floating-window.*`, ikony `ui/goods-icon.ts`, receptury `ui/recipe.*`.
- Fabryka zapamiętuje wejścia składników (`factories::Inputs`: automatycznie do wyczerpania, ręcznie – `Intent::SetFactoryInput`). Budynek kontroluje właściciel jego kafla (`Game::controller`, nie właściciel prowincji); budowy na mapie – `Game::constructions` → `constructions` w zdarzeniu → pierścień postępu w `render/buildings.ts`. Budynki na mapie: miejsca `core/src/slots.rs`, ikony `render/buildings.ts`, okienko `ui/building-popup.ts`, panel fabryki `ui/factory-panel.ts`. Ikony surowców: `glyph` (piktogram – rolnictwo, hodowla, las, chemia) albo `symbol` (metale) w `render/resources.ts` – wspólne dla legendy, mapy i `ui/goods-icon.ts`. W buildzie dev `globalThis.__renderer` (testy w przeglądarce).
- Centrum dowodzenia (część etapu 5 – zrobiona): `Building::CommandCenter` w stolicy (`Game::capital`), `Game::command_level` – odblokowania (zaopatrzenie i jego siła/paliwo `war::MILITARY`, fronty `war::MAX_FRONTS` / `MAX_FRONT_PROVINCES` / `MAX_SEA_LANDINGS`, umocnienia); tabele w `ui/unlocks.ts`, gwiazdki `ui/cd-stars.ts`. Front = kilka prowincji (`Attack::provinces`, Ctrl + klik, dwuklik – atak), armia tubylcza `Game::natives`, desanty przez `Water` (zbiorniki z mapy). Lista brakujących mechanik: `docs/gdd/plan.md` „Czego jeszcze nie ma”.
- Następna sesja: aneksja po utracie stolicy (reszta etapu 6), potem etap 4 (cywile → podatki) albo 5 (centrum dowodzenia) – kolejność uzgodnij z użytkownikiem – zacznij od sekcji README „Gdzie wejdą mechaniki” (stan wyjściowy, proponowana kolejność; kolejność uzgodnij z użytkownikiem).

## Weryfikacja przed commitem

```bash
cargo fmt --all                                    # rustfmt.toml: max_width 120 (CI: --check)
cargo clippy --workspace --all-targets --all-features -- -D warnings   # CI wymaga zera ostrzeżeń
cargo test --workspace --all-features              # 157 testów, zero ostrzeżeń (cargo build --all-features)
cargo run -p game-mapgen --release --features cli -- --seed 1 --out /tmp/m.png   # hashe z tabeli w README
cd web && npm run prep && npx ng build --configuration development && npx ng build
terraform fmt -recursive infra                     # przy zmianach w infra/
```

- CI (`.github/workflows/ci.yml`) robi to samo na każdy push i PR – w tym porównuje hashe seeda 1 z tabelą w README (CLI wypisuje je na stderr). Wyjątki clippy są w `[workspace.lints.clippy]` w głównym `Cargo.toml` (z uzasadnieniem).

- Hashe seeda 1 w CLI i w panelu przeglądarki muszą być identyczne (determinizm native vs wasm). Po zmianie domyślnych parametrów lub algorytmu zaktualizuj tabelę w README; po zmianie algorytmu podbij `GENERATOR_VERSION`.
- Nowe etapy generatora dostają własny RNG (`seed ^ SALT`), żeby nie zmieniać terenu i biomów – dodaj test, że teren/biomy się nie zmieniają.
- Palety mają jedno źródło prawdy tylko w praktyce: zmieniaj je w obu plikach naraz (wygodnie skryptem, który generuje oba fragmenty z jednej tabeli). Lodowiec (maska, cieniowanie) też jest w obu.
- Palety i wygląd są zdublowane w CLI (`crates/mapgen/src/bin/mapgen.rs`) i w `web/src/app/render/terrain.ts` / `render/provinces.ts` – zmieniaj oba.
- Shadery (`render/waves.ts`, `render/inland.ts`, `render/highlight.ts`) Pixi kompiluje w starszym GLSL: brak `fwidth`, unikaj nazw zmiennych typu `patch`. Błędy shadera widać tylko w konsoli przeglądarki – zawsze sprawdź ją w teście.
- Oświetlenie: światło z lewego górnego rogu, cienie w prawo w dół (teren, dno oceanu).
