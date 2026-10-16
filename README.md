# Mapa – generator świata i szkielet gry (Rust/WASM + Angular + Pixi)

[![CI](https://github.com/dawidbis/WebAssembly/actions/workflows/ci.yml/badge.svg)](https://github.com/dawidbis/WebAssembly/actions/workflows/ci.yml) [![Quality Gate](https://sonarcloud.io/api/project_badges/measure?project=dawidbis_WebAssembly&metric=alert_status)](https://sonarcloud.io/summary/new_code?id=dawidbis_WebAssembly)

Proceduralny generator map z seeda (Rust, kompilowany natywnie i do WebAssembly), serwer tur (lockstep) z działającą pętlą tur w przeglądarce i frontend w Angularze z rendererem Pixi. Całość jest wdrożona na AWS (Terraform w repo, wdrażanie z IDE – patrz [Wdrożenie na AWS](#wdrożenie-na-aws)). Mechanik gry jeszcze nie ma – są miejsca, w które wejdą (patrz [Gdzie wejdą mechaniki](#gdzie-wejdą-mechaniki)).

Ten sam seed i te same parametry dają identyczną mapę natywnie, w wasm i u każdego gracza. Typy wiadomości i parametrów są zdefiniowane raz w Ruście, a TypeScript dostaje je automatycznie (`ts-rs`).

## Spis treści

- [Wymagania](#wymagania)
- [Szybki start](#szybki-start)
- [Struktura repozytorium](#struktura-repozytorium)
- [Architektura](#architektura)
- [Generator map](#generator-map)
- [Parametry generatora](#parametry-generatora)
- [Frontend i renderer](#frontend-i-renderer)
- [Interfejs gracza](#interfejs-gracza)
- [Panel debugu i klawisze](#panel-debugu-i-klawisze)
- [CLI `mapgen`](#cli-mapgen)
- [Serwer i protokół](#serwer-i-protokół)
- [Wdrożenie na AWS](#wdrożenie-na-aws)
- [Determinizm i hashe](#determinizm-i-hashe)
- [Kontrakty utrzymywane ręcznie](#kontrakty-utrzymywane-ręcznie)
- [Gdzie wejdą mechaniki](#gdzie-wejdą-mechaniki)
- [Stan projektu i plan](#stan-projektu-i-plan)
- [Czas wczytania](#czas-wczytania)
- [Rozwiązywanie problemów](#rozwiązywanie-problemów)

## Wymagania

| Narzędzie | Wersja | Uwagi |
|---|---|---|
| Rust | stable ≥ 1.85 (edycja 2024) | `rustup target add wasm32-unknown-unknown` |
| wasm-pack | aktualny | `cargo install wasm-pack` |
| Node.js | ≥ 22.22.3 albo ≥ 24 | Angular CLI 22 odmawia pracy na starszym |

Wersje, na których projekt jest sprawdzany: Angular 22.2, TypeScript 6.0, Pixi 8.21, axum 0.8, wasm-bindgen 0.2.

## Szybki start

```bash
cd web
npm install
npm run prep        # typy TS z Rusta (cargo types) + pakiet wasm (wasm-pack)
cd ..

# terminal 1 – serwer (z katalogu głównego, ścieżka do frontendu jest względna)
cargo run -p game-server --features debug -- --dev

# terminal 2 – frontend
cd web && npm start
```

Otwórz `http://localhost:4200`. Mapa powstaje z konfiguracji gry przysłanej przez serwer (`Welcome`), a sekcja „Serwer i gra” w panelu (Esc) pokazuje `online`, rosnące liczby tur i hash stanu. Frontend działa też bez serwera – wtedy mapa powstaje z parametrów domyślnych, połączenie jest `offline`, a `Transport` ponawia je co 5 s (po połączeniu gra rusza na mapie, która już jest, jeśli serwer ma te same parametry).

`npm run prep` trzeba powtórzyć po każdej zmianie w Ruście, która dotyka typów (`MapGenParams`, `MapStats`, protokół) albo generatora (pakiet wasm).

**Build produkcyjny:** `cd web && npm run build`, potem z katalogu głównego `cargo run -p game-server --release` i `http://localhost:3000`. Panel debugu nie istnieje w tym buildzie.

**Testy:** `cargo test --workspace --all-features`. CI (GitHub Actions, `.github/workflows/ci.yml`) na każdy push i PR: `cargo fmt --check`, `clippy -D warnings`, testy, hashe seeda 1 z tabelą w README, build frontendu (dev i prod), `terraform fmt`/`validate`.

## Struktura repozytorium

```
.
├── Cargo.toml              # workspace (+ wyjątki clippy z uzasadnieniem)
├── rustfmt.toml            # max_width 120
├── .cargo/config.toml      # alias `cargo types` + katalog dla typów TS
├── .gitattributes          # LF dla skryptów uruchamianych na Linuksie (*.sh, *.tftpl, kod CloudFront)
├── .github/workflows/ci.yml # CI: fmt, clippy, testy, hashe seeda 1, build frontendu, terraform
├── crates/
│   ├── mapgen/             # generator map z seeda (+ CLI do podglądu PNG)
│   │   └── src/
│   │       ├── lib.rs      # MapGenParams, MapData, Province, Terrain, BiomeType, Biome, generate() (3 fazy)
│   │       ├── layout.rs   # siatka makro-chunków i kontynenty
│   │       ├── relief.rs   # ląd, wybrzeża, rzeźba, klasyfikacja terenu
│   │       ├── biome.rs    # typy klimatu kontynentów i rodzaje biomów z płynnymi przejściami
│   │       ├── hydro.rs    # jeziora i rzeki
│   │       ├── mountains.rs # góry nieprzechodnie i lodowiec (kieszenie lądu)
│   │       ├── ocean.rs    # dno oceanu: szelf, stok, rzeźba dna
│   │       ├── vegetation.rs # lasy
│   │       ├── provinces.rs # prowincje o równej wielkości (faza 2)
│   │       ├── polish.rs   # ostatnie szlify (faza 3): tunele, enklawy
│   │       ├── resources.rs # zasoby prowincji (koniec fazy 3)
│   │       ├── resource_table.rs # tabela szans zasobów (generowana: tools/resources/chances.mjs)
│   │       ├── util.rs     # RNG, szum, pola odległości, percentyle
│   │       └── bin/mapgen.rs # CLI: podgląd PNG, statystyki, hashe
│   ├── core/               # deterministyczny rdzeń
│   │   └── src/
│   │       ├── game.rs     # Game: stan gry, tury i intencje, nadrabianie, hash stanu
│   │       ├── owners.rs   # właściciele kafli (państwa), udziały w prowincjach
│   │       ├── protocol.rs # wiadomości klient ↔ serwer, intencje, GameConfig
│   │       └── lobby.rs    # API lobby: pokoje, MapSettings (ustawienia mapy w lobby)
│   ├── wasm/               # cienkie bindingi wasm-bindgen (GeneratedMap, WasmGame)
│   ├── ticket/             # bilety dołączenia do pokoju (JWT Ed25519) + CLI `ticket` (klucze, bilety testowe)
│   ├── meta/               # lobby: Lambda w Ruście – app.rs (logika API), store.rs (DynamoDB / pamięć), main.rs
│   └── server/             # game-server (axum): main.rs, rooms.rs (rejestr pokoi), room.rs (pokój – aktor),
│                           #   heartbeat.rs (do lobby, cecha `aws`), limits.rs (limity połączeń i wiadomości)
└── web/                    # Angular 22
    ├── proxy.conf.json     # /ws → serwer Rust w trybie dev
    └── src/
        ├── generated/      # typy z ts-rs (generowane, nie w repo)
        ├── wasm/pkg/       # wynik wasm-pack (generowany, nie w repo)
        └── app/
            ├── worker/     # web worker: ładuje wasm, generuje mapę, prowadzi grę (WasmGame); protocol.ts
            ├── game/       # serwisy: GameSession (pętla tur), Lobby, WorkerBridge, MapStore, Transport
            ├── render/     # czysty TS + Pixi: map-renderer.ts (warstwy, kamera), malowanie w workerze
            │               #   (paint.worker.ts, painter.ts; terrain.ts, provinces.ts, resources.ts),
            │               #   shadery (waves.ts, inland.ts, highlight.ts), właściciele (owners.ts), yeti, ikony zasobów
            ├── ui/         # interfejs gracza: górny pasek, lobby, ramka prowincji, legenda zasobów,
            │               #   napis ładowania, komunikat o grze
            └── debug/      # panel deweloperski (tylko w buildzie dev)
tools/
├── cdp.mjs                 # wspólne dla testów: Chromium bez okna + surowe CDP
├── deploy/                 # wdrożenie na AWS: all, infra, frontend, game-server, smoke (wspólne: aws.mjs)
├── loadtest/               # pomiar czasu wczytania (dławienie sieci i CPU)
├── lobby/                  # test end-to-end lobby (atrapa /api + game-server z biletami + Chromium)
├── lockstep/               # test pętli tur na kilku kartach przeglądarki
└── resources/              # chances.mjs – tabela szans zasobów (→ resource_table.rs, --md do GDD)
infra/                      # Terraform: bootstrap, envs/prod, modules (dns, frontend, game_server, meta,
                            #   monitoring) – infra/README.md
docs/adr/                   # decyzje architektoniczne (ADR 0001–0007)
docs/gdd/                   # GDD gospodarki: surowce, zaopatrzenie, budynki, rynek, plan wdrażania
.vscode/tasks.json          # zadania wdrożenia na AWS (Terminal → Run Task…)
```

## Architektura

Kierunek zależności: `mapgen` ← `core` ← (`wasm`, `ticket` ← `server`). `web` nie importuje Rusta bezpośrednio, tylko wygenerowane typy i pakiet wasm.

Podział odpowiedzialności we frontendzie:

- **Worker** (`worker/game.worker.ts`) ładuje wasm raz i generuje mapę **w trzech fazach**: najpierw teren, biomy, wodę i lasy (`generate_base` – od razu na ekran), potem w osobnym zadaniu prowincje (`generate_provinces`, najdłuższy etap), a na końcu „ostatnie szlify” (`polish`, faza 3: tunele i enklawy – patrz „Dostęp do prowincji”; wiadomość `polished`, z nową tablicą `province` tylko wtedy, gdy enklawy zmieniły numerację). Do końca fazy 3 prowincji nie da się zaznaczyć. Prowincje przychodzą wiadomością `provinces` z tym samym `id`; renderer przebudowuje wtedy tylko warstwy prowincji. Nowsze żądanie mapy pomija prowincje starszej. Wynik jest identyczny z `generate` (test). Bufory mapy kopiuje z wasm i wysyła jako *transferable*; sama mapa zostaje w pamięci wasm (`GeneratedMap`), bo może z niej powstać gra. Liczy też hashe i odległość kafli oceanu od brzegu (do animacji fal).
- **Pętla gry** (`game/game-session.ts` + worker) – patrz [Pętla tur w przeglądarce](#pętla-tur-w-przeglądarce).
- **Renderer** (`render/`) to czysty TS + Pixi, poza Angularem. Maluje teren do tekstur, rysuje fale shaderem i obsługuje kamerę.
- **Angular** obsługuje tylko UI i stan w sygnałach (`MapStore`). Duże bufory mapy nigdy nie przechodzą przez change detection – sygnał trzyma referencję do gotowego obiektu.
- **Panel debugu** ładuje się dynamicznie tylko gdy `DEV_TOOLS = true` (opcja `define` w `angular.json`). W produkcji esbuild wycina go razem z jego chunkiem.

## Generator map

`generate(params)` przepuszcza parametry przez `sanitized()` (przycięcie do bezpiecznych zakresów) i uruchamia kolejne etapy:

1. **Układ** (`layout.rs`) – siatka makro-chunków. Kontynenty rosną z rozstawionych centrów; dwa różne kontynenty nigdy się nie stykają (nawet po przekątnej), więc między nimi zawsze jest co najmniej jeden chunk wody.
2. **Rzeźba** (`relief.rs`) – kształt lądu z pola „odległość od chunku wodnego” odkształconego domain warpem i fBm; przy `keepOffEdges` poszarpana bariera trzyma ląd z dala od krawędzi mapy. Usuwanie wysp mniejszych niż `minIslandArea` i zasypywanie kałuż. Pasma górskie wzdłuż linii zerowych wolnozmiennego szumu, pocięte na masywy, z ridged noise w środku. Klasyfikacja równiny/wyżyny/góry **percentylami**, więc proporcje terenu są stałe niezależnie od seeda.
3. **Biomy** (`biome.rs`) – patrz niżej.
4. **Hydrologia** (`hydro.rs`) – pojezierza z limitem rozmiaru jeziora (tafla płaska); rzeki: Priority-Flood wypełnia dołki, kierunek najbardziej stromego spadku (D8), akumulacja przepływu, źródła na szczytach rozstawione w odstępach, rzeki poszerzają się z przepływem i meandrują.
5. **Góry nieprzechodnie** (`mountains.rs`) – patrz niżej.
6. **Dno oceanu** (`ocean.rs`) – patrz niżej.
7. **Roślinność** (`vegetation.rs`) – patrz niżej.
8. **Prowincje** (`provinces.rs`) – patrz niżej.

Biomy, ocean, roślinność i prowincje mają **własne strumienie losowości** (seed XOR stała), więc ich ustawienia nie zmieniają kształtu lądu, rzek ani jezior. Tak samo biomy nie zależą od ustawień oceanu ani lasów, a nic nie zależy od ustawień prowincji.

Szum jest liczony na siatce co 2 kafle i interpolowany (`CoarseField`) – około 4× mniej obliczeń bez widocznej straty.

### Biomy

Dwa poziomy: **5 typów klimatu** i **13 rodzajów biomów** (rodzaj zawsze należy do jednego typu):

| Typ | Rodzaje | Jak wyznaczane |
|---|---|---|
| Tropikalny | las deszczowy, sawanna | las deszczowy w wilgotniejszej części, sawanna w suchszej |
| Suchy | pustynia, step | pustynia w suchszej części, step w wilgotniejszej |
| Umiarkowany | śródziemnomorski, subtropikalny, oceaniczny | oceaniczny w najchłodniejszej części; z reszty śródziemnomorski w suchszej, subtropikalny w wilgotniejszej |
| Kontynentalny | gorące lato, ciepłe lato, borealny | pasy od bieguna ciepła: gorące lato → ciepłe lato → borealny (mocno zielona tajga bez śniegu) |
| Polarny | tajga, tundra, lądolód | pasy od bieguna ciepła: tajga (las jak borealny, chłodniejszy grunt, góry oblodzone od połowy) → tundra (góry oblodzone w 3/4, zamarznięte jeziora i rzeki) → lądolód (całe zamarznięte; przy brzegu lód morski; przechodni i w prowincjach, a z `glacier` – lodowiec: nieprzechodni i niczyj) |

Rodzaje tego samego typu wyglądają podobnie; w typach tropikalnym, suchym i polarnym różnice są wyraźne (pustynia/step, tundra/lądolód), w umiarkowanym i kontynentalnym – subtelne.

**Warianty typów.** Obszar typu na kontynencie nie zawsze ma wszystkie rodzaje: losowany jest **wariant** – zbiór rodzajów typu (od jednego do wszystkich), np. polarny „sam lądolód” albo umiarkowany „śródziemnomorski + subtropikalny”. Szanse wariantów to wagi `biomeVariants` (27: tropikalny 3, suchy 3, umiarkowany 7, kontynentalny 7, polarny 7; układ – `variant_index` w `lib.rs` / `VARIANTS` w `render/terrain.ts`). Domyślnie pełny wariant ma wagę 1.0, a każdy niepełny 0.15. Rodzaje spoza wariantu mają udział 0, a pozostałe dzielą obszar w proporcjach udziałów. Warianty mają własny strumień losowości, więc ich wagi nie zmieniają typów kontynentów.

- **Typ na kontynent.** Każdy kontynent dostaje typ główny, a z szansą `biomeMixChance` także drugi. Losowanie ważone szansami typów (`biomeTropical` … `biomePolar`). Przy `biomeLatitude > 0` (wpływ biegunów klimatu) szanse przesuwa położenie kontynentu względem **biegunów klimatu**: biegun zimna leży przy górnej albo dolnej krawędzi (losowo), a biegun ciepła naprzeciwko, po przekątnej mapy. Chłód kontynentu jest rozciągnięty na pełny zakres – najzimniejszy kontynent „leży na biegunie zimna”, najcieplejszy na biegunie ciepła. Idealny chłód typów od najcieplejszego: tropikalny, suchy, umiarkowany, kontynentalny, polarny (`IDEAL_COLDNESS` w `biome.rs`). Twarda reguła: polarny tylko po zimnej połowie, tropikalny i suchy tylko po ciepłej, umiarkowany i kontynentalny wszędzie.
- **Dozwolone pary typów:** drugi typ wybierany jest tylko z par dozwolonych w `biomePairs` (maska bitowa, bit = indeks w `BIOME_PAIRS`). Domyślnie – sąsiedzi w klimacie:

  | Para | Domyślnie |
  |---|---|
  | Tropikalny + Suchy, Tropikalny + Umiarkowany | ✅ |
  | Suchy + Umiarkowany, Suchy + Kontynentalny | ✅ |
  | Umiarkowany + Kontynentalny, Kontynentalny + Polarny | ✅ |
  | Tropikalny + Kontynentalny / Polarny, Suchy + Polarny, Umiarkowany + Polarny | ❌ |

- **Przejście typów:** granica między typami to pofalowana szumem linia w poprzek kontynentu (przy wpływie biegunów chłodniejszy typ leży bliżej bieguna zimna). Strefa przejścia o szerokości `biomeTransition` kafli miesza oba typy płynnie (smoothstep) z przeplatającymi się płatami. Udział drugiego typu (`biomeSecondaryShare`) jest dobierany percentylem.
- **Rodzaj w typie** wynika z dwóch pól na kaflu (w kaflach, pofalowanych szumem – `biomeKindRoughness`): **chłodu** (o ile kafel jest bliżej bieguna zimna niż ciepła) i **suchości** – odległości od morza z wagą `biomeCoastInfluence` plus wielkoskalowych stref wilgotności (reszta). Przy małym wpływie morza pustynia, sawanna itd. sięgają wybrzeża, a cała wyspa może być jednym rodzajem. Progi są dobierane **percentylem** w obszarze typu na kontynencie (`biomeRainforestShare`, `biomeDesertShare`, `biomeOceanicShare`, `biomeMediterraneanShare`, `biomeHotSummerShare`, `biomeBorealShare`, `biomePolarTaigaShare`, `biomeIceShare`), więc każdy taki obszar ma wszystkie rodzaje swojego wariantu w zadanych proporcjach, niezależnie od seeda. Przejście między rodzajami ma szerokość `biomeKindTransition` kafli.
- **Lodowiec (`glacier`, domyślnie włączony):** kafle lądu z co najmniej połową wagi lądolodu są nieprzechodnie i niczyje jak góry (razem z kieszeniami dostępnego lądu stykającymi się z lodem i mniejszymi niż `glacierPocket` kafli, domyślnie 150 – mniej zamkniętych „bąbli” lądu w lodzie; maska lodowca idzie do renderera jako `MapData.glacier`), a renderer rysuje je jako lodowiec – wyraźna krawędź, oświetlona krawędź, klif i cień (patrz „Frontend i renderer”). Teren się nie zmienia. Wyłączony = lądolód to zwykły ląd w prowincjach, z miękkimi przejściami.
- **Lód morski:** kafle oceanu do `iceShelfWidth` kafli od lądu, których biom (dziedziczony z najbliższego lądu) to lądolód, są zamarznięte (`MapData.seaIce`). Nadal to ocean – nieprzechodni dla jednostek lądowych; zmienia wygląd, a fale łamią się na krawędzi lodu.
- **Wynik na kafel:** `biome` (rodzaj dominujący – liczy się w rozgrywce i w hashu stanu gry), `biomeLayers` (6 bajtów: typ i dwa płynne parametry rodzaju σ1, σ2 dla typu głównego i drugiego typu kontynentu) i `biomeMix` (udział drugiego typu, 0..255). Wagi wszystkich rodzajów liczy z tego `kind_weights` (Rust) / `kindWeights` (`render/terrain.ts`) – kolory i lasy mieszają się według nich, więc nie ma szwów także tam, gdzie granica typów spotyka granice rodzajów (do sześciu rodzajów w jednym kaflu). Woda dostaje biom najbliższego lądu.
- Cały spójny ląd należy do jednego kontynentu (głosowanie chunków), więc w obrębie lądu nie ma twardych szwów.

### Dno oceanu

Głębokość kafla oceanu (`shade`, 0..255) zależy od odległości od lądu:

- **szelf** – płytki pas przy brzegu o średniej szerokości `shelfWidth`, zmiennej szumem (`shelfVariation`): szerokie ławice obok urwisk,
- **stok kontynentalny** – spadek do głębi, stromy przy dużym `slopeSteepness`,
- **równina abisalna** z rzeźbą dna (`seabedRelief`): podwodne grzbiety, rowy i góry podwodne.

### Roślinność

**Lasy.** Każdy kafel lądu ma gęstość lasu `forest` (0..255; ≥ 128 = kafel leśny w rozgrywce). Typ lasu nie jest zapisywany osobno – wynika z rodzaju biomu, więc w strefach przejścia las przechodzi płynnie tak jak kolory:

| Rodzaj | Las (domyślny udział) |
|---|---|
| Las deszczowy | dżungla, bardzo gęsta (0.95) |
| Sawanna | zagajniki, głównie przy wodzie (0.12) |
| Pustynia | oazy tylko przy wodzie (0.03) |
| Step | zagajniki, głównie wzdłuż rzek (0.08) |
| Śródziemnomorski / subtropikalny / oceaniczny | liściasty (0.25 / 0.55 / 0.4) |
| Gorące lato / ciepłe lato | liściasty i mieszany (0.35 / 0.5) |
| Borealny | tajga bez śniegu, rzednie szybciej z wysokością (0.75) |
| Tajga (polarna) | tajga jak borealna, odrobinę jaśniejsza (0.75) |
| Tundra | pojedyncze krzewy (0.04) |
| Lądolód | brak (0) |

Gdzie rośnie las: zwarte masywy z szumu (`forestClumping`), więcej przy rzekach, jeziorach i wybrzeżu (`forestMoisture`; na sawannie, stepie i pustyni ta waga jest dużo większa), mniej na wyżynach, nigdy na górach. Udział lasu w rodzaju (`forestRainforest` … `forestIceSheet`) jest ustalany **percentylem** wśród kafli bez gór, więc nie zależy od seeda. Próg jest mieszany między rodzajami według ich wag w kaflu, więc na granicy biomów nie ma szwów. Skraj lasu jest szeroki i miękki – renderer rozbija go na pojedyncze kafle koron.

### Góry nieprzechodnie

Góry **blokują ruch jednostek i są niczyje**: nie należą do żadnej prowincji – tak jak woda (ocean, jeziora i **rzeki**). W danych: przechodni jest tylko kafel z `province > 0`; kafel lądu z `province == 0` to góry. Lądolód jest zwykłym (przechodnim) lądem i należy do prowincji – chyba że włączony jest lodowiec (`glacier`): wtedy kafel lądu z `province == 0` to góry albo lodowiec. Góry w typie polarnym są oblodzone: w tajdze od połowy wysokości masywów, w tundrze w 3/4, na lądolodzie całe (palety: `snowStart` – od jakiej wysokości góry bieleją, `snowFull` – od jakiej są całe w lodzie).

Generator nie wycina przełęczy (wyglądały sztucznie). Obszar odcięty górami jest dla prowincji osobnym lądem, jak wyspa; przejścia przez góry (drążenie tunelu, desant) będą mechaniką rozgrywki – generator tylko pilnuje, żeby do każdej prowincji dało się dojść (patrz „Dostęp do prowincji”). Kieszenie dostępnego lądu przy górach (także zamknięte między górami a rzeką) mniejsze niż 40 kafli stają się górami (`mountains.rs`, bez losowości).

### Prowincje

Dostępny ląd (bez gór i bez wody – rzeki też nie należą do prowincji) jest podzielony na prowincje – statyczny podział administracyjny: granice nie zmieniają się po wygenerowaniu. Wynik: `MapData.province` (numer prowincji na kaflu, od 1; 0 = woda albo góry) i `MapData.provinces` (lista `Province` ze **stałymi właściwościami** z GDD):

| Pole | Znaczenie |
|---|---|
| `id` | numer prowincji (jak w `MapData.province`) |
| `area` | wielkość – liczba kafli lądu |
| `biome` | biom – rodzaj (`Biome`) dominujący na największej liczbie kafli prowincji |
| `coastal`, `river`, `lake`, `mountains` | czy prowincja graniczy (sąsiedztwo 4) z oceanem, rzeką, jeziorem, górami |
| `centerX`, `centerY` | kafel środka (należy do prowincji) – pod etykietę albo stolicę |
| `tunnel` | 0 = dostęp zwykły; > 0 = prowincja zamknięta górami albo lodowcem, dostęp tylko tunelem tej długości (kafle) – z ostatnich szlifów |
| `resource` | zasób prowincji (numer z `RESOURCE_NAMES`, patrz „Zasoby prowincji”) – z ostatnich szlifów |
| `name` | nazwa prowincji, np. „Końska Równina” (`names.rs`, koniec ostatnich szlifów): przymiotnik + rzeczownik ze słownika z zgodnym rodzajem (w co czwartej szyk „Marchia Wschodnia”). Dobór słów to **wagi** według cech prowincji – biom, zalesienie (≥ 45% / ≥ 20% / < 8%), wyżyna albo nizina, wielkość względem średniej, wyspa (bez sąsiadów po lądzie), morze, góry, rzeka, jezioro, zasób i położenie środka na mapie (północ, południe, wschód, zachód, środek): słowo ma wymagania (`need` – wszystkie, `any` – choć jedno), wykluczenia (`avoid`) i premie (`like`); niespełnione wymagania = waga 0, tematyczne słowo `THEMED` (8), ogólne `GENERIC` (2), +`LIKE` (6) za każdą pasującą cechę; przymiotnik o tym samym temacie co rzeczownik ×¼, o tym samym rdzeniu – wcale. Niepowtarzalna na mapie (w razie powtórki – „II”, „III”). Na ekranie z numerem: „Końska Równina #123” |

#### Dostęp do prowincji (ostatnie szlify, `polish.rs`)

Do każdej prowincji musi się dać dostać – normalnie albo tunelem – a kafle prowincji są ciągłe:

- **Ciągłość** (faza 2): mała wyspa bez własnego zalążka dołącza do najbliższej prowincji **tylko przez wodę** (morze, jezioro, rzekę). Kieszeń lądu zamknięta w górach albo lodowcu nie staje się już odciętym kawałkiem cudzej prowincji – dostaje własną. Odłamki prowincji to więc tylko wyspy osiągalne z wody i brzegi tej samej prowincji po obu stronach rzeki (test na pełnej mapie).
- **Grupy** (faza 3): prowincje łączą się, gdy ich kafle stykają się bokiem albo leżą naprzeciw siebie w poprzek rzeki (do 4 kafli – rzeka nie jest kaflem prowincji ani przechodnim terenem, ale wąska rzeka to przyszły most albo bród), a prowincje nadmorskie łączą się przez morze. Grupa z morzem jest zdrowa; każda inna (np. dwie albo trzy prowincje w dolinie widzące tylko siebie) jest odcięta.
- **Tunel**: Dijkstra od wszystkich osiągalnych prowincji przez kafle gór, lodowca i rzek (**nie** przez jeziora ani morze). Odcięta grupa osiągnięta w `tunnelMax` kafli (domyślnie 60) dostaje `tunnel` = długość i sama staje się źródłem dla dalszych grup.
- **Enklawa**: grupa dalej niż `tunnelMax` przestaje być prowincjami – ląd niczyj (`MapData.enclaves`: wielkość i środek), prowincje są numerowane od nowa. W enklawie mieszka pikselowy yeti (easter egg, `render/creatures.ts`; wielkość: `YETI_PIXEL` – kafle na piksel, domyślnie 0,5, suwak „Wielkość yeti” w panelu debugu, sekcja Widok); w ramce po najechaniu „Enklawa”, na mapie politycznej szara jak góry.

Statystyki: `MapStats.tunnelProvinces`, `MapStats.enclaves` (panel debugu). Na domyślnych ustawieniach mapy mają 0–11 prowincji z tunelem i zwykle 0 enklaw (seed 10: jedna enklawa w lodowcu i trzy prowincje ze wspólnym tunelem).

- **Równa wielkość.** Każda prowincja ma podobną liczbę kafli lądu – średnio `provinceSize`; typowe odchylenie ok. 11% (zależy od `provinceRounds`). Teren, biomy i lasy wpływają tylko na przebieg granic, nie na wielkość.
- **Liczba prowincji** na każdym lądzie = powierzchnia lądu / `provinceSize`, z poprawką tak, żeby prowincje mieściły się między `provinceMinSize` a `provinceMaxSize` kafli.
- **Wzrost.** Zalążki startują z pocięcia krzywej Hilberta na kawałki o równej liczbie kafli. Potem w kilkudziesięciu rundach prowincje rosną od zalążków (Dijkstra, sąsiedztwo 8, kolejka kubełkowa), prowincja za duża dostaje „handicap” i startuje później, za mała – wcześniej, a zalążki przesuwają się do środka prowincji (Lloyd). Większość rund liczy się na siatce 2 × 2 (4× szybciej), ostatnie w pełnej rozdzielczości.
- **Naturalne granice.** Woda (także rzeki) i góry są nieprzekraczalne, więc granice biegną rzekami; wspinaczka jest droga, więc granice chętnie biegną też grzbietami wyżyn (`provinceNaturalBorders`). Na siatce zgrubnej blok z rzeką jest drogi do wejścia, więc już wstępny rozrost nie przeskakuje rzek. Woda i góry są nieprzekraczalne, więc wyspy i półwyspy za cieśniną mają własne prowincje. Małe wyspy (mniejsze niż `provinceMinSize`) dołączają przez morze do najbliższej prowincji, a zbyt odległe dostają własną.
- **Kształt.** Szum w kosztach i drobne przesunięcie granic (bez przeskakiwania rzek) dają nieregularne granice jak prawdziwe granice powiatów (`provinceRoughness`). Każda prowincja jest spójna na swoim lądzie; okruchy i za małe prowincje dołączają do sąsiadów.

Przy domyślnych ustawieniach (1400 × 1400, dużo lądu) generowanie trwa natywnie ok. 3,6 s, z czego większość to prowincje.

#### Zasoby prowincji (`resources.rs`, koniec `polish`)

Każda prowincja ma jeden z 39 zasobów (realia dwudziestolecia; uran pod przyszłą bombę atomową) (`Province.resource`, nazwy w `RESOURCE_NAMES`). Szanse to tabela `[biom][ukształtowanie][zasób]` w dziesiątych częściach procenta (`resource_table.rs`); każda kolumna sumuje się do 100%, a zero oznacza zasób niemożliwy (np. ryż w tundrze). Ukształtowania prowincji: płaski albo wyżyna (przewaga kafli), góry (`mountains`), rzeki i jeziora (`river` lub `lake`), brzeg (`coastal`) – prowincja kwalifikuje się do kilku naraz, a jej szanse to średnia tych kolumn dla jej biomu. Losowanie ma własny strumień losowości, więc nie zmienia terenu ani prowincji.

Tabelę generuje `node tools/resources/chances.mjs` (nie edytuj `resource_table.rs` ręcznie): zasoby dominujące z tabeli GDD (lista `U` w skrypcie) biorą 43–91% kolumny, resztę rozdzielają reguły – zasoby biologiczne według przydatności klimatu, kopaliny według ukształtowania (bez zależności od klimatu). `--md plik` wypisuje tabele w markdown do dokumentu GDD. Złoto jest bardzo rzadkie (tylko kolumny gór i rzek) i `resources.rs` gwarantuje co najmniej jedną prowincję ze złotem na mapie – walka o nie od startu. Na 20 mapach domyślnych najczęstsze są zboża i drewno (ok. 11% prowincji), molibden ma ok. 4 prowincje na mapę, złoto 1–4 (średnio 1,5); kauczuk, kawa i herbata (tropiki) bywają nieobecne.

## Parametry generatora

Wszystkie pola `MapGenParams` w camelCase (tak jak w JSON i TS). Wartości spoza zakresu są przycinane.

**Rozmiar i układ**

| Pole | Domyślnie | Działanie |
|---|---|---|
| `seed` | 1 | seed (u32) |
| `width`, `height` | 1400, 1400 | rozmiar w kaflach (64..4096); przy losowej mapie najlepszy jest kwadrat |
| `chunkCols`, `chunkRows` | 10, 10 | siatka makro-chunków |
| `continents` | 3 | liczba kontynentów (1 = jeden duży ląd) |
| `landRatio` | 0.65 | udział chunków lądowych; reszta to chunki wodne |
| `sizeVariance` | 0.5 | różnice wielkości kontynentów |
| `minIslandArea` | 50 | wyspy mniejsze niż tyle kafli są usuwane |

**Wybrzeże i rzeźba**

| Pole | Domyślnie | Działanie |
|---|---|---|
| `coastRoughness` | 0.5 | poszarpanie wybrzeża i odkształcenie kształtów |
| `keepOffEdges`, `edgeMargin` | false, 12 | ląd z dala od krawędzi mapy i chunków wodnych |
| `mountainShare`, `highlandShare` | 0.12, 0.22 | udział gór i wyżyn w lądzie |
| `rangeScale` | 2.0 | skala pasm górskich (większa = dłuższe, szersze) |

**Woda śródlądowa**

| Pole | Domyślnie | Działanie |
|---|---|---|
| `rivers`, `riverCount` | true, 30 | rzeki i liczba źródeł |
| `lakes`, `lakeAmount` | true, 0.5 | jeziora i ilość pojezierzy |
| `minLakeArea`, `maxLakeArea` | 40, 2500 | dopuszczalny rozmiar jeziora |

**Biomy**

| Pole | Domyślnie | Działanie |
|---|---|---|
| `biomes` | true | wyłączone = cały ląd umiarkowany oceaniczny |
| `biomeTropical`, `biomeDry`, `biomeTemperate`, `biomeContinental`, `biomePolar` | 0.4, 0.45, 0.6, 0.55, 0.35 | szanse typów (wagi względne; panel pokazuje je jako procent sumy) |
| `biomeLatitude` | 0.6 | wpływ biegunów klimatu (0 = typy losowe) |
| `biomeMixChance` | 0.5 | szansa, że kontynent ma dwa typy |
| `biomePairs` | patrz tabela par | maska dozwolonych par typów |
| `biomeSecondaryShare` | 0.4 | średni udział drugiego typu (losowany ±25%, max 0.5) |
| `biomeTransition` | 60 | szerokość strefy przejścia typów (kafle) |
| `biomeRoughness` | 0.5 | pofalowanie granicy typów i przeplatanie płatów |
| `biomeRainforestShare` | 0.5 | tropikalny: udział lasu deszczowego (reszta sawanna) |
| `biomeDesertShare` | 0.5 | suchy: udział pustyni (reszta step) |
| `biomeOceanicShare`, `biomeMediterraneanShare` | 0.35, 0.5 | umiarkowany: udział oceanicznego; z reszty udział śródziemnomorskiego (dalej subtropikalny) |
| `biomeHotSummerShare`, `biomeBorealShare` | 0.33, 0.33 | kontynentalny: gorące lato i borealny (środek – ciepłe lato) |
| `biomePolarTaigaShare`, `biomeIceShare` | 0.3, 0.45 | polarny: tajga i lądolód (środek – tundra) |
| `biomeVariants` | pełny 1.0, inne 0.15 | szanse wariantów typów (27 wag; panel: sekcja „Warianty biomów”, procent w obrębie typu) |
| `iceShelfWidth` | 8 | lód morski: do tylu kafli od lądolodu ocean zamarza (0 = brak) |
| `glacier` | true | lądolód jako lodowiec: nieprzechodni, bez prowincji, rysowany z wyraźną krawędzią i cieniem |
| `glacierPocket` | 150 | z lodowcem: kieszenie lądu przy lodzie mniejsze niż tyle kafli też są lodowcem |
| `tunnelMax` | 60 | najdłuższy tunel do odciętej grupy prowincji (kafle); dalej – enklawa |
| `biomeCoastInfluence` | 0.4 | wpływ odległości od morza na suchość (1 = wybrzeża zawsze wilgotne, 0 = same strefy wilgotności) |
| `biomeKindTransition` | 40 | szerokość przejścia między rodzajami (kafle) |
| `biomeKindRoughness` | 0.5 | pofalowanie granic rodzajów |

**Ocean**

| Pole | Domyślnie | Działanie |
|---|---|---|
| `shelfWidth` | 14 | średnia szerokość szelfu (kafle) |
| `shelfVariation` | 0.6 | zmienność szerokości szelfu |
| `slopeSteepness` | 0.7 | stromość stoku (1 = urwisko) |
| `seabedRelief` | 0.5 | rzeźba dna |

**Lasy**

| Pole | Domyślnie | Działanie |
|---|---|---|
| `forest` | true | wyłączone = brak lasów |
| `forestRainforest` … `forestIceSheet` (13, po jednym na rodzaj) | patrz tabela lasów | docelowy udział lasu w lądzie rodzaju (bez gór) |
| `forestClumping` | 0.85 | zwartość: 0 = drobne kępy, 1 = duże masywy |
| `forestMoisture` | 0.5 | jak mocno las ciągnie do wody |

**Prowincje**

| Pole | Domyślnie | Działanie |
|---|---|---|
| `provinces` | true | podział lądu na prowincje |
| `provinceSize` | 600 | docelowa (średnia) wielkość prowincji w kaflach lądu |
| `provinceMinSize`, `provinceMaxSize` | 120, 4000 | najmniejsza i największa prowincja (kafle) |
| `provinceNaturalBorders` | 0.6 | jak mocno granice trzymają się grani (i rzek na siatce zgrubnej) |
| `provinceRoughness` | 0.5 | nieregularność granic (0 = gładkie, zaokrąglone) |
| `provinceRounds` | 14 | dokładność wyrównania wielkości: rundy na siatce zgrubnej (14 ≈ odchylenie 11%) |

## Frontend i renderer

- **Teren** (`render/terrain.ts`) – każdy z 13 rodzajów ma własną paletę: równiny i wyżyny (gradient wg wysokości), skały i śnieg/lód na górach (`snowStart`–`snowFull`), jeziora i rzeki (na tundrze i lądolodzie zamarznięte – bez animacji). Kolory rodzajów są mieszane według ich wag w kaflu (`kindWeights`). Rzeźbę lądu cieniuje światło z lewego górnego rogu.
- **Ocean** – paleta głębokości z wyraźnym, jasnym szelfem, jasna linia brzegu, słabe cieniowanie dna i **izobaty** (linie jednakowej głębokości na 5 stałych poziomach, bardzo przezroczyste – `CONTOUR_OPACITY`).
- **Lasy** – korony drzew w kolorze zależnym od rodzaju biomu (liściasty, oliwkowy śródziemnomorski, zielona tajga borealna, polarna tajga i krzewy tundry przyprószone śniegiem, ciemna dżungla, zagajniki, palmy oaz) z ziarnistą teksturą; na skraju lasu pojedyncze kafle koron.
- **Widok „mapa biomów”** – płaskie kolory rodzajów (rodzina barw na typ) zamiast pełnego stylu, do strojenia (las jako ciemniejszy odcień).
- **Fale brzegowe** (`render/waves.ts`) – nakładka rysowana shaderem GLSL co klatkę nad terenem: grzbiety przyboju płyną w stronę brzegu i wygasają dalej od lądu, a przy samej linii brzegu pulsuje piana. To czysto wizualny efekt – nie zmienia danych mapy. Gdy system prosi o ograniczenie ruchu (`prefers-reduced-motion`), fale są domyślnie wyłączone.
- **Rzeki i jeziora** (`render/inland.ts`) – animacja rysowana shaderem od ok. 1,5 px na kafel (w pełni od 3,5): po rzekach płyną z prądem jasne smugi i zmarszczki (ok. 3 kafle/s, w stronę ujścia), a na jeziorach powoli przesuwają się delikatne zmarszczki i falująca piana przy brzegu. Kierunek nurtu daje generator (`MapData.riverFlow` – odległość do ujścia wzdłuż rzeki; dopływ dziedziczy odległość rzeki, do której wpada). Włączana razem z falami brzegowymi (klawisz W), jasność suwakiem „Rzeki i jeziora”.
- **Granice prowincji** (`render/provinces.ts`) – nakładka z półprzezroczystych szarych kafli (krycie suwakiem w górnym pasku, domyślnie 0.3 – teren pod granicą pozostaje widoczny): granicą jest kafel, którego prawy albo dolny sąsiad należy do innej prowincji, więc linia ma grubość jednego kafla (bez wektorów i linii na siatce). Brzeg morza i jezior nie jest granicą. Rysowana nad terenem, pod falami.
- **Lodowiec** – gdy mapa ma `glacier` (przełącznik „Lądolód jako lodowiec” w panelu, sekcja Biomy): kafel z co najmniej połową wagi lądolodu jest w całości lodem, a lądolód i lód morski wyglądają jak płyta odstająca od lądu – oświetlona krawędź od lewej góry, ciemniejsza ściana klifu od prawej i od dołu, cień rzucany w prawo w dół na ląd i wodę (`iceMask`, `iceShade` w `render/terrain.ts`, tak samo w CLI). Bez `glacier` lądolód przechodzi miękko w sąsiednie biomy. Lód morski jest zawsze jasną taflą zamiast oceanu. Na mapie politycznej lodowiec jest biały, a w ramce prowincji po najechaniu – „Lodowiec: nieprzechodni i niczyj”. Maska lodu na lądzie to `MapData.glacier` z generatora (lądolód i kieszenie w lodzie), więc kieszenie wyglądają jak lód.
- **Mapa polityczna** – prowincje niczyje szare (`POLITICAL_UNOWNED`, granice `POLITICAL_BORDER` w `render/provinces.ts`, tak samo w CLI), góry szarobrązowe, lodowiec biały, rzeki i jeziora w kolorze wody; bez rzeźby, lasów, rzek i animacji wody. W grze na wierzchu nakładka właścicieli (`render/owners.ts`): kafle państw w ich kolorach (`NATION_COLORS` w `render/nations.ts`, 12 kolorów po kolei), granica państwa ciemniejsza (`NATION_EDGE`), granica prowincji wewnątrz państwa lekko przyciemniona (`PROVINCE_EDGE`).
- **Państwa na mapie terenu** (`render/owners.ts`, warstwy z workera malującego) – granice państw w kolorze państwa (krawędź pełna, `borderWidth` kafli w głąb – suwak „Grubość granic państw” w panelu debugu; brzeg morza, jezior i rzek też jest granicą) i półprzezroczyste tło: kolor państwa albo **flaga** (`render/flags.ts` – na razie stała paleta 12 flag, flaga `n` należy do państwa `n`; wzory liczone z (u, v), więc te same flagi maluje worker i rysuje UI – `flagDataUrl`). Flaga jest rozciągnięta z zachowaniem proporcji 3:2 na każdy spójny kawałek terytorium.
- **Nazwy państw** (`render/territory-names.ts`, jak w EU4) – nazwa gracza wielkimi literami wzdłuż głównej osi największego kawałka terytorium (PCA kafli), lekko wygięta po paraboli przez środki terytorium w poprzek osi (wygięcie najwyżej 12% długości), wielkość według grubości i długości terytorium (`SIZE_OF_THICKNESS`, `SIZE_OF_SPACING`). Widoczne na mapie terenu i politycznej; przy dużym przybliżeniu znikają (`FADE_OUT_FROM`/`FADE_OUT_TO` – wielkość litery na ekranie w px), za małe – też.
- **Podświetlenie prowincji** (`render/highlight.ts`) – shader na teksturze numerów prowincji: prowincja pod kursorem lekko rozjaśniona, zaznaczona (kliknięcie) mocniej, z wyraźnym białym skrajem.
- **Malowanie warstw poza wątkiem głównym** (`render/paint.worker.ts`, klient `render/painter.ts`) – RGBA terenu i granic prowincji maluje osobny worker (dostaje kopię mapy raz na mapę), więc zmiana rodzaju mapy nie zamraża strony. Ostatnie 3 widoki są pamiętane (powrót jest natychmiastowy). Nowy widok przenika stary (350 ms, pierwsza mapa 600 ms), granice prowincji pojawiają się łagodnie (900 ms) – `CROSSFADE_MS`, `PROVINCES_FADE_MS` w `map-renderer.ts`.
- Mapa jest cięta na tekstury 512×512 (bezpieczny limit dla mobilnych GPU). Renderer działa na WebGL, bo shadery fal, rzek i jezior mają tylko wersję GLSL.

## Interfejs gracza

Dostępny dla każdego gracza (także w buildzie produkcyjnym), w `web/src/app/ui/`:

- **Górny pasek (HUD, `app.html`)** – trzy strefy: po lewej przycisk „Menu” (wyjście z gry z lobby, z potwierdzeniem – okno w panelu lobby), na środku **pasek zasobów** (`resource-bar`) i pod nim odliczanie wyboru startu, po prawej **kalendarz** (`calendar`). Pasek zasobów: flaga i nazwa państwa, populacja / limit z przyrostem na dzień gry („+380/d”, odświeżane raz na sekundę) i paskiem zapełnienia (kreska = 55%), wojsko i robotnicy z suwakiem podziału populacji (czerwone – wojsko, niebieskie – robotnicy, biała kreska – cel; ludzie przechodzą między grupami stopniowo, podpis „Wojsko 62% → 80%”), złoto (÷10) z kółkiem postępu do podatku – przy wpływie zielony dymek, np. „+1,7k” wyłania się pod złotem, unosi i znika – i przycisk ze skrzynką (liczba rodzajów surowców), który otwiera okno zapasów. Podczas wyboru startu, zanim gracz ma prowincję, pasek pokazuje sam suwak podziału – cel czeka w rdzeniu (`Game::pending_share`), a przed końcem odliczania podział zmienia się od razu, bez bezwładności (`Economy::split_now`). Liczby skrócone (`ui/format.ts`: 850, 25,3k, 1,2m), pełne w podpowiedziach. Na węższym ekranie (≤ 900 px) wyjście i kalendarz są w pierwszym rzędzie, zasoby pod nimi bez nazwy państwa i podpisów; na telefonie (≤ 520 px) wszystko mniejsze, żeby pasek zmieścił się w szerokości. Panel debugu zaczyna się pod HUD.
- **Kalendarz** (`ui/calendar.ts`, prawy górny róg) – dzień, miesiąc i rok (od 1 stycznia roku 0, 00:00) oraz pora dnia 00:00 / 06:00 / 12:00 / 18:00; liczony z tur od początku gry właściwej (`playingSince` w zdarzeniu gry). **Miesiąc trwa 30 s** (`TICKS_PER_MONTH` = 300 tur = okres podatku), więc początek miesiąca to zbiór podatków; dni mają prawdziwą liczbę (28–31), dzień trwa ok. 1 s, rok 6 minut. Na razie tylko na ekranie – gdy kalendarz zacznie wpływać na grę, trzeba go przenieść do rdzenia. Przed grą właściwą – data startowa, przygaszona.
- **Rodzaje mapy** (`top-bar`, dok w lewym górnym rogu, pod przyciskiem „Menu” – `app.html` `.dock-top`) – ikony w pionie: rodzaje mapy z klawiszami 1–4 (Teren – góry, Polityczna – mapa, Biomy – liść, Zasoby – kamień; nazwa i klawisz w podpowiedzi), dopasowanie widoku (F) i zębatka. Pod zębatką rozwija się w prawo lista opcji wyświetlania: granice prowincji z suwakiem krycia (P); grupa „Państwa”: krycie granic państw (osobno od prowincji), tło terytoriów na mapie terenu – kolor państwa albo jego flaga, każde z własnym suwakiem krycia, nazwy państw (włącz/wyłącz) z suwakiem krycia; izobaty (I), animacja wody (W).
- **Okno zapasów surowców** (`ui/stock-window.*`, stan otwarte / zminimalizowane / zamknięte w `MapStore.stockWindow`) – grupy jak legenda mapy zasobów, rozwijane kliknięciem nagłówka; ilość w puli i wydobycie na dzień, wydobycie złota w pasku tytułu; „Ukryj nieposiadane” (domyślnie włączone). Okno przeciąga się za pasek tytułu (mysz i dotyk, w granicach ekranu), ma przyciski minimalizacji i zamknięcia; pozycja, zwinięte grupy i ukrywanie są zapamiętane w `localStorage` (`mapa.stockWindow`). Domyślne wartości: `DEFAULT_TERRITORY_STYLE` w `render/map-renderer.ts`.
- **Napis ładowania** (`loading`, środek ekranu, z kręcącym się kółkiem): „Łączenie z serwerem…” (mapa powstaje z konfiguracji serwera, więc do `Welcome` nic się nie generuje), „Generowanie mapy…”, „Rysowanie mapy…”, „Wyznaczanie prowincji…”, „Ostatnie szlify…”, „Chowanie easter eggów…” (gdy są enklawy – w każdej renderer rysuje pikselowego yeti, `render/creatures.ts`). Nie blokuje myszy – mapę można oglądać, gdy dochodzą kolejne warstwy.
- **Odliczanie wyboru startu** (`start-timer`, na środku u góry, pod paskiem zasobów) – w czasie wyboru prowincji startowej: pasek znika od obu brzegów do środka, na środku czas (00:20, ostatnie 5 s jaśniej), pod spodem podpowiedź („Kliknij wolną prowincję…” albo „Twój start: prowincja N – kliknij inną wolną, aby zmienić”). Kliknięcie wolnej prowincji (bez innego gracza i bez tunelu) wysyła `ChooseStart`. Przed wyborem, gdy inni jeszcze wczytują mapę – „Czekanie, aż wszyscy gracze wczytają mapę…”.
- **Komunikat o grze** (`game-status`, pod górnym paskiem) – tylko gdy jest problem: stan gry rozjechał się z innymi graczami (desync), gra zatrzymana błędem (np. inna wersja generatora niż na serwerze) albo utracone połączenie z serwerem. Bez serwera od początku (strojenie generatora) nic nie pokazuje.
- **Ramka prowincji** (`province-info`, lewy dolny róg, pod dokiem rodzajów mapy) – prowincja pod kursorem, a gdy kursor jest poza lądem – zaznaczona: miniaturka flagi głównego właściciela przy numerze, numer, właściciel w grze (kolor państwa i gracz; przy prowincji współdzielonej – wszyscy z udziałem w kaflach; „niczyja”), wielkość (liczba kafli) z paskiem odchyłu od średniej wielkości prowincji na mapie (pionowa linia = średnia, skala ±50%, kolor: do ±10% zielony, do ±25% żółty, dalej czerwony – pod przyszłe balansowanie prowincji startowych), udział nizin i wyżyn (brązowy przy przewadze wyżyn), zalesienie (zielone od 50%), biom prowincji (typ – rodzaj), zasób (z kolorem z mapy zasobów) i czy graniczy z morzem, rzeką, jeziorem i górami (tak/nie z liczbą kafli prowincji stykających się z danym terenem). Ramka działa tylko przed grą właściwą (podgląd, poczekalnia, odliczanie wyboru startu) i pokazuje też bazowy poziom infrastruktury (z raportu rdzenia). W grze zastępuje ją **okno prowincji** (`ui/buildings-window.ts`) – otwiera się po zaznaczeniu dowolnej prowincji: na górze najważniejsze dane (`ui/province-summary.ts`: surowiec z wydobyciem na dzień i ceną – do rynku cena bazowa `goods::BASE_PRICE`, właściciel, wkład w limit populacji właściciela – jego kafle × 100 + miasta, infrastruktura obecna i bazowa, wielkość i teren), niżej budynki (`province-buildings` – w cudzej i niczyjej prowincji tylko poziomy; we własnej przy budynkach z obsadą – robotnicy w budynku / potrzebni, pokrycie wspólne dla państwa). Górny pasek pokazuje w części robotników, ilu z nich obsadza budynki (turkusowy odcinek); liczby gospodarki odświeżają się co 5 tur (`ECONOMY_EVERY`, 2 razy na sekundę). Okno prowincji pokazuje udziały państw w kaflach zamiast właściciela. Wybór startu: podpis „Twój start” i kolor prowincji (także przy pierwszym wyborze, gdy nikt nie ma jeszcze prowincji) pojawiają się od razu po kliknięciu (`GameSession.pendingStart`, `TerritoryLayer.preview`), zanim tura z wyborem wróci z serwera; w czasie wyboru pełne `owners` idą co turę. Nad górami ramka informuje, że są nieprzechodnie i niczyje. Kliknięcie prowincji zaznacza ją, ponowne kliknięcie albo kliknięcie wody – odznacza.
- **Legenda zasobów** (`resource-legend`, prawy dolny róg, tylko na mapie zasobów) – kolory 39 zasobów grupami (rolnictwo, hodowla i połowy, las, energia, metale, chemia) i liczba prowincji z każdym zasobem na bieżącej mapie; na wąskim ekranie zwinięta. Jest też filtrem: kliknięcie zasobu włącza/wyłącza go na mapie (wyłączone prowincje są szare), kliknięcie kategorii zostawia tylko ją (ponowne – przywraca wszystkie), przyciski „Wszystkie” (W) / „Żadne” (Z) przy nagłówku. Przyciski zaopatrzenia (lekkie, standardowe i ciężkie wojskowe, paliwo, prowiant, podstawowe i luksusowe cywilne, materiały inżynieryjne, pozostałe – `SUPPLY_CATEGORIES` w `render/resources.ts`, półprodukty według `docs/gdd/zaopatrzenie.md`) zostawiają surowce potrzebne do danego zaopatrzenia; ponowne kliknięcie przywraca wszystkie. Skład kategorii zależy od uniwersytetów (`categoryResources`: np. węgiel w paliwie od progu 10 – na razie 0, etap 7). Na mapie zasobów prowincje nie mają obwódek, tylko cienką ciemną granicę; cenniejsze rodzaje mają żywsze kolory, podstawowe przygaszone. Energia i metale mają znak na środku prowincji (`render/resource-markers.ts`): piorun – węgiel, kropla – ropa (czarna), znak radioaktywności – uran, symbol pierwiastka – metale i złoto (ścieżki ikon `ICONS` są te same w legendzie i na mapie).

## Panel debugu i klawisze

Panel (tylko build dev) pozwala stroić wszystkie parametry generatora. Suwaki przegenerowują mapę po puszczeniu, gdy zaznaczone jest „Generuj po każdej zmianie”. Sekcja „Wynik” pokazuje czas generowania, statystyki terenu, udział 13 rodzajów biomów, liczbę kontynentów z dwoma typami, udział lasu, statystyki prowincji (liczba, kafle: średnia ± odchylenie, min/max), hashe (terenu, biomów, roślinności, prowincji, zasobów) i wersję generatora. Sekcja „Serwer i gra” pokazuje połączenie, numer gracza, mapę gry (seed, rozmiar), czy na ekranie jest mapa gry, czy lokalny podgląd, liczbę tur rozesłanych przez serwer i wykonanych przez grę, bieżący i ostatnio odesłany hash stanu, desync, etap gry (czekanie / wybór startu z tickiem końca / gra) i państwa (gracze kierujący państwami). Mapa wygenerowana z panelu (G, N, suwaki) w trakcie gry to **lokalny podgląd** – gra (tury, hashe) toczy się dalej na mapie z serwera. Sekcja „Widok” zawiera siatkę chunków i suwaki animacji wody (fale przy brzegu, rzeki i jeziora, prędkość); pozostałe przełączniki widoku są w górnym pasku.

Klawisze G, N, C i Esc obsługuje panel debugu (Esc otwiera i zamyka ustawienia generatora – domyślnie schowane, w prawym górnym rogu jest wtedy przycisk „Narzędzia generatora Esc”), pozostałe – górny pasek (działają też w produkcji).

| Klawisz | Akcja |
|---|---|
| 1 / 2 / 3 / 4 | mapa: teren / polityczna (same prowincje) / biomy / zasoby (prowincje w kolorach zasobów, z legendą) |
| F | dopasuj widok do mapy |
| P | granice prowincji |
| I | izobaty |
| W | animacja wody (fale brzegowe, rzeki, jeziora); na mapie zasobów – wszystkie zasoby |
| Z | na mapie zasobów – żaden zasób |
| Esc | otwórz / zamknij ustawienia generatora (tylko build dev) |
| G | generuj z bieżącymi ustawieniami |
| N | nowy losowy seed i generuj |
| C | siatka chunków (chunki wodne lekko podświetlone; domyślnie wyłączona) |

Przeciąganie przesuwa mapę, kółko przybliża względem kursora, kliknięcie (bez przeciągania) zaznacza prowincję. Klawisze nie działają, gdy kursor jest w polu seeda.

## CLI `mapgen`

Podgląd generatora bez przeglądarki:

```bash
cargo run -p game-mapgen --release --features cli -- --seed 1 --out map.png
cargo run -p game-mapgen --release --features cli -- --params p.json --view biomes --no-contours
cargo run -p game-mapgen --release --features cli -- --seed 1 --view political --out political.png
cargo run -p game-mapgen --release --features cli -- --seed 1 --borders --out borders.png
```

- `--params p.json` – JSON z polami jak `MapGenParams` (camelCase), np. `{"continents": 1, "landRatio": 0.8}`; brakujące pola mają wartości domyślne,
- `--view biomes` – płaska mapa biomów, `--view political` – mapa polityczna,
- `--borders` – granice prowincji na mapie, `--border-opacity 0.55` – ich krycie,
- `--no-contours` – bez izobat (lodowiec: `{"glacier": true}` w `--params`).

CLI wypisuje statystyki oraz hashe terenu, biomów, roślinności i prowincji. Paleta jest ta sama co w przeglądarce (bez animacji fal).

## Serwer i protokół

`crates/server` to jedna binarka: WebSocket pod `/ws`, `GET /health` (JSON: status, liczba pokoi) i – lokalnie – statyczny frontend z `web/dist/web/browser` (w chmurze frontend jest w S3 za CloudFront, a katalogu nie ma).

```bash
cargo run -p game-server [--features debug] -- [--dev] [--port 3000] [--seed 7] [--params p.json]
                                               [--static-dir DIR] [--ticket-key ticket.pub.pem]
```

- `--seed` – seed mapy gry, `--params` – JSON z polami `MapGenParams` jak w CLI `mapgen` (brakujące pola domyślne; `--seed` nadpisuje seed z pliku). Bez nich mapa ma parametry domyślne.
- Zmienne środowiskowe (tak konfiguruje go systemd na EC2): `PORT`, `STATIC_DIR`, `TICKET_PUBLIC_KEY` (PEM) albo `TICKET_PUBLIC_KEY_FILE`, `ORIGIN_VERIFY_SECRET` (`/ws` wymaga nagłówka `X-Origin-Verify` – zna go tylko CloudFront), `ROOM_IDLE_SECS` (domyślnie 300), `LOG_FORMAT=json`, `RUST_LOG`, `ROOMS_TABLE` (heartbeat do lobby, tylko build z cechą `aws`).

**Pokoje i bilety** (docs/adr/0004). Serwer prowadzi wiele pokoi naraz; rejestr pokoi (`rooms.rs`) to aktor, który tworzy pokój przy pierwszym połączeniu i zapomina go, gdy pokój zamknie się sam (pusty dłużej niż `ROOM_IDLE_SECS`). Dwa tryby:

- **otwarty** (bez klucza biletów – lokalnie, `npm start`, narzędzia `tools/`): jeden pokój `default` z mapą z `--seed`/`--params`, jak dotąd,
- **bilety** (`--ticket-key` / `TICKET_PUBLIC_KEY`): `/ws?ticket=<JWT>`. Bilet (crate `crates/ticket`, Ed25519, ważny 60 s, jednorazowy) wystawia lobby; niesie ID pokoju, nazwę gracza i `GameConfig`, z którym serwer tworzy pokój. Odmowy: 401 (brak/zły/przeterminowany bilet), 403 (brak nagłówka originu), 409 (bilet użyty drugi raz, inna konfiguracja niż istniejący pokój). Bilet do testów ręcznych: `cargo run -p game-ticket -- --key ticket.pem --room r1 --name Ala [--seed 7]` (klucze: `cargo run -p game-ticket -- keygen --out .` → `ticket.pem` + `ticket.pub.pem`; OpenSSL daje te same formaty).

Serwer nie symuluje gry – zbiera intencje, stempluje je ID gracza, co 100 ms rozsyła numerowaną turę (`Turn`) i porównuje hashe stanu od klientów (`Desync`, gdy się różnią od hasha zgłoszonego dla tego ticka jako pierwszy; pamięta ostatnie 600 ticków). Tury lecą od dołączenia pierwszego gracza (pokój `default`) albo od startu gry przez gospodarza (pokój z biletu); gdy pokój `default` się opróżni, gra zaczyna się od nowa (tick 0). Pokój gry to aktor z wyłącznym dostępem do swojego stanu; połączenia rozmawiają z nim kanałami, bez `Mutex`.

**Tożsamość gracza.** ID gracza należy do **karty**, nie do połączenia: z biletem – ID karty z biletu, w trybie otwartym – `/ws?client=<ID karty>&name=<nick>` (`game/tab.ts`, `sessionStorage`). Po zerwanym połączeniu (Wi-Fi, uśpiony laptop, chwilowy brak sieci) gracz wraca z tym samym ID, więc dalej kieruje swoim państwem. Drugie połączenie tej samej karty zastępuje pierwsze (stare dostaje `Refused`); każde połączenie ma numer (`ConnId`), więc spóźnione wyjście starego nie wyrzuca nowego.

**Wybór prowincji startowej** (bez zegarów na serwerze). Klient po zbudowaniu gry (mapa gotowa, przebieg nadrobiony) wysyła `ClientMsg::Ready`. Gdy po starcie gotowi są wszyscy obecni (najdłużej `READY_TIMEOUT_TICKS` = 60 s), serwer wstawia na początek tury `Intent::BeginSelection { players, random }` ze stemplem `SERVER` (klient takiej intencji wysłać nie może). Dalej czas liczy rdzeń w turach: wybór trwa `SELECTION_TICKS` = 200 tur (20 s), więc koniec i losowanie startów dla tych, którzy nie wybrali, wypada u wszystkich w tym samym ticku. Ekran pokazuje odliczanie z numeru tury, a między turami płynnie dopowiada je zegar przeglądarki. Uczestnicy: z biletem – wszyscy z poczekalni (także chwilowo rozłączeni), w trybie otwartym – obecni (kto dołączy w czasie wyboru, też może wybrać). Przy starcie losowym (`RoomSettings::random_start`, „Start losowy” w oknie lobby) starty są losowane od razu.

Wiadomości (`crates/core/src/protocol.rs`, typy TS generowane):

- klient → serwer: `Join`, `Start` i `Settings` (tylko gospodarz, w poczekalni; `Settings` niesie `RoomSettings`: nazwa, `MapSettings`, seed, limit graczy, start losowy), `Intent`, `Ready` (gra u klienta gotowa), `Hash` (hash stanu po wykonaniu tury `tick`),
- serwer → klient: `Welcome` (ID gracza, `GameConfig` z parametrami mapy i `Catchup` – przebieg gry do nadrobienia), `Lobby` (skład pokoju, gospodarz, czy gra ruszyła, bieżące `RoomSettings` – po każdej zmianie; nowa mapa w ustawieniach = nowy `Welcome` dla wszystkich), `Turn`, `Desync`, `Refused` (pokój pełny / gra trwa bez tego gracza / ta sama karta połączyła się ponownie – potem serwer zamyka połączenie), `Closed` (gospodarz zamknął poczekalnię),
- `Catchup { tick, turns }` – rozegrano `tick` tur, a w `turns` są tylko te z intencjami (reszta była pusta), więc wiadomość jest krótka nawet po długiej grze,
- intencje debugowe (`RegenerateMap`, `SetPaused`) istnieją tylko z cechą `debug` i tylko gdy serwer działa z `--dev`.

### Lobby (meta-serwer)

**Przepływ gracza:** ekran powitalny (nick, liczba osób na stronie, lista lobby w poczekalni z „gracze / limit”, „+ Utwórz lobby” – okno modalne) → **poczekalnia** pokoju (skład na żywo przez WebSocket, gospodarz = gracz obecny najdłużej; mapa generuje się już w tle) → gospodarz klika „Start gry” → ekran lobby płynnie znika (animacja `screen-leave` w `ui/lobby-panel.css`), lecą tury; z gry nie ma powrotu do poczekalni – tylko „Menu” w górnym pasku (z potwierdzeniem). Gdy grę opuszczą wszyscy gracze, pokój się kończy. Nick domyślnie `guest` + 4 cyfry, nazwa nowego lobby „<nick> lobby”. Gospodarz zmienia ustawienia w poczekalni zębatką obok nazwy pokoju (to samo okno co przy zakładaniu): game-server jest źródłem prawdy o ustawieniach od utworzenia pokoju (pokój zostaje przy swojej konfiguracji także dla biletów sprzed zmiany), a do lobby trafiają heartbeatem (nazwa, `config`, `maxPlayers`). Wyjście gospodarza z poczekalni („Zamknij lobby”) kończy lobby: serwer wysyła wszystkim `Closed` i goście wracają do listy z komunikatem; w trakcie gry gospodarzem zostaje następny gracz. Przy zakładaniu lobby – uproszczone ustawienia mapy (`MapSettings` w `core/src/lobby.rs`: rozmiar – pole mapy jak kwadrat 1000/1400/1800, proporcje 1:1 / 3:2 / 2:1 przy tym samym polu i z siatką chunków dobraną tak, żeby chunki zostały kwadratowe, 1–6 kontynentów, ilość lądu, klimat), do tego **start losowy** (pole wyboru pod limitem graczy: zamiast 20 s na wybór gracze dostają starty od razu – `RoomSettings::random_start`, `CreateRoom::random_start`, w bilecie i w tabeli lobby); `MapSettings::params(seed)` nadpisuje tylko te pola domyślnych `MapGenParams`. Na ekranie powitalnym nic się nie generuje. Po starcie pokój znika z listy; do gry wraca tylko karta, która była w poczekalni (ID karty w bilecie), np. po zerwanym połączeniu (o ile w grze ktoś jeszcze jest). Pokój `default` w trybie otwartym startuje od razu (bez poczekalni) – lokalnie i w narzędziach nic się nie zmienia.

`crates/meta` – Lambda w Ruście (`provided.al2023`, arm64, budowana `cargo zigbuild` jako binarka `bootstrap`) za API Gateway HTTP API, pod `/api/*` tej samej domeny (docs/adr/0002). Typy żądań i odpowiedzi są w `crates/core/src/lobby.rs` (TS generowany).

| Żądanie | Odpowiedź |
|---|---|
| `GET /api/rooms` | pokoje w poczekalni (`RoomSummary[]`: nazwa, seed, gracze, limit) – najnowsze pierwsze |
| `POST /api/rooms` (`CreateRoom`: nazwa, opcjonalnie seed, limit 1–16 i `MapSettings`) | nowy pokój (`RoomSummary`, z rozmiarem mapy i liczbą kontynentów) |
| `POST /api/rooms/{id}/join` (`JoinRoom`: nazwa gracza, ID karty) | `JoinResponse`: `wsPath` = `/ws?ticket=<JWT>` (60 s, jednorazowy; niesie ID karty i limit graczy) |
| `POST /api/presence` (`PresenceUpdate`: ID karty) | `Presence`: liczba kart widzianych w ostatnich 45 s (karta zgłasza się co 15 s) |

- Pokoje są w DynamoDB (jedna tabela, indeks `byStatus`, TTL). Liczbę graczy i status (`open` – poczekalnia, `playing` – gra, `closed`) dopisuje heartbeat game-servera co 5 s (`server/src/heartbeat.rs`, cecha `aws`); lista pokazuje tylko `open`. Obecność to wpisy `USER#<karta>` (status `online`) w tym samym indeksie. Starsza niż minuta liczba graczy jest pokazywana jako 0.
- Logika API (`meta/src/app.rs`) nie zależy od Lambdy – testy idą na magazynie w pamięci (`RoomStore`).
- **Ochrona przed spamem/DDoS** (docs/adr/0007): API działa tylko przez CloudFront (nagłówek `X-Origin-Verify`), limity na adres IP (zakładanie lobby 5/min, bilety 30/min → `429`), throttling API Gateway; game-server – limity połączeń na IP i łącznie, wiadomości na połączenie, liczby pokoi (`server/src/limits.rs`).
- **Frontend** (`game/lobby.ts`, `ui/lobby-panel.ts` + `.html`): przy starcie pyta `/api/rooms`. Odpowiedź JSON = lobby (ekrany `list` → `room` → `game`; lista odświeżana co 5 s tylko na ekranie listy; nick w `localStorage`, ID karty w `sessionStorage`), inaczej (np. `ng serve`, serwer lokalny) – tryb otwarty jak dotąd. `Transport` pobiera ścieżkę połączenia z funkcji: przy każdej próbie (także po restarcie serwera) bierze nowy bilet.
- **Test end-to-end bez AWS:** `node tools/lobby/e2e.mjs` – game-server w trybie biletów + Chromium, a odpowiedzi `/api/*` podstawia test przez przechwytywanie żądań w przeglądarce (CDP `Fetch`); bilety i klucze z CLI `ticket` (wymaga `cargo build -p game-server -p game-ticket` i `npm run build`; binarki z innego katalogu – zmienna `CARGO_TARGET_DIR`).

### Pętla tur w przeglądarce

1. `GameSession` (`game/game-session.ts`) łączy się z serwerem. Mapy nie generuje, dopóki nie przyjdzie `Welcome` – dopiero z `config.map`. Bez serwera (połączenie odrzucone albo brak `Welcome` przez 3 s – `OFFLINE_FALLBACK_MS`) mapa powstaje z parametrów domyślnych.
2. Po `Welcome` sesja wysyła do workera `startGame` (konfiguracja + `Catchup`), a `MapStore.show(config.map)` zleca mapę na ekran – **tylko jeśli ostatnio zlecona mapa (gotowa albo w trakcie generowania) ma inne parametry**. Mapa jest więc generowana raz – także gdy powstała wcześniej bez serwera z tymi samymi parametrami.
3. Worker trzyma ostatnią wygenerowaną mapę w pamięci wasm. Gdy ma ona parametry z `config.map` i policzone prowincje, buduje z niej grę: `WasmGame.fromMap(config, map)` przejmuje mapę (bez generowania drugi raz; sprawdza wersję generatora i parametry), nadrabia `Catchup` i wykonuje tury, które w tym czasie czekały w kolejce.
4. Każdą turę z serwera sesja przekazuje workerowi; worker wykonuje ją (`applyTurn`) i co 10 tur (`HASH_EVERY` w workerze) zwraca `stateHash`, który sesja odsyła serwerowi jako `ClientMsg::Hash`. Po pierwszym zdarzeniu gry po `Welcome` sesja wysyła `ClientMsg::Ready`; zdarzenie niesie też etap gry (`phase`) i koniec wyboru startu (`selectionEnd`).
5. Ponowne połączenie z tą samą konfiguracją (np. restart serwera) zaczyna grę od nowa na tej samej mapie (`WasmGame.restart` + `Catchup`) – bez generowania i bez zmiany mapy na ekranie. Inna konfiguracja = nowa mapa i nowa gra.

Symulacja działa w workerze, więc nie zależy od odświeżania strony – karta w tle nadal wykonuje tury i odsyła hashe.

**Test na kilku kartach** (`tools/lockstep/two-tabs.mjs`, surowe CDP bez zależności – wspólny kod uruchamiania Chromium i połączenia CDP jest w `tools/cdp.mjs`, używa go też `tools/loadtest/sim.mjs`): karta A wchodzi od razu, karta B po kilku sekundach (nadrabia tury), a skrypt podsłuchuje ramki WebSocket i porównuje hashe stanu obu kart, sprawdza brak desynców, to, że gra nadąża za serwerem, i że każda karta wygenerowała mapę dokładnie raz. `--tamper` dodaje kartę, która psuje odsyłane hashe – serwer musi jej zgłosić desync.

```bash
cd web && npm run build && cd ..
cargo run -p game-server &            # albo z --seed / --params
node tools/lockstep/two-tabs.mjs http://127.0.0.1:3000/ --seconds 40 --delay 8 --tamper --shots /tmp/lockstep
```

Na Windows wskaż przeglądarkę zmienną `CHROME` (np. `C:\Program Files\Google\Chrome\Application\chrome.exe`); profil Chromium trafia do katalogu tymczasowego systemu. Ten sam test działa na wdrożonej grze: `node tools/lockstep/two-tabs.mjs https://<adres>/ --tamper`.

## Wdrożenie na AWS

Szczegóły, komendy i koszty: [infra/README.md](infra/README.md); decyzje: [docs/adr/](docs/adr/).

```
https://<id>.cloudfront.net (opcjonalnie własna domena)
   │ CloudFront
   ├── /*     → S3 (frontend; pliki z hashem – cache na rok, index.html i wasm – rewalidacja)
   ├── /api/* → API Gateway HTTP API → Lambda `meta` (Rust) → DynamoDB (pokoje)
   └── /ws*   → EC2 t4g.micro: game-server (port tylko dla CloudFront + nagłówek X-Origin-Verify)
```

- **Infrastruktura jako kod:** Terraform w `infra/` (stan w S3, budżety 1 i 10 USD z alarmem e-mail), moduły `dns` (opcjonalny), `frontend`, `game_server` (stały adres – Elastic IP, więc stop/start instancji nie wymaga `terraform apply`), `meta` (lobby), `monitoring` (alarmy CloudWatch z powiadomieniem e-mail – `alert_email` w `terraform.tfvars`).
- **Wdrażanie z IDE:** `.vscode/tasks.json` (Terminal → Run Task…: plan/apply z buildem Lambdy, deploy frontend, deploy game-server, logi na żywo) albo skrypty `tools/deploy/*.mjs` (`all.mjs` – wszystko po kolei, `infra.mjs [--plan]` – build Lambdy + Terraform, `smoke.mjs` – sprawdzenie produkcji, tylko odczyt). Lambda i game-server są kroskompilowane z Windows na Linux arm64 (`cargo zigbuild`, musl); game-server podmieniany przez SSM Run Command – bez SSH.

## Determinizm i hashe

Rdzeń i generator muszą dawać ten sam wynik na każdej platformie:

- bez `HashMap`/`HashSet` w logice (losowa kolejność iteracji) – `Vec`/`BTreeMap`,
- bez zegara i bez losowości spoza seeda (własny RNG SplitMix64),
- w generatorze tylko operacje zmiennoprzecinkowe jednoznaczne w IEEE 754 (bez `sin`/`cos`/`exp` z biblioteki systemowej); funkcje przestępne w rdzeniu tylko z crate'a `libm`.

Szybki test „natywnie vs wasm”: dla seeda 1 z domyślnymi parametrami CLI i panel w przeglądarce muszą pokazać te same wartości:

| Hash | Seed 1, domyślne parametry |
|---|---|
| terenu (FNV-1a z `terrain`) | `32922838` |
| biomów (FNV-1a z `biome`, `biomeLayers`, `biomeMix`, `seaIce`) | `b63de25b` |
| roślinności (FNV-1a z `forest`) | `63dc5761` |
| prowincji (FNV-1a z bajtów `province`, u16 little endian) | `a23ddba7` |
| zasobów (FNV-1a z `resource` prowincji w kolejności numerów) | `68b9b8b5` |

Hashe zmieniają się przy każdej zmianie wartości domyślnych albo algorytmu – wtedy zaktualizuj tę tabelę.

**Hash stanu gry** (`Game::state_hash`, do wykrywania desynców między graczami) to FNV-1a po mapie (teren, biom dominujący, kafle leśne, prowincje, zasoby prowincji – liczony raz w `Game::from_map`) i dalej po stanie gry: tick i właściciele (przyrostowy skrót kafli z państwami + gracze kierujący państwami – `Owners::hash_bytes`), a w przyszłości każde nowe pole stanu. Dzięki temu hash co turę nie przechodzi przez całą mapę.

`GENERATOR_VERSION` (obecnie 25) podbijaj przy każdej zmianie algorytmu – seed i wersja idą do konfiguracji gry i replayów.

## Kontrakty utrzymywane ręcznie

Większość zgodności pilnuje kompilator dzięki `ts-rs`. Kilka rzeczy trzeba utrzymywać ręcznie:

| Co | Gdzie | Uwaga |
|---|---|---|
| Wiadomości, intencje, `Catchup`, `MapGenParams`, `MapStats`, `Province` | `core/protocol.rs`, `mapgen/lib.rs` | TS generowany automatycznie (`npm run types`) |
| Wartości `Terrain`, `BiomeType` i `Biome` | `mapgen/lib.rs` ↔ `render/terrain.ts` | ręcznie |
| Wagi rodzajów z warstw (`kind_weights` ↔ `kindWeights`) | `mapgen/biome.rs` ↔ `render/terrain.ts` | ręcznie, ten sam wzór |
| Rodzaje typów i kolejność wariantów (`BiomeType::kinds`, `variant_index` ↔ `TYPE_KINDS`, `VARIANTS`) | `mapgen/lib.rs` ↔ `render/terrain.ts` | ręcznie |
| Lód: maska, cieniowanie, wyraźna krawędź (`ice_mask`/`ice_shade`/`sharpen_ice` ↔ `iceMask`/`iceShade`/`sharpenIce`) | CLI `mapgen` ↔ `render/terrain.ts` | tylko wygląd |
| Kolejność `BIOME_PAIRS` (bity `biomePairs`) | `mapgen/lib.rs` ↔ `render/terrain.ts` | ręcznie |
| Palety terenu, oceanu, koron drzew, poziomy izobat | CLI `mapgen` ↔ `render/terrain.ts` | tylko wygląd |
| Hash kafla do ziarna lasu (`tile_hash` / `tileHash`) | CLI `mapgen` ↔ `render/terrain.ts` | tylko wygląd |
| Granica prowincji (kolor, krycie), kolory mapy politycznej (prowincja niczyja, granice) | CLI `mapgen` ↔ `render/provinces.ts`, `game/map-store.ts` | tylko wygląd; kolory państw tylko w przeglądarce (`render/owners.ts`) |
| Kolejność zasobów (`RESOURCE_NAMES` ↔ `RESOURCES` z kolorami) | `tools/resources/chances.mjs` → `mapgen/resource_table.rs` ↔ `render/resources.ts` | ręcznie; mapa zasobów tylko w przeglądarce |
| Granice chunków | `mapgen::chunk_start` ↔ `drawChunkGrid` | `ceil(c * size / count)` |
| Hashe terenu, biomów, roślinności, prowincji i zasobów (FNV-1a) | CLI `mapgen` ↔ `game.worker.ts` | do porównań native vs wasm |
| `GENERATOR_VERSION` | `mapgen/lib.rs` | podbij przy każdej zmianie algorytmu |

## Gdzie wejdą mechaniki

Docelowe zasady gospodarki (surowce, zaopatrzenie, budynki, centrum dowodzenia, rynek) i kolejność ich wdrażania opisuje **GDD: [docs/gdd/](docs/gdd/README.md)**.

### Stan wyjściowy (co już jest, a czego brakuje)

- **Rdzeń** (`crates/core/src/game.rs`): `Game` trzyma `GameConfig`, `MapData` (niezmienną), licznik ticków, graf sąsiedztwa prowincji (`Game::neighbours`), prostokąty prowincji i **właścicieli** (`Game::owners`). `Game::from_map` buduje grę z gotowej mapy (tak robi przeglądarka), `Game::new` generuje mapę sam (testy, narzędzia). `apply_turn` wykonuje intencje po kolei (`execute` – walidacja w rdzeniu). `catch_up` nadrabia `Catchup`, `restart` zaczyna od nowa na tej samej mapie. `state_hash` – patrz [Determinizm i hashe](#determinizm-i-hashe).
- **Właściciele** (`crates/core/src/owners.rs`, GDD: kafel ma jednego właściciela, prowincja może być współdzielona): właścicielem kafla jest **państwo** – numer 1..255 w bajcie na kafel (0 = niczyj), nadawany graczowi przy zdobyciu pierwszego kafla (`nations()[n - 1]` = gracz kierujący państwem, więc państwo da się kiedyś przekazać bez zmiany kafli). Do tego udziały w każdej prowincji (`shares`: państwo → liczba kafli, aktualizowane przy każdej zmianie kafla) i skrót do hasha liczony przyrostowo (suma skrótów `(kafel, państwo)` – zmiana kafla to O(1), bez przechodzenia po mapie). `revision` rośnie przy każdej zmianie – worker wysyła kafle na ekran tylko wtedy.
- **Graf sąsiedztwa** (`mapgen::province_graph`, `polish.rs`): sąsiedzi prowincji po lądzie, rosnąco po numerze, z flagą `river` (styk tylko w poprzek wąskiej rzeki – przyszły most albo bród); ta sama zasada co przy grupach w ostatnich szlifach. Przez morze, góry i lodowiec prowincje nie sąsiadują (`coastal`, `tunnel`).
- **Protokół** (`crates/core/src/protocol.rs`): `Intent`: `Ping`, `ChooseStart { province }` (wybór prowincji startowej w czasie wyboru – kliknięcie wolnej prowincji; ponowny wybór przenosi start; prowincja z innym graczem albo z tunelem – przepada), `BeginSelection { players, random }` (tylko od serwera, stempel `SERVER`) i `Debug` z cechą `debug`. `ClientMsg`: `Join`, `Start`, `Settings`, `Intent`, `Ready`, `Hash`; `ServerMsg`: `Welcome` (ID gracza + `GameConfig` + `Catchup`), `Lobby`, `Turn`, `Desync`, `Refused`, `Closed`.
- **Serwer** (`crates/server/src/room.rs`, `rooms.rs`): wiele pokoi (z biletów lobby albo jeden `default`), poczekalnia i start gry przez gospodarza (pokój `default` startuje sam), tury co 100 ms, log tur (replay, z niego `Catchup` dla dołączających), porównanie hashy, heartbeat do lobby.
- **Klient:** pętla lockstep działa end-to-end ([Pętla tur w przeglądarce](#pętla-tur-w-przeglądarce)) – mapa z `GameConfig` z `Welcome` generowana raz, `WasmGame` w workerze zbudowany z tej mapy, tury, hashe co 10 tur, nadrabianie dla spóźnionych i po ponownym połączeniu, test na kilku kartach (`tools/lockstep/`). Właściciele idą z workera w zdarzeniu `game` (`owners` – tylko po zmianie: kafle, państwa, udziały) do `GameSession.owners`; renderer maluje je na mapie politycznej (`render/owners.ts`), ramka prowincji pokazuje właściciela (przy współdzielonej – udziały).
- **Etapy gry** (`Phase` w `game.rs`): `Waiting` (czekanie na gotowość graczy) → `Selection { end }` (wybór startów do ticka `end`) → `Playing`. Na koniec wyboru (albo od razu przy starcie losowym) gracze bez startu dostają losową prowincję (`random_start`: wolna, bez tunelu, wielkość ±15% średniej – `START_AREA_PCT`, nie graniczy z cudzym startem, losowo spośród `START_SPREAD` najdalszych od innych startów; własny strumień losowości `START_SALT`). Starty (`starts`), uczestnicy i etap są w hashu stanu.
- **Gospodarka** (`core/src/economy.rs`, stan `Game::economy` na państwo, w hashu): populacja (wojsko + robotnicy, w tysięcznych osoby – liczby całkowite) i złoto – zasoby globalne gracza. Limit populacji: `PEOPLE_PER_TILE` (100) × kafle państwa (`Owners::area`); start: `START_POPULATION` (25 000) przy pierwszej prowincji startowej, złoto 0. W grze właściwej co turę przyrost `growth`: do `PEAK_PPM` (55%) limitu proporcjonalny do populacji (przyspiesza), dalej do pierwiastka z brakującej części (zwalnia, ale dochodzi do limitu w skończonym czasie – ok. 140 s od 55% do pełna, bez długiego „pełzania”; ponad limitem ujemny); w szczycie `PEAK_GROWTH_PPM` limitu na turę. Suwak (`Intent::SetTroopShare`, domyślnie 50%) ustawia docelowy udział wojska w **całej** populacji (`target_share`): nowa populacja od razu w tych proporcjach, a istniejąca przechodzi między wojskiem a robotnikami najwyżej `SHIFT_PPM` (500 = 0,05% populacji) na turę – ok. 0,5 punktu procentowego na sekundę, także gdy populacja stoi na limicie; rzeczywisty udział to `Economy::troop_share()`. Co `TAX_TICKS` tur (30 s = miesiąc gry, od początku gry) podatek `GOLD_PER_PERSON` od **robotnika** – wojsko podatków nie płaci. Interfejs dostaje `NationReport` (`Game::reports`, `WasmGame.reportsJson`) w każdym zdarzeniu `game` i tick najbliższego podatku (`nextTax`).
- **Surowce** (etap 1 z `docs/gdd/plan.md`; tabela `core/src/goods.rs`: budynek wydobycia i cena bazowa każdego surowca): każda prowincja daje **0,10 swojego surowca na dzień** (`BASE_YIELD_PER_DAY`, dzień = 10 tur) do globalnej puli właściciela prowincji – gracza z ponad 50% jej kafli (`Owners::province_owner`); zapas `Economy::stock` (w tysięcznych, w hashu stanu). Złoto z kopalni idzie od razu do skarbca (`GOLD_PER_UNIT` = 50 złota z interfejsu za jednostkę). W pasku zasobów przycisk ze skrzynką otwiera okno zapasów (`ui/stock-window.*` – grupy jak legenda mapy zasobów, ilość i wydobycie na dzień; na górze zaopatrzenie – zapas startowy `goods::START_SUPPLIES`: 50 materiałów inżynieryjnych, 30 paliwa, 40 podstawowego cywilnego, 60 prowiantu (start: 40 materiałów, 30 paliwa, 100 podstawowego cywilnego); prowiant – `Supply::Provisions` – z fabryk z 8 surowców spożywczych (w tym dopisane na końcu listy: kukurydza, drób, trzoda chlewna, ryby – łowisko `Extraction::Fishery`); wydobycie rolnicze, hodowlane i połowy tylko za złoto (`Extraction::gold_only`, `Game::build_cost`); zaspokojenie ludności – kolorowe % w oknie zapasów).
- **Budynki** (etap 2; `core/src/buildings.rs` – tabele i stałe do strojenia, zasady w `Game`): poziomy budynków w prowincji (`Game::buildings`, w hashu stanu; nigdy nie maleją) – wydobycie (nazwa z surowca: kopalnia, tartak…), fabryka, miasto, uniwersytet, umocnienia, port (tylko nad morzem), infrastruktura. Infrastruktura daje punkty na budynki (poziom 1 – 100, każdy kolejny +50), jej poziom startowy liczy `start_infrastructure` z cech prowincji (bazowo 2, −1 trudny biom, −1 wyżyny/góry, +1 rzeka/morze; 1–3). `Intent::Build` / `Intent::CancelBuild` waliduje `Game::check_build` (właściciel > 50% kafli, gra właściwa, jedna budowa na prowincję, punkty, złoto i materiały inżynieryjne – pobierane od razu; anulowanie zwraca 50%); czas 10 + 5 × poziom dni, budowa stoi, gdy prowincja nie ma właściciela. Rozliczenie tury (`Game::rates` → `NationRates`): limit populacji = kafle × 100 + 25 000 za poziom miasta; obsada budynków robotnikami (wydobycie 10 000, fabryka 20 000, uniwersytet 15 000, port 5 000 na poziom) – pokrycie mnoży wydobycie (+1,0/dzień za poziom); pola uprawne zużywają nawozy (0,2/dzień na poziom) i przy pełnym pokryciu dają +50%. Umocnienia czekają na centrum dowodzenia (etap 5), uniwersytety (poza premią fabryk i paliwem syntetycznym) i porty – na swoje etapy. UI: okno budynków (`ui/buildings-window.ts` + `ui/province-buildings.*`, przeciągane – wspólne `ui/floating-window.*`) otwiera się po zaznaczeniu własnej prowincji: punkty infrastruktury, budowa z postępem i anulowaniem, poziomy (kropki), przycisk budowy z kosztem (złoto, materiały) i powodem blokady w podpowiedzi; raport prowincji liczy rdzeń (`Game::province_report`), worker dołącza go do zdarzeń gry dla prowincji z ramki (`watch`). Obsada w podpowiedzi przy robotnikach (żółci – za mało robotników).
- **Fabryki i zaopatrzenie** (etap 3; `core/src/factories.rs` – receptury z `docs/gdd/zaopatrzenie.md` w jednej tabeli `RECIPES`): fabryka produkuje jeden rodzaj zaopatrzenia (`ProvinceBuildings::product`, domyślnie materiały inżynieryjne), 1,0 sztuki na dzień za poziom × obsada × premia uniwersytetów (+10% od 5, +20% od 20 poziomów). Składnik to lista wymiennych wejść (surowiec z puli albo zaopatrzenie – np. paliwo w podstawowym cywilnym i materiałach, zamiennik: konie); wybór zapamiętuje fabryka (`factories::Inputs` w `ProvinceBuildings`): automatycznie – wejście, z którego zrobi najwięcej, i trzyma się go do wyczerpania zapasu; ręcznie (`Intent::SetFactoryInput`, klik w zamiennik w recepturze; ponowny – z powrotem automatycznie) – do zmiany przez gracza; przy braku produkuje ułamek (`factories::produce`; kolejność fabryk – rosnąco po numerze prowincji). Paliwo syntetyczne z węgla od 10 poziomów uniwersytetów. `Intent::SetFactory` zmienia produkt (przezbrojenie 5 dni); rodzaje wojskowe odblokowuje centrum dowodzenia (do etapu 5 każde państwo ma poziom 1 – `Game::command_level`: lekkie tak, standardowe od 2, ciężkie od 4). Zaopatrzenie trafia do `Economy::supplies`; raport: produkcja zaopatrzenia i zużycie surowców na dzień (`supplyProduction`, `resourceUse`). UI: w oknie budynków wybór produktu fabryki (ikony rodzajów) i receptura jako „funkcja” (`ui/recipe.*`: składnik – ikony zamienników z ilością na sztukę, zapasem i zaznaczonym wejściem w użyciu z czasem do wyczerpania + … → produkt z zapasem i „na ile starczy”; `factories::ingredients` / `pick` w rdzeniu), produkcja „0,6 / 1,0 na dzień”, przezbrojenie; ikony dóbr – `ui/goods-icon.ts`. Okno zapasów ma zakładki: zapasy (bilans na dzień, ujemny na czerwono; zaopatrzenie z produkcją i zużyciem), receptury (drzewko zaopatrzenia z rdzenia – `factories::rules`, z poziomem centrum dowodzenia) i odblokowania.
- **Ekspansja i walka** (etap 6, bez aneksji; `core/src/war.rs` – stałe do strojenia, zasady w `Game`): `Intent::Attack { provinces, percent }` wysyła % wojska z kraju na **front** – jedną albo kilka prowincji naraz (limity według centrum dowodzenia: `war::MAX_FRONTS` frontów, `MAX_FRONT_PROVINCES` prowincji na froncie, `MAX_SEA_LANDINGS` desantów morskich; do trwającego frontu można dosłać wojsko i dołączyć prowincje). Styk: granica kafli, przeprawa w poprzek rzeki do 4 kafli, desant przez jezioro nad naszym brzegiem (od CD 1) albo przez morze (od CD 2) – zbiorniki wody liczy `Water` z mapy, kafel zajmowany z wody ×1,5 (`LANDING_PCT`); prowincje za tunelem – niedostępne (tunele w przyszłości). Co turę front (`Game::frontier` – kafle prowincji frontu nie nasze, sąsiadujące z naszymi albo z wodą) zajmuje kafle od najłatwiejszych (`war::front_score`): trudność terenu (`Game::terrain_pct` – niziny 100%, las 125%, wyżyny 150%, wyżyny z lasem 190%), nasze kafle wokół (kieszenie znikają od razu, front jest spójny) i „zawirowanie taktyczne” (losowy składnik z `war::front_key`); za punkty szybkości (30 na turę; kafel niczyj 10, cudzy 20, × teren; paliwo do ×1,5, konie zastępują do 50% paliwa). Koszt kafla (× teren): niczyj – walka z **armią tubylczą** (`Game::natives`, w hashu: 8 osób na kafel × infrastruktura startowa 80/100/130%; kafel 3 + 25% gęstości tubylców, tubylcy tracą gęstość), cudzy 20 + 2 × gęstość wojska obrońcy na kafel × (zaopatrzenie obrońcy / atakującego; `war::MILITARY`: bez zaopatrzenia 25%, lekkie 60 → 80% od CD 3, standardowe 100 → 120% od CD 4, ciężkie 140 → 160% od CD 6) – obrońca traci tę gęstość za kafel; zaopatrzenie wojskowe zużywa się tylko, gdy front styka się z graczem (1 sztuka na dzień na 1 000 wojska). Paliwo zależy od zaopatrzenia w walce (na 1 000 wojska na dzień: bez 0,05, lekkie 0,1, standardowe 0,2, ciężkie 0,4; od progu CD 40% mniej). Wojsko w ataku to `Economy::deployed` (w populacji i udziale wojska, ale nie broni); atak kończy się po zajęciu prowincji, utracie styku albo braku wojska na kolejny kafel – resztka wraca (`Intent::CancelAttack` – odwołanie). Ataki są w hashu stanu. UI: Ctrl + klik zaznacza kilka prowincji na jeden front (`MapStore.attackTargets`, czerwonawe podświetlenie – tekstura wyboru w `render/highlight.ts`; worker sprawdza zbiór – `checkAttackJson`), dwuklik na cudzą prowincję atakuje od razu z udziałem wojska z suwaka (`MapStore.attackShare`); panel ataku na dole na środku (`ui/attack-panel.*` – po zaznaczeniu cudzej prowincji albo przypięty: suwak wojska, kafle do zajęcia, szacunek kosztu, „Zajmij” / „Atakuj” / „Dośli wojsko”, postęp i odwołanie, powód blokady), menu kołowe pod prawym przyciskiem (`ui/radial-menu.ts` – budynki dla własnej, atak dla cudzej, wyśrodkowanie), panel frontów w prawym dolnym rogu (`ui/fronts-panel.*` – `Game::fronts`: strony z flagą, wojskiem i zaopatrzeniem w użyciu, % okupacji, „Idź do” – płynny przelot kamery `MapRenderer::flyTo`), w pasku zasobów „+5,4k w ataku”. Terytoria na ekranie co turę: rdzeń zapisuje zmiany kafli (`Owners::take_changes`), worker wysyła je w każdym zdarzeniu (`changes`), a `render/territory.ts` (`TerritoryLayer`) trzyma żywych właścicieli i przelicza na CPU tylko otoczenie zmienionych kafli (krawędzie polityczne, siła granicy państwa – odległość do innego państwa w promieniu grubości granicy); GPU (shader na kawałkach 512 × 512, wysyłane tylko zmienione) rysuje kolor z palety: mapa polityczna – pełne kolory z krawędziami, mapa terenu – tło i granica; nowy kafel rozbłyskuje i przechodzi w kolor państwa (300 ms). Flagi i nazwy maluje dalej worker z pełnych `owners`, najwyżej raz na sekundę (`OWNERS_EVERY`). Po końcu odliczania kamera płynnie przelatuje na prowincję startową.
- **Budynki na mapie:** stałe miejsca w każdej prowincji to cecha mapy – `Province::slots` (generator, `mapgen/src/slots.rs`, faza `polish` po zasobach; razem z nimi startowa infrastruktura `Province::infrastructure`): miasto blisko środka w głębi prowincji (tu też uniwersytet – czapka obok ikony miasta – i centrum dowodzenia), port przy morzu najbliżej miasta, wydobycie na pasującym terenie (`slots::FIT`, zgodne z `goods::EXTRACTION` – pilnuje test rdzenia), fabryka jak najdalej od pozostałych, ale w głębi prowincji (co najmniej połowa największej głębokości – `INNER_DEPTH_PCT`; infrastruktura i umocnienia nie mają miejsca ani ikony – działają na całą prowincję). **Budynek kontroluje państwo, które ma jego kafel** (`Game::controller`; centrum dowodzenia – kafel miasta), niezależnie od udziału w prowincji: buduje, anuluje, ustawia fabrykę, dostaje produkcję, limit z miasta i płaci obsadę; wydobycie bazowe (bez budynku) też idzie do właściciela kafla wydobycia. Infrastruktura i umocnienia (bez miejsca) – właściciel prowincji (> 50% kafli). Trwające budowy idą z workera po zmianie postępu (`constructions`) – na mapie ikona budynku z kołowym paskiem postępu (budynki bez miejsca – obok miasta). Worker wysyła miejsca raz na grę i poziomy wszystkich prowincji po zmianie (`buildingsRevision`); `render/buildings.ts` rysuje okrągłe ikony z poziomem, w kolorze rodzaju budynku (`BUILDING_COLORS`), o stałej wielkości na mapie (`ICON_TILES` kafli – rosną z przybliżeniem; widać je od 2 px na kafel) i gwiazdki centrum dowodzenia nad miastem stolicy (1–2 brązowe, 3–4 srebrne, 5–6 złote; ułożone w rzędach jak kropki na kostce – `STAR_ROWS`: 1, 2, 2+1, 2+2, 3+2, 3+3). Kliknięcie ikony – okienko budynku (`ui/building-popup.ts`; fabryka – panel produkcji `ui/factory-panel.ts`).
- **Centrum dowodzenia** (część etapu 5): budynek `CommandCenter` tylko w stolicy (prowincja startowa – `Game::capital`), poziom 1 od startu, poziom 2 za 30 000 złota i 30 materiałów, każdy kolejny ×2 (`buildings::cost`). `Game::command_level` odblokowuje: rodzaje zaopatrzenia (`factories::COMMAND_LEVEL`), fronty (`war::MAX_FRONTS`: 2, 2, 3, ∞…; prowincji na froncie `MAX_FRONT_PROVINCES`: 1, 2, 3, 4, ∞, ∞), desant morski (`MAX_SEA_LANDINGS`: od 2 – jeden, od 3 – bez limitu), siłę i oszczędność paliwa zaopatrzenia (`war::MILITARY`), umocnienia (`FORTIFICATION_CAP`: 1 od CD 2, 2 od CD 3, 3 od CD 5). Tabele odblokowań (CD jako gwiazdki – `ui/cd-stars.ts`, uniwersytety) – zakładka „Odblokowania” w oknie zapasów (`ui/unlocks.ts`). Stan GDD i lista brakujących mechanik: `docs/gdd/plan.md`.
- **Brakuje:** aneksji po utracie stolicy, centrum dowodzenia (etap 5), cywilów → podatki (etap 4) i rynku.

### Dane mapy gotowe dla mechanik

- **Ruch jednostek:** przechodni jest tylko kafel z `province > 0`; woda (ocean, jeziora, rzeki) i góry są nieprzechodnie. Obszary odcięte górami i morzem są osobnymi lądami – przejścia (tunel, desant, statki) to mechanika do zrobienia. Przy wielu kontynentach bez przepraw kontynenty są dla siebie nieosiągalne.
- **Prowincje:** `MapData.province` (numer na kaflu) i `MapData.provinces` (stałe właściwości: `area`, `biome`, `coastal`, `river`, `lake`, `mountains`, `centerX`/`centerY`, `tunnel` – z fazy 3) i `MapData.enclaves`. Każdy kafel lądu poza górami, lodowcem i enklawami należy do prowincji – ziarno prowincji nie może przeskoczyć na sąsiedni ląd na siatce zgrubnej – a do każdej prowincji da się dojść, zwykle albo tunelem (test na pełnej mapie). Pod mechanikę tuneli: `Province.tunnel` mówi, które prowincje jej potrzebują. Wszystkie prowincje mają podobną wielkość (~`provinceSize` kafli, odchylenie ok. 11%) – pasek odchyłu w ramce prowincji jest pomyślany pod balansowanie prowincji startowych.
- **Biomy i lasy:** `MapData.biome` (rodzaj dominujący; typ – `Biome::kind_of`) i `MapData.forest` (≥ 128 = las), np. koszt ruchu, drewno, premia do obrony.

### Gdzie dopisywać

- **Intencje:** warianty `Intent` w `core/protocol.rs` (TS generuje się sam – `npm run types`).
- **Egzekucja i walidacja:** `Game::apply_turn` w `core/game.rs` – walidacja w rdzeniu, nie na serwerze. Nowy stan gry to nowe pola `Game` inicjalizowane w `Game::from_map` (wtedy `restart` zeruje je sam) i dopisane do `state_hash`. Mapy nie zmieniaj – np. wykarczowany las trzymaj jako osobny stan. W symulacji tylko liczby całkowite / stałoprzecinkowe albo `libm` (determinizm natywny vs wasm, patrz niżej).
- **Stan gry dla UI:** worker ma `WasmGame` (`worker/game.worker.ts`); nowe dane dla ekranu (np. właściciele prowincji) dodaj jako metodę `WasmGame` i pole zdarzenia `game` w `worker/protocol.ts` (worker wysyła je po wykonaniu tur), a w `GameSession` – sygnał.
- **Wysyłanie intencji:** `Transport.send({ type: 'intent', intent })` – serwer stempluje intencję ID gracza i dokłada do najbliższej tury, więc skutek widać dopiero po jej wykonaniu (u wszystkich graczy w tym samym ticku).
- **Renderowanie stanu:** terytoria maluje worker malujący (`paintTerritories` w `render/owners.ts`) po każdej zmianie właścicieli: warstwa polityczna, tło w kolorze państwa, tło z flagą (flaga rozciągnięta z zachowaniem proporcji na każdy spójny kawałek terytorium) i granice państw (`borderWidth` kafli w głąb, krawędź pełna, w głąb słabsza – `BORDER_INNER`; woda, ląd niczyj i inne państwo są granicą), do tego położenie nazw. Przy częstych zmianach (ekspansja kafel po kaflu) lepsza będzie tekstura numerów państw + shader (jak `render/highlight.ts`) albo przemalowanie tylko zmienionych tekstur 512 × 512.
- **Lobby:** lista pokoi, zakładanie, bilety, poczekalnia ze startem przez gospodarza i ustawienia mapy (`MapSettings`) już są (`crates/meta`, `ui/lobby-panel.ts`); mapa generuje się w tle już w poczekalni. Dalej ewentualnie: automatyczny start po N graczach lub czasie (`room.rs`).

### Proponowana kolejność pierwszych mechanik (do uzgodnienia z użytkownikiem)

1. ~~**Pętla lockstep end-to-end:** klient generuje mapę z `GameConfig` z `Welcome`, worker trzyma `WasmGame` zbudowany z tej mapy, wykonuje tury i odsyła hashe; test na dwóch kartach przeglądarki bez desynców.~~ – zrobione.
2. ~~**Właściciele w rdzeniu:** właściciel na kaflu (państwa), udziały w prowincjach, graf sąsiedztwa prowincji w Ruście, w `state_hash`; mapa polityczna w kolorach państw.~~ – zrobione (testowa intencja `Claim`).
3. ~~**Tożsamość gracza po ponownym połączeniu:** ten sam `PlayerId` dla tej samej karty.~~ – zrobione.
4. ~~**Start gracza:** 20 s na wybór prowincji startowej (odliczanie pod paskiem map) albo start losowy z lobby; bez wyboru – start losowy z odstępami.~~ – zrobione.
5. ~~**Populacja i złoto:** limit z kafli, przyrost ze szczytem przy 55% i skończonym dojściem do limitu, wojsko/robotnicy z suwakiem z bezwładnością, podatek od robotników co miesiąc (30 s), pasek zasobów, kalendarz, nazwy prowincji.~~ – zrobione.
6. ~~**Pierwsza ekspansja:** intencja zajęcia sąsiedniej prowincji (bez gór i przez morze tylko przy przeprawie), prosty koszt/czas.~~ – zrobione (front kafel po kaflu, walka z graczami; aneksja – następny krok).

## Stan projektu i plan

**Gotowe** (wszystko na `main`):

| Obszar | Co jest |
|---|---|
| Szkielet | workspace Rust (`mapgen`, `core`, `wasm`, `ticket`, `meta`, `server`), Angular 22 + Pixi 8, worker z wasm, serwer tur lockstep (wiele pokoi, bilety Ed25519, `/health`), typy TS z `ts-rs` |
| Lobby | ekran powitalny z listą pokoi i liczbą graczy online, zakładanie lobby z ustawieniami mapy, poczekalnia (skład na żywo, gospodarz zmienia ustawienia i startuje grę), powrót do gry po zerwaniu połączenia, test e2e bez AWS (`tools/lobby/`) |
| Chmura (AWS) | Terraform w repo: S3 + CloudFront (frontend, `/api/*`, `/ws*`), lobby (Lambda w Ruście + HTTP API + DynamoDB, bilety Ed25519), EC2 t4g.micro z game-serverem (Elastic IP, SG tylko z CloudFront, sekret originu w SSM, logi CloudWatch, alarm `recover`), alarmy CloudWatch z e-mailem, ochrona przed spamem (limity na IP – ADR 0007), budżety; wdrażanie z VS Code (`tools/deploy/`, `all.mjs` + `smoke.mjs`), ADR-y w `docs/adr/` |
| Pętla gry | właściciele kafli (państwa, prowincje współdzielone, graf sąsiedztwa) z testową intencją `Claim`, mapa polityczna w kolorach państw; lockstep end-to-end: mapa z `GameConfig` z `Welcome` (generowana raz), `WasmGame` w workerze z tej mapy, tury, hashe stanu co 10 tur, nadrabianie (`Catchup`) dla spóźnionych i po ponownym połączeniu, komunikat o desyncu, test na kilku kartach (`tools/lockstep/`) |
| Generator | kontynenty, wybrzeża, góry nieprzechodnie i niczyje (polarne oblodzone), lód morski przy lądolodzie, jeziora, rzeki (nieprzechodnie, granice prowincji), 5 typów klimatu i 13 rodzajów biomów z wariantami, płynnymi przejściami i zasadami par, dno oceanu, lasy, prowincje o równej wielkości (liczbie kafli) z naturalnymi granicami i stałymi właściwościami (biom, sąsiedztwo morza, rzeki, jeziora, gór), dostęp do każdej prowincji (tunele, enklawy z yeti), 39 zasobów z tabeli szans (złoto gwarantowane) |
| Renderer | palety biomów, wyraźny lądolód z cieniem, zamarznięte wody, ocean z izobatami, fale brzegowe, nurt rzek i zmarszczki jezior (shaderami), widok biomów, granice prowincji, mapa polityczna, mapa zasobów z ikonami, podświetlenie prowincji, sterowanie dotykiem (telefony) |
| Interfejs gracza | górny pasek (dopasowanie F, mapy 1–4, opcje pod zębatką), ramka z danymi prowincji z paskiem odchyłu wielkości, zalesieniem i zasobem (najechanie, kliknięcie), legenda mapy zasobów, napis ładowania z kółkiem na środku |
| Wydajność | generowanie trójfazowe (teren, prowincje, ostatnie szlify), mapa generowana już w poczekalni, malowanie warstw w osobnym workerze z pamięcią 3 widoków i przenikaniem, kompresja plików w serwerze, narzędzia `tools/loadtest/` |
| Narzędzia | panel debugu ze strojeniem wszystkiego (z podglądem prowincji pod kursorem), CLI `mapgen` z podglądem PNG, 160 testów w Ruście |

**Następne kroki:**

1. **Gospodarka wg GDD** ([docs/gdd/plan.md](docs/gdd/plan.md)) – surowce i pula gracza, budynki z obsadą robotników, fabryki i zaopatrzenie, zaopatrzenie cywilne a podatki, centrum dowodzenia; potem pierwsza ekspansja z zużyciem zaopatrzenia wojskowego (kolejność do uzgodnienia).
2. **Chmura – kolejne etapy** (infra/README.md): symulacja-cień na serwerze (autorytatywny hash, hashe mapy w `Welcome` – docs/adr/0006) – najlepiej zaraz po pierwszych intencjach zmieniających stan.
3. **Wydajność (gdy będzie potrzebna):** pamięć wygenerowanych map w IndexedDB (seed + parametry + wersja); prowincje równolegle per kontynent w kilku workerach (bez wątków wasm i COOP/COEP – świadomie odłożone, zysk ok. 2×).

**Odrzucone pomysły** (sprawdzone i wycofane – nie wracać bez wyraźnej prośby):

- animacje otwartego oceanu: grzywacze, paczki fal niesione prądami morskimi, falowanie/refleksy, błyski słońca na tafli – zostały tylko fale brzegowe,
- żółte, oliwkowe i rdzawe (kwitnące) korony w dżungli – dżungla ma być zielona–ciemnozielona,
- wartość prowincji z ukształtowania (nizina/wyżyna/góry), a potem z żyzności – prowincje mają po prostu równą liczbę kafli; żyzność usunięta z generatora,
- przełęcze wycinane przez generator w górach – wyglądały sztucznie; przejścia przez góry mają być mechaniką (tunel, desant),
- wątki wasm (Rayon + SharedArrayBuffer + COOP/COEP) – nightly Rust i ograniczenia izolacji strony, a zysk tylko ok. 2×.

## Czas wczytania

Mapa nie jest przesyłana – przeglądarka generuje ją z seeda. Przez sieć idzie tylko aplikacja: ok. 1 MB (ok. 350 KB po kompresji, w tym wasm 285 KB / 119 KB brotli). Serwer (`crates/server`) kompresuje pliki w locie (`tower-http` `Compression`, brotli albo gzip). Renderer oznacza teren na ekranie znacznikiem `performance.mark('map-rendered')`, a prowincje – `provinces-rendered` (narzędzia: `tools/loadtest/`).

Pomiar (wrzesień 2026, domyślna mapa 1400 × 1400, build produkcyjny, Chromium, kontener 4 × Xeon 2,1 GHz; sieć dławiona serwerem testowym, CPU – wstrzymywaniem procesu przeglądarki):

| Etap | Czas |
|---|---|
| teren na ekranie (faza 1 + tekstury) | ok. 3,3 s |
| prowincje na ekranie (faza 2) | ok. 5,9 s od startu (liczenie ok. 3 s) |
| sieć: światłowód / kablówka / LTE | +0,1 / +0,2 / +0,5 s |
| sieć: słabe 4G (1,6 Mb/s, 150 ms) | +2,5 s z gzipem, +5,6 s bez |
| sieć: EDGE (0,4 Mb/s, 400 ms) | +9 s z gzipem, +23 s bez |

Czas CPU skaluje się liniowo z wydajnością jednego rdzenia (sprawdzone dla spowolnienia 2× i 3,5×).

## Rozwiązywanie problemów

- **Angular CLI zgłasza wersję Node** – zaktualizuj Node do 22.22.3+ lub 24+.
- **`Cannot find module '../../generated/...'`** – uruchom `npm run types` (albo `npm run prep`).
- **Pusta mapa, w konsoli błąd workera o MIME lub 404 dla `.wasm`** – sprawdź wpis `assets` z `src/wasm/pkg` w `angular.json` i czy `npm run wasm` utworzył `game_wasm_bg.wasm`.
- **Błąd wersji `wasm-bindgen`** – wersja CLI musi być identyczna z wersją crate'a. Używaj `wasm-pack`, który pilnuje tego sam.
- **`wasm-pack` nie może pobrać `wasm-opt`** (np. za proxy) – zainstaluj binaryen i dodaj `wasm-opt` do `PATH`; `wasm-pack` użyje go zamiast pobierać.
- **Terminal `ng serve` pokazuje błędy proxy `ECONNREFUSED`** – serwer Rust nie działa. Frontend działa dalej, a `Transport` ponawia połączenie co 5 s.
- **Hash w panelu inny niż w CLI** – pakiet wasm jest nieaktualny; uruchom `npm run wasm` i przeładuj stronę.
- **„Gra zatrzymana: wersja generatora serwera … inna niż klienta”** – serwer i frontend są z różnych wersji; przebuduj oba (`npm run prep`, restart serwera) i odśwież stronę.
- **„Stan gry rozjechał się z innymi graczami”** – desync: hashe stanu różnią się między graczami. Sprawdź, czy wszyscy mają ten sam pakiet wasm, a potem szukaj niedeterminizmu w rdzeniu (patrz [Determinizm i hashe](#determinizm-i-hashe)); log serwera podaje gracza i tick.
- **Generowanie za wolne** – zmniejsz `width`/`height` w panelu albo wyłącz „Generuj po każdej zmianie” i generuj klawiszem G.
