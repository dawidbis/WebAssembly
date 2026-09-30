# Mapa – generator świata i szkielet gry (Rust/WASM + Angular + Pixi)

Proceduralny generator map z seeda (Rust, kompilowany natywnie i do WebAssembly), serwer tur (lockstep) i frontend w Angularze z rendererem Pixi. Mechanik gry jeszcze nie ma – są miejsca, w które wejdą (patrz [Gdzie wejdą mechaniki](#gdzie-wejdą-mechaniki)).

Ten sam seed i te same parametry dają identyczną mapę natywnie, w wasm i u każdego gracza. Typy wiadomości i parametrów są zdefiniowane raz w Ruście, a TypeScript dostaje je automatycznie (`ts-rs`).

## Spis treści

- [Wymagania](#wymagania)
- [Szybki start](#szybki-start)
- [Struktura repozytorium](#struktura-repozytorium)
- [Architektura](#architektura)
- [Generator map](#generator-map)
- [Parametry generatora](#parametry-generatora)
- [Frontend i renderer](#frontend-i-renderer)
- [Panel debugu i klawisze](#panel-debugu-i-klawisze)
- [CLI `mapgen`](#cli-mapgen)
- [Serwer i protokół](#serwer-i-protokół)
- [Determinizm i hashe](#determinizm-i-hashe)
- [Kontrakty utrzymywane ręcznie](#kontrakty-utrzymywane-ręcznie)
- [Gdzie wejdą mechaniki](#gdzie-wejdą-mechaniki)
- [Stan projektu i plan](#stan-projektu-i-plan)
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

Otwórz `http://localhost:4200`. Sekcja „Serwer” w panelu pokazuje `online` i rosnący numer tury. Frontend działa też bez serwera – wtedy tylko połączenie jest `offline`, a `Transport` ponawia je co 5 s.

`npm run prep` trzeba powtórzyć po każdej zmianie w Ruście, która dotyka typów (`MapGenParams`, `MapStats`, protokół) albo generatora (pakiet wasm).

**Build produkcyjny:** `cd web && npm run build`, potem z katalogu głównego `cargo run -p game-server --release` i `http://localhost:3000`. Panel debugu nie istnieje w tym buildzie.

**Testy:** `cargo test --workspace --all-features`.

## Struktura repozytorium

```
.
├── Cargo.toml              # workspace
├── .cargo/config.toml      # alias `cargo types` + katalog dla typów TS
├── crates/
│   ├── mapgen/             # generator map z seeda (+ CLI do podglądu PNG)
│   │   └── src/
│   │       ├── lib.rs      # MapGenParams, MapData, Terrain, Biome, generate()
│   │       ├── layout.rs   # siatka makro-chunków i kontynenty
│   │       ├── relief.rs   # ląd, wybrzeża, rzeźba, klasyfikacja terenu
│   │       ├── biome.rs    # biomy kontynentów z płynnymi przejściami
│   │       ├── hydro.rs    # jeziora i rzeki
│   │       ├── ocean.rs    # dno oceanu: szelf, stok, rzeźba dna
│   │       ├── vegetation.rs # lasy i żyzność gleby
│   │       ├── util.rs     # RNG, szum, pola odległości, percentyle
│   │       └── bin/mapgen.rs
│   ├── core/               # deterministyczny rdzeń: protokół, stan gry, hash
│   ├── wasm/               # cienkie bindingi wasm-bindgen
│   └── server/             # axum: przekaźnik tur + serwowanie frontendu
└── web/                    # Angular 22
    ├── proxy.conf.json     # /ws → serwer Rust w trybie dev
    └── src/
        ├── generated/      # typy z ts-rs (generowane, nie w repo)
        ├── wasm/pkg/       # wynik wasm-pack (generowany, nie w repo)
        └── app/
            ├── worker/     # web worker: ładuje wasm, generuje mapę
            ├── game/       # serwisy: WorkerBridge, MapStore, Transport
            ├── render/     # czysty TS + Pixi: teren, fale, rzeki i jeziora, drzewa, siatka chunków, kamera
            └── debug/      # panel deweloperski (tylko w buildzie dev)
```

## Architektura

Kierunek zależności: `mapgen` ← `core` ← (`wasm`, `server`). `web` nie importuje Rusta bezpośrednio, tylko wygenerowane typy i pakiet wasm.

Podział odpowiedzialności we frontendzie:

- **Worker** (`worker/game.worker.ts`) ładuje wasm raz i generuje mapę. Gotowe bufory wysyła jako *transferable*, bez kopiowania. Liczy też hashe i odległość kafli oceanu od brzegu (do animacji fal).
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
7. **Roślinność i żyzność** (`vegetation.rs`) – patrz niżej.
8. **Prowincje** (`provinces.rs`) – patrz niżej.

Biomy, ocean, roślinność i prowincje mają **własne strumienie losowości** (seed XOR stała), więc ich ustawienia nie zmieniają kształtu lądu, rzek ani jezior. Tak samo biomy nie zależą od ustawień oceanu ani lasów, a nic nie zależy od ustawień prowincji.

Szum jest liczony na siatce co 2 kafle i interpolowany (`CoarseField`) – około 4× mniej obliczeń bez widocznej straty.

### Biomy

Pięć biomów: **umiarkowany, pustynny, zimny, wilgotny (dżungla), step**. Każdy kontynent dostaje biom główny, a z szansą `biomeMixChance` także drugi.

- **Wybór biomu:** ważone losowanie według szans biomów (`biomeTemperate` … `biomeSteppe`). Przy `biomeLatitude > 0` (wpływ biegunów klimatu) szanse przesuwa położenie kontynentu względem **biegunów klimatu**: biegun zimna leży przy górnej albo dolnej krawędzi (losowo), a biegun ciepła naprzeciwko, po przekątnej mapy. Chłód kontynentu (odległości do biegunów) jest rozciągnięty na pełny zakres – najzimniejszy kontynent „leży na biegunie zimna”, najcieplejszy na biegunie ciepła. Idealny chłód biomów od najcieplejszego: pustynia, dżungla, step, umiarkowany, zimny (`IDEAL_COLDNESS` w `biome.rs`). Twarda reguła: zimny biom tylko po zimnej połowie, pustynia, dżungla i step tylko po ciepłej, umiarkowany wszędzie – zimno i ciepło są jak najdalej od siebie.
- **Dozwolone pary:** drugi biom wybierany jest tylko z par dozwolonych w `biomePairs` (maska bitowa, bit = indeks w `BIOME_PAIRS`). Domyślnie:

  | Para | Domyślnie |
  |---|---|
  | Umiarkowany + Pustynny / Zimny / Step | ✅ |
  | Umiarkowany + Wilgotny | ❌ |
  | Pustynny + Step | ✅ |
  | Pustynny + Zimny, Pustynny + Wilgotny | ❌ |
  | Zimny + Wilgotny, Zimny + Step | ❌ |
  | Wilgotny + Step | ✅ |

  Wilgotny (dżungla) łączy się więc tylko ze stepem, a zimny tylko z umiarkowanym.
- **Przejście:** granica między biomami to pofalowana szumem linia w poprzek kontynentu (przy wpływie biegunów chłodniejszy biom leży bliżej bieguna zimna). Strefa przejścia o szerokości `biomeTransition` kafli miesza oba biomy płynnie (smoothstep) z przeplatającymi się płatami. Udział drugiego biomu (`biomeSecondaryShare`) jest dobierany percentylem.
- **Wynik na kafel:** `biome` (biom dominujący – liczy się w rozgrywce i w hashu stanu gry), `biomeOther` (drugi biom w strefie przejścia) i `biomeMix` (udział drugiego biomu, 0..128). Woda dostaje biom najbliższego lądu.
- Cały spójny ląd należy do jednego kontynentu (głosowanie chunków), więc w obrębie lądu nie ma twardych szwów.

### Dno oceanu

Głębokość kafla oceanu (`shade`, 0..255) zależy od odległości od lądu:

- **szelf** – płytki pas przy brzegu o średniej szerokości `shelfWidth`, zmiennej szumem (`shelfVariation`): szerokie ławice obok urwisk,
- **stok kontynentalny** – spadek do głębi, stromy przy dużym `slopeSteepness`,
- **równina abisalna** z rzeźbą dna (`seabedRelief`): podwodne grzbiety, rowy i góry podwodne.

### Roślinność i żyzność

**Lasy.** Każdy kafel lądu ma gęstość lasu `forest` (0..255; ≥ 128 = kafel leśny w rozgrywce). Typ lasu nie jest zapisywany osobno – wynika z biomu kafla, więc w strefach przejścia biomów las przechodzi płynnie tak jak kolory:

| Biom | Las |
|---|---|
| Umiarkowany | liściasty / mieszany |
| Zimny | tajga (rzednie szybciej z wysokością – tundra) |
| Wilgotny | dżungla, bardzo gęsta |
| Step | zagajniki, głównie wzdłuż rzek |
| Pustynny | oazy tylko przy wodzie |

Gdzie rośnie las: zwarte masywy z szumu (`forestClumping`), więcej przy rzekach, jeziorach i wybrzeżu (`forestMoisture`; na stepie i pustyni ta waga jest dużo większa), mniej na wyżynach, nigdy na górach. Udział lasu w biomie (`forestTemperate` … `forestSteppe`) jest ustalany **percentylem** wśród kafli bez gór, więc nie zależy od seeda. Próg jest mieszany między biomami według `biomeMix`, więc na granicy biomów nie ma szwów. Skraj lasu jest szeroki i miękki – renderer rozbija go na pojedyncze drzewa.

**Żyzność** (`fertility`, 0..255) – wyznacza wartość prowincji, a później pola uprawne wokół miast (ich intensywność będzie zależeć od infrastruktury prowincji). Zależy od biomu (umiarkowany 1.0, wilgotny i step 0.8, zimny 0.6 – tajga rośnie, więc gleba nie jest jałowa – pustynia 0.1; `BIOME_FERTILITY` w `vegetation.rs`), rzeźby (równiny > wyżyny – poza biomem umiarkowanym, gdzie wyżyny są tak żyzne jak niziny; `HIGHLAND_PENALTY` w `vegetation.rs`; góry jałowe) i bliskości wody. Brzegi rzek i jezior dostają dodatek `fertilityRiverBonus` w wąskim pasie `fertilityRiverReach` kafli – także na pustyni, jak dolina Nilu. Nie zależy od lasów – las można wykarczować.

### Góry nieprzechodnie

Góry – i rzeki płynące przez góry (w otoczeniu 5 × 5 więcej gór niż dostępnego lądu) – **blokują ruch jednostek i są niczyje**: nie należą do żadnej prowincji. W danych: kafel lądu albo rzeki z `province == 0` jest nieprzechodni.

Generator nie wycina przełęczy (wyglądały sztucznie). Obszar odcięty górami jest dla prowincji osobnym lądem, jak wyspa; przejścia przez góry (np. budowa tunelu, desant) będą mechaniką rozgrywki. Kieszenie dostępnego lądu przy górach mniejsze niż 40 kafli stają się górami (`mountains.rs`, bez losowości).

### Prowincje

Dostępny ląd (razem z kaflami rzek, bez gór) jest podzielony na prowincje – podział administracyjny pod przyszłe mechaniki. Wynik: `MapData.province` (numer prowincji na kaflu, od 1; 0 = woda) i `MapData.provinces` (lista `Province`: `id`, `area`, `value`, `fertility` – średnia żyzność kafli lądu 0..255, `centerX`/`centerY` – kafel środka, `riverTiles`, `coastal`).

- **Wartość z żyzności.** Każda prowincja ma podobną wartość – sumę wartości kafli, a wartość kafla rośnie z żyznością: od `provinceValueFloor` (jałowa ziemia; tyle ma też kafel rzeki) do 1 (najżyźniejsza). Prowincja na pustyni jest więc duża, a na żyznej dolinie mała. Średnia wartość to `provinceValue`; typowe odchylenie ok. 13%.
- **Liczba prowincji** na każdym lądzie = wartość lądu / `provinceValue`, z poprawką tak, żeby prowincje mieściły się między `provinceMinSize` a `provinceMaxSize` kafli.
- **Wzrost.** Zalążki startują z pocięcia krzywej Hilberta na kawałki o równej wartości. Potem w kilkudziesięciu rundach prowincje rosną od zalążków (Dijkstra, sąsiedztwo 8, kolejka kubełkowa), prowincja za cenna dostaje „handicap” i startuje później, za uboga – wcześniej, a zalążki przesuwają się do środka prowincji (Lloyd). Większość rund liczy się na siatce 2 × 2 (4× szybciej), ostatnie w pełnej rozdzielczości.
- **Naturalne granice.** Koszt drogi przez teren: przejście przez rzekę i wspinaczka są drogie, więc granice chętnie biegną rzekami i grzbietami wyżyn (`provinceNaturalBorders`); góry, jeziora i morze są nieprzekraczalne, więc wyspy i półwyspy za cieśniną mają własne prowincje. Małe wyspy (mniejsze niż `provinceMinSize`) dołączają przez morze do najbliższej prowincji, a zbyt odległe dostają własną.
- **Kształt.** Szum w kosztach i drobne przesunięcie granic (bez przeskakiwania rzek) dają nieregularne granice jak prawdziwe granice powiatów (`provinceRoughness`). Każda prowincja jest spójna na swoim lądzie; okruchy i za małe prowincje dołączają do sąsiadów.

Przy domyślnych ustawieniach (dużo lądu) generowanie mapy 2000 × 1000 trwa natywnie ok. 3,8 s, z czego większość to prowincje.

## Parametry generatora

Wszystkie pola `MapGenParams` w camelCase (tak jak w JSON i TS). Wartości spoza zakresu są przycinane.

**Rozmiar i układ**

| Pole | Domyślnie | Działanie |
|---|---|---|
| `seed` | 1 | seed (u32) |
| `width`, `height` | 2000, 1000 | rozmiar w kaflach (64..4096) |
| `chunkCols`, `chunkRows` | 12, 6 | siatka makro-chunków |
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
| `biomes` | true | wyłączone = cały ląd umiarkowany |
| `biomeTemperate`, `biomeDesert`, `biomeCold`, `biomeHumid`, `biomeSteppe` | 0.9, 0.3, 0.3, 0.35, 0.4 | szanse (wagi względne; panel pokazuje je jako procent sumy) |
| `biomeLatitude` | 0.6 | wpływ biegunów klimatu (0 = biomy losowe) |
| `biomeMixChance` | 0.5 | szansa, że kontynent ma dwa biomy |
| `biomePairs` | patrz tabela par | maska dozwolonych par |
| `biomeSecondaryShare` | 0.4 | średni udział drugiego biomu (losowany ±25%, max 0.5) |
| `biomeTransition` | 60 | szerokość strefy przejścia (kafle) |
| `biomeRoughness` | 0.5 | pofalowanie granicy i przeplatanie płatów |

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
| `forest` | true | wyłączone = brak lasów (żyzność liczona zawsze) |
| `forestTemperate`, `forestDesert`, `forestCold`, `forestHumid`, `forestSteppe` | 0.4, 0.03, 0.75, 0.95, 0.08 | docelowy udział lasu w lądzie biomu (bez gór) |
| `forestClumping` | 0.85 | zwartość: 0 = drobne kępy, 1 = duże masywy |
| `forestMoisture` | 0.5 | jak mocno las ciągnie do wody |
| `fertilityRiverBonus`, `fertilityRiverReach` | 0.6, 15 | dodatek do żyzności na brzegach rzek i jezior i szerokość tego pasa (kafle) |

**Prowincje**

| Pole | Domyślnie | Działanie |
|---|---|---|
| `provinces` | true | podział lądu na prowincje |
| `provinceValue` | 400 | docelowa (średnia) wartość prowincji; = liczba kafli o najwyższej żyzności |
| `provinceValueFloor` | 0.25 | wartość jałowego kafla (i kafla rzeki); wartość kafla = minimum + (1 − minimum) × żyzność |
| `provinceMinSize`, `provinceMaxSize` | 120, 4000 | najmniejsza i największa prowincja (kafle) |
| `provinceNaturalBorders` | 0.6 | jak mocno granice trzymają się rzek i grani |
| `provinceRoughness` | 0.5 | nieregularność granic (0 = gładkie, zaokrąglone) |

## Frontend i renderer

- **Teren** (`render/terrain.ts`) – każdy biom ma własną paletę: równiny i wyżyny (gradient wg wysokości), skały i śnieg na górach (próg śniegu zależny od biomu), jeziora i rzeki. W strefie przejścia kolory obu biomów są mieszane według `biomeMix`. Rzeźbę lądu cieniuje światło z lewego górnego rogu.
- **Ocean** – paleta głębokości z wyraźnym, jasnym szelfem, jasna linia brzegu, słabe cieniowanie dna i **izobaty** (linie jednakowej głębokości na 5 stałych poziomach, bardzo przezroczyste – `CONTOUR_OPACITY`).
- **Lasy** – korony drzew w kolorze zależnym od biomu (liściasty, tajga przyprószona śniegiem, ciemna dżungla, zagajniki, palmy oaz) z ziarnistą teksturą; na skraju lasu pojedyncze drzewa.
- **Symbole drzew** (`render/trees.ts`) – przy przybliżeniu (od ok. 4 px na kafel, w pełni od 8) na kaflach lasu pojawiają się drzewa rysowane shaderem: dęby o pofalowanych koronach z kępami liści, a co piąte drzewo to świerk (las umiarkowany), piętrowe stożki z czapami śniegu (tajga), zwarty dach koron-„brokułów” z drobnymi, oświetlonymi guzkami, ciemnymi szczelinami cienia, pojedynczymi wyższymi drzewami w odcieniach od ciemnej po średnią zieleń (dżungla), akacje sawannowe z płaskim daszkiem korony i kępy wysokiej trawy (step) oraz palmy (oazy). W tajdze i dżungli przypadają do dwóch drzew na kafel. Każde drzewo ma losowe położenie w kaflu, rozmiar, jasność i odcień (od żółtawej do niebieskawej zieleni), a wolnozmienny szum dodaje płaty innej zieleni – w dżungli wyraźne; w strefie przejścia biomów losuje gatunek według udziału biomów. Cień pada w prawo w dół, zgodnie z oświetleniem rzeźby (światło z lewego górnego rogu). Razem z pojawianiem się drzew ziarnista warstwa koron płynnie ustępuje gruntowi lekko przyciemnionemu cieniem lasu, więc pod symbolami nie ma podwójnego lasu. Z daleka shader jest wyłączony.
- **Widok „mapa biomów”** – płaskie kolory biomów zamiast pełnego stylu, do strojenia (las jako ciemniejszy odcień).
- **Widok „mapa żyzności”** – ląd od jałowego brązu przez słomkowy do soczystej zieleni.
- **Fale brzegowe** (`render/waves.ts`) – nakładka rysowana shaderem GLSL co klatkę nad terenem: grzbiety przyboju płyną w stronę brzegu i wygasają dalej od lądu, a przy samej linii brzegu pulsuje piana. To czysto wizualny efekt – nie zmienia danych mapy. Gdy system prosi o ograniczenie ruchu (`prefers-reduced-motion`), fale są domyślnie wyłączone.
- **Rzeki i jeziora** (`render/inland.ts`) – animacja rysowana shaderem od ok. 1,5 px na kafel (w pełni od 3,5): po rzekach płyną z prądem jasne smugi i zmarszczki (ok. 3 kafle/s, w stronę ujścia), a na jeziorach powoli przesuwają się delikatne zmarszczki i falująca piana przy brzegu. Kierunek nurtu daje generator (`MapData.riverFlow` – odległość do ujścia wzdłuż rzeki; dopływ dziedziczy odległość rzeki, do której wpada). Włączana razem z falami brzegowymi (klawisz W), jasność suwakiem „Rzeki i jeziora”.
- **Granice prowincji** (`render/provinces.ts`) – nakładka z półprzezroczystych szarych kafli (krycie suwakiem w górnym pasku, domyślnie 0.3 – teren pod granicą pozostaje widoczny): granicą jest kafel, którego prawy albo dolny sąsiad należy do innej prowincji, więc linia ma grubość jednego kafla (bez wektorów i linii na siatce). Brzeg morza i jezior nie jest granicą. Rysowana nad drzewami, pod falami.
- **Mapa polityczna** – same prowincje (góry szare, niczyje): płaskie kolory (sąsiednie prowincje zawsze w różnych kolorach – zachłanne kolorowanie grafu sąsiedztwa), ciemnoczerwone granice, jednolita woda; bez rzeźby, lasów, rzek, drzew i animacji wody.
- **Podświetlenie prowincji** (`render/highlight.ts`) – shader na teksturze numerów prowincji: prowincja pod kursorem lekko rozjaśniona, zaznaczona (kliknięcie) mocniej, z wyraźnym białym skrajem.
- Mapa jest cięta na tekstury 512×512 (bezpieczny limit dla mobilnych GPU). Renderer działa na WebGL, bo shadery fal, rzek i drzew mają tylko wersję GLSL.

## Interfejs gracza

Dostępny dla każdego gracza (także w buildzie produkcyjnym), w `web/src/app/ui/`:

- **Górny pasek** (`top-bar`) – rodzaj mapy (Teren, Polityczna, Biomy, Żyzność), granice prowincji z suwakiem krycia, drzewa, izobaty, animacja wody i „Dopasuj”. Obsługuje skróty widoku z tabeli niżej.
- **Ramka prowincji** (`province-info`, lewy dolny róg) – prowincja pod kursorem, a gdy kursor jest poza lądem – zaznaczona: numer, wartość z paskiem odchyłu od ustalonej średniej (`provinceValue`; pionowa linia = średnia, skala ±50%, kolor: do ±10% zielony, do ±25% żółty, dalej czerwony – pod przyszłe balansowanie prowincji startowych), powierzchnia, średnia żyzność, udział nizin/wyżyn/gór, biom dominujący, rzeki i dostęp do morza. Nad górami ramka informuje, że są nieprzechodnie i niczyje. Kliknięcie prowincji zaznacza ją, ponowne kliknięcie, kliknięcie wody albo Esc – odznacza.

## Panel debugu i klawisze

Panel (tylko build dev) pozwala stroić wszystkie parametry generatora. Suwaki przegenerowują mapę po puszczeniu, gdy zaznaczone jest „Generuj po każdej zmianie”. Sekcja „Wynik” pokazuje czas generowania, statystyki terenu, udział biomów, liczbę kontynentów z dwoma biomami, udział lasu i żyznego lądu, statystyki prowincji (liczba, wartość średnia ± odchylenie, min/max, rozmiary), hashe (terenu, biomów, roślinności, prowincji) i wersję generatora.  Sekcja „Widok” zawiera siatkę chunków i suwaki animacji wody (fale przy brzegu, rzeki i jeziora, prędkość); pozostałe przełączniki widoku są w górnym pasku.

Klawisze G, N, C i ` obsługuje panel debugu, pozostałe – górny pasek (działają też w produkcji).

| Klawisz | Akcja |
|---|---|
| G | generuj z bieżącymi ustawieniami |
| N | nowy losowy seed i generuj |
| F | dopasuj widok do mapy |
| 1 | mapa terenu |
| C | siatka chunków (chunki wodne lekko podświetlone) |
| B | mapa biomów (ponownie – powrót do terenu) |
| Z | mapa żyzności (ponownie – powrót do terenu) |
| P | granice prowincji |
| M | mapa polityczna – same prowincje (ponownie – powrót do terenu) |
| Esc | odznacz prowincję |
| T | symbole drzew przy przybliżeniu |
| I | izobaty |
| W | animacja wody (fale brzegowe, rzeki, jeziora) |
| ` | zwiń / rozwiń panel |

Przeciąganie przesuwa mapę, kółko przybliża względem kursora, kliknięcie (bez przeciągania) zaznacza prowincję. Klawisze nie działają, gdy kursor jest w polu seeda.

## CLI `mapgen`

Podgląd generatora bez przeglądarki:

```bash
cargo run -p game-mapgen --release --features cli -- --seed 1 --out map.png
cargo run -p game-mapgen --release --features cli -- --params p.json --view biomes --no-contours
cargo run -p game-mapgen --release --features cli -- --seed 1 --view fertility --out fertility.png
cargo run -p game-mapgen --release --features cli -- --seed 1 --view political --out political.png
cargo run -p game-mapgen --release --features cli -- --seed 1 --borders --out borders.png
```

- `--params p.json` – JSON z polami jak `MapGenParams` (camelCase), np. `{"continents": 1, "landRatio": 0.8}`; brakujące pola mają wartości domyślne,
- `--view biomes` – płaska mapa biomów, `--view fertility` – mapa żyzności, `--view political` – mapa polityczna,
- `--borders` – granice prowincji na mapie, `--border-opacity 0.55` – ich krycie,
- `--no-contours` – bez izobat.

CLI wypisuje statystyki oraz hashe terenu, biomów, roślinności i prowincji. Paleta jest ta sama co w przeglądarce (bez animacji fal).

## Serwer i protokół

`crates/server` to jedna binarka: WebSocket pod `/ws` i statyczny frontend z `web/dist/web/browser`.

```bash
cargo run -p game-server [--features debug] -- [--dev] [--port 3000]
```

Serwer nie symuluje gry – zbiera intencje, stempluje je ID gracza, co 100 ms rozsyła numerowaną turę (`Turn`) i porównuje hashe stanu od klientów (`Desync`, gdy się różnią). Pokój gry to aktor z wyłącznym dostępem do swojego stanu; połączenia rozmawiają z nim kanałami, bez `Mutex`.

Wiadomości (`crates/core/src/protocol.rs`, typy TS generowane):

- klient → serwer: `Join`, `Intent`, `Hash`,
- serwer → klient: `Welcome` (ID gracza i `GameConfig` z parametrami mapy), `Turn`, `Desync`,
- intencje debugowe (`RegenerateMap`, `SetPaused`) istnieją tylko z cechą `debug` i tylko gdy serwer działa z `--dev`.

## Determinizm i hashe

Rdzeń i generator muszą dawać ten sam wynik na każdej platformie:

- bez `HashMap`/`HashSet` w logice (losowa kolejność iteracji) – `Vec`/`BTreeMap`,
- bez zegara i bez losowości spoza seeda (własny RNG SplitMix64),
- w generatorze tylko operacje zmiennoprzecinkowe jednoznaczne w IEEE 754 (bez `sin`/`cos`/`exp` z biblioteki systemowej); funkcje przestępne w rdzeniu tylko z crate'a `libm`.

Szybki test „natywnie vs wasm”: dla seeda 1 z domyślnymi parametrami CLI i panel w przeglądarce muszą pokazać te same wartości:

| Hash | Seed 1, domyślne parametry |
|---|---|
| terenu (FNV-1a z `terrain`) | `833fe5a4` |
| biomów (FNV-1a z `biome`, `biomeOther`, `biomeMix`) | `b1965737` |
| roślinności (FNV-1a z `forest`, `fertility`) | `d5d45ccf` |
| prowincji (FNV-1a z bajtów `province`, u16 little endian) | `b2d7e509` |

Hashe zmieniają się przy każdej zmianie wartości domyślnych albo algorytmu – wtedy zaktualizuj tę tabelę.

`GENERATOR_VERSION` (obecnie 8) podbijaj przy każdej zmianie algorytmu – seed i wersja idą do konfiguracji gry i replayów.

## Kontrakty utrzymywane ręcznie

Większość zgodności pilnuje kompilator dzięki `ts-rs`. Kilka rzeczy trzeba utrzymywać ręcznie:

| Co | Gdzie | Uwaga |
|---|---|---|
| Wiadomości, intencje, `MapGenParams`, `MapStats`, `Province` | `core/protocol.rs`, `mapgen/lib.rs` | TS generowany automatycznie (`npm run types`) |
| Wartości `Terrain` i `Biome` | `mapgen/lib.rs` ↔ `render/terrain.ts` | ręcznie |
| Kolejność `BIOME_PAIRS` (bity `biomePairs`) | `mapgen/lib.rs` ↔ `render/terrain.ts` | ręcznie |
| Palety terenu, oceanu, koron drzew, żyzności, poziomy izobat | CLI `mapgen` ↔ `render/terrain.ts` | tylko wygląd |
| Hash kafla do ziarna lasu (`tile_hash` / `tileHash`) | CLI `mapgen` ↔ `render/terrain.ts` | tylko wygląd |
| Granica prowincji (kolor, krycie), kolory i kolorowanie mapy politycznej | CLI `mapgen` ↔ `render/provinces.ts`, `game/map-store.ts` | tylko wygląd |
| Granice chunków | `mapgen::chunk_start` ↔ `drawChunkGrid` | `ceil(c * size / count)` |
| Hashe terenu, biomów, roślinności i prowincji (FNV-1a) | CLI `mapgen` ↔ `game.worker.ts` | do porównań native vs wasm |
| `GENERATOR_VERSION` | `mapgen/lib.rs` | podbij przy każdej zmianie algorytmu |

## Gdzie wejdą mechaniki

- **Intencje** (atak, budowa, sojusz): warianty `Intent` w `core/protocol.rs`.
- **Egzekucja i walidacja**: `Game::apply_turn` w `core/game.rs`; każde nowe pole stanu dopisz do `state_hash` (teren, biom dominujący, kafle leśne i prowincje już tam są).
- **Pętla tur po stronie klienta**: w `Transport.onMessage` przekaż turę do workera; worker trzyma `WasmGame`, wywołuje `applyTurn`, co 10 ticków odsyła `stateHash` jako `ClientMsg::Hash`.
- **Terytoria**: worker zwraca delty kafli, renderer trzyma teksturę właścicieli i rysuje ją shaderem nad terenem.
- **Lobby**: `room.rs` – start gry po N graczach lub czasie, `Welcome` z konfiguracją i seedem mapy, `Catchup` z logiem tur dla wracających.
- **Biomy i lasy w rozgrywce**: `MapData.biome` (biom dominujący) i `MapData.forest` (≥ 128 = las) są gotowe do użycia, np. dla kosztu ruchu, drewna czy premii do obrony.
- **Ruch jednostek**: kafel lądu albo rzeki z `province == 0` (góry) jest nieprzechodni. Przejścia przez pasma (tunel, desant) – mechanika do zrobienia.
- **Prowincje w rozgrywce**: `MapData.province` i `MapData.provinces` (wartość, średnia żyzność, środek, dostęp do morza) są gotowe, np. pod podatki, rekrutację, własność terytoriów czy lokalizację stolic (`centerX`/`centerY`).
- **Pola uprawne**: pojawią się wokół miast na podstawie `MapData.fertility`; ich intensywność będzie zależeć od poziomu infrastruktury prowincji. Rysowane jako mozaika działek w teksturze terenu.

Przy wielu kontynentach bez statków kontynenty są dla siebie nieosiągalne, więc gra będzie potrzebować mechaniki przepraw albo trybu z jednym lądem.

## Stan projektu i plan

**Gotowe** (wszystko na `main`):

| Obszar | Co jest |
|---|---|
| Szkielet | workspace Rust (`mapgen`, `core`, `wasm`, `server`), Angular 22 + Pixi 8, worker z wasm, serwer tur lockstep, typy TS z `ts-rs` |
| Generator | kontynenty, wybrzeża, góry nieprzechodnie i niczyje, jeziora, rzeki, biomy z płynnymi przejściami i zasadami par, dno oceanu, lasy, żyzność, prowincje o równej wartości (z żyzności) z naturalnymi granicami |
| Renderer | palety biomów, ocean z izobatami, fale brzegowe, nurt rzek i zmarszczki jezior, symbole drzew przy przybliżeniu (wszystko shaderami), widoki biomów i żyzności, granice prowincji, mapa polityczna, podświetlenie prowincji |
| Interfejs gracza | górny pasek z rodzajem mapy i opcjami renderu, ramka z danymi prowincji (najechanie, kliknięcie) |
| Narzędzia | panel debugu ze strojeniem wszystkiego (z podglądem prowincji pod kursorem), CLI `mapgen` z podglądem PNG, 36 testów w Ruście |

**Następne kroki** (uzgodnione, jeszcze nie zrobione):

1. **Pola uprawne** – nie w generatorze. Pojawią się wokół miast na podstawie `MapData.fertility`, a ich intensywność będzie zależeć od poziomu infrastruktury prowincji. Prowincje już są – wymaga jeszcze miast.
2. **Mechaniki gry** – patrz [Gdzie wejdą mechaniki](#gdzie-wejdą-mechaniki).

**Odrzucone pomysły** (sprawdzone i wycofane – nie wracać bez wyraźnej prośby):

- animacje otwartego oceanu: grzywacze, paczki fal niesione prądami morskimi, falowanie/refleksy, błyski słońca na tafli – zostały tylko fale brzegowe,
- żółte, oliwkowe i rdzawe (kwitnące) korony w dżungli – dżungla ma być zielona–ciemnozielona.

## Rozwiązywanie problemów

- **Angular CLI zgłasza wersję Node** – zaktualizuj Node do 22.22.3+ lub 24+.
- **`Cannot find module '../../generated/...'`** – uruchom `npm run types` (albo `npm run prep`).
- **Pusta mapa, w konsoli błąd workera o MIME lub 404 dla `.wasm`** – sprawdź wpis `assets` z `src/wasm/pkg` w `angular.json` i czy `npm run wasm` utworzył `game_wasm_bg.wasm`.
- **Błąd wersji `wasm-bindgen`** – wersja CLI musi być identyczna z wersją crate'a. Używaj `wasm-pack`, który pilnuje tego sam.
- **`wasm-pack` nie może pobrać `wasm-opt`** (np. za proxy) – zainstaluj binaryen i dodaj `wasm-opt` do `PATH`; `wasm-pack` użyje go zamiast pobierać.
- **Terminal `ng serve` pokazuje błędy proxy `ECONNREFUSED`** – serwer Rust nie działa. Frontend działa dalej, a `Transport` ponawia połączenie co 5 s.
- **Hash w panelu inny niż w CLI** – pakiet wasm jest nieaktualny; uruchom `npm run wasm` i przeładuj stronę.
- **Generowanie za wolne** – zmniejsz `width`/`height` w panelu albo wyłącz „Generuj po każdej zmianie” i generuj klawiszem G.
