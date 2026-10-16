# Plan: balans surowców, receptur i gospodarki

Stan na 2026-10-17 (wersja generatora 25). Plan do uzgodnienia – kolejność kroków na końcu, decyzje do podjęcia w sekcji „Pytania”.

## 1. Pomiar – jak jest teraz

6 map z domyślnymi parametrami (seed 1–6), 10 443 prowincje. Pomiar: tymczasowy test w `mapgen/src/resources.rs` (do zastąpienia stałym raportem – krok A1).

### Udział surowców w prowincjach

| Grupa | Surowce (udział prowincji) | Razem |
|---|---|---|
| **Jedzenie** | Zboża 10,3 · Bydło 8,5 · Kukurydza 4,4 · Trzoda 4,1 · Ziemniaki 3,4 · Drób 3,1 · Ryby 3,1 · Ryż 2,3 | **39%** |
| Włókna | Bawełna 4,2 · Włókna roślinne 1,8 · Wełna 1,6 | 7,6% |
| Luksus | Tytoń 3,2 · Trzcina 2,8 · Futra 1,5 · Kawa 0,7 · Herbata 0,7 | 8,9% |
| Las | Drewno 8,2 | 8,2% |
| Energia | Ropa 3,5 · Węgiel brunatny 2,5 · Węgiel kamienny 2,3 · Uran 0,8 | 9,1% |
| Metale | Żelazo 3,7 · Mangan 1,6 · Miedź 1,5 · Chrom 1,3 · Ołów 1,1 · Cynk 1,1 · Cyna 1,0 · Nikiel 0,7 · Aluminium 0,5 · Wolfram 0,5 · Molibden **0,1** · Złoto 0,07 | 13,1% |
| Chemia | Nawozy 3,8 · Cement 2,5 · Saletra 2,0 · Siarka 0,7 | 9,0% |
| Inne | Kauczuk 2,0 · Konie 2,8 | 4,8% |

### Biomy (udział prowincji i najczęstsze surowce)

| Biom | Prowincji | Najczęstsze |
|---|---|---|
| Subtropikalny | **19%** | zboża 21%, bydło 14%, kukurydza 9%, tytoń 7% |
| Sawanna | 11% | bydło 20%, bawełna 17%, kukurydza 10% |
| Pustynia | 10% | nawozy 26%, ropa 25%, saletra 14% |
| Borealny | 10% | drewno 25%, trzoda 13%, ziemniaki 10% |
| Las deszczowy | 7% | kauczuk 26%, trzcina 21%, ryż 7% |
| Step, oceaniczny, gorące lato, ciepłe lato, śródziemnomorski | po 7–8% | zboża/bydło 16–23% każdy |
| Tajga | 6% | drewno 35%, futra 11% |
| Tundra, lądolód | **0** | nie ma w nich prowincji (lądolód/lodowiec niczyj) |

### Popyt – ile czego potrzeba

Na przykładzie państwa z ~10 prowincjami: populacja 150 000, w tym 50 000 wojska, z czego 30 000 walczy.

| Zaopatrzenie | Zużycie dzisiaj | Na dzień | Poziomy fabryk (1/d) |
|---|---|---|---|
| Prowiant | 1 / 50 000 osób | 3 | 3 |
| Podstawowe cywilne | 1 / 25 000 osób | **6** | 6 |
| Luksusowe | 1 / 50 000 osób | 3 | 3 |
| Wojskowe (walka) | 1 / 1 000 walczących | **30** | 30 |
| Materiały | budowa 10–60 za poziom | ~2–5 | 2–5 |

Wnioski:
1. **Jedzenia jest za dużo** – 39% prowincji, a potrzeba go najmniej (fabryka prowiantu karmi 50 000, poziom pól daje 1,1/d, a państwo ma ~4 prowincje z jedzeniem już przy 10).
2. **Proporcje potrzeb są odwrócone** – podstawowe cywilne (ubrania, opał) zużywa się 2× szybciej niż jedzenie; powinno być odwrotnie.
3. **Walka zjada zaopatrzenie wojskowe 10× za szybko** – 30 poziomów fabryk na jedną armię to nierealne; dziś walczy się praktycznie bez zaopatrzenia (siła 0,25).
4. **Rzadkie metale są martwe** – molibden (0,1%), wolfram, aluminium, nikiel (≤ 0,7%) – ciężkie zaopatrzenie prawie nieosiągalne bez rynku.
5. **Uran nie ma zastosowania** (0,8% prowincji „pustych” do etapu broni jądrowej).
6. **Biomy są mało różne gospodarczo** – w pięciu biomach umiarkowanych dominuje to samo (zboża + bydło); mało powodów do handlu i ekspansji w konkretny kierunek.

## 2. Cele balansu

- Każda **potrzeba** ma swój udział w mapie, proporcjonalny do zużycia (jedzenie ≈ 18–20%, włókna ≈ 8%, drewno ≈ 6%, energia ≈ 11%, metale ≈ 25%, chemia ≈ 9%, luksus ≈ 9%, inne ≈ 5%).
- Każdy **biom ma tożsamość**: 2–3 mocne surowce i wyraźne braki → kierunek ekspansji i powód do handlu.
- Żaden surowiec **poniżej ~0,4%** prowincji (inaczej nie istnieje w grze) i żaden **bez zastosowania**.
- Jedna gospodarka wspiera **jedną średnią armię w walce** bez rynku; duża wojna wymaga handlu albo podboju.
- Liczby sprawdzalne narzędziem (krok A1, A2), nie „na oko”.

## 3. Proponowane zmiany

### A. Narzędzia (najpierw – bez nich balans to zgadywanie)

1. **Raport rozkładu** – `cargo run -p game-mapgen --features cli -- --stats` (albo test `#[ignore]`): udział surowców i grup na N mapach, per biom. Do README – jak hashe.
2. **Symulator bilansu** – `tools/balance/` (node albo test Rust): wzorcowe państwo (N prowincji wylosowanych z rozkładu, populacja, armia), produkcja przy X poziomach budynków vs zużycie → tabela nadwyżek i braków na dzień. Tym stroimy stałe z kroku C.
3. **Cel udziałów w `chances.mjs`** – skrypt wypisuje oczekiwany udział każdej grupy przy obecnych częstościach biomów i ostrzega, gdy grupa wychodzi poza cel.

### B. Rozkład surowców (generator – `tools/resources/chances.mjs`, `GENERATOR_VERSION`)

1. **Jedzenie 39% → ~19%**: obniżyć dominanty zbóż i bydła w biomach umiarkowanych i na sawannie (100 → 50–60), `BIO_BASE` dla jedzenia mniejszy; jedzenie ma być **wszędzie po trochu**, ale nie dominować.
2. **Metale 13% → ~25%**: więcej w wyżynach i górach (`ORE`), rzadkie metale ×2–3 (molibden ≥ 0,4%), każdy rzadki metal z „ojczyzną” (wolfram, molibden – tajga i góry polarne; nikiel – borealny; aluminium, cyna – tropiki; chrom – sawanna, pustynia).
3. **Tożsamość biomów** (propozycja):

| Biom | Mocne | Braki |
|---|---|---|
| Las deszczowy | kauczuk, trzcina, aluminium, cyna | zboża, węgiel, żelazo |
| Sawanna | bawełna, bydło, mangan, chrom | drewno, węgiel |
| Pustynia | ropa, nawozy, saletra, miedź | jedzenie, drewno, włókna |
| Step | konie, bydło, wełna, miedź | drewno, ryby |
| Śródziemnomorski | tytoń, cement, ołów, ryby | węgiel, drewno |
| Subtropikalny | ryż, herbata, kukurydza, żelazo | ropa |
| Oceaniczny | ryby, wełna, węgiel kamienny, cynk | ropa, metale rzadkie |
| Gorące lato | kukurydza, trzoda, żelazo, węgiel kamienny | luksus |
| Ciepłe lato | zboża, ziemniaki, węgiel brunatny, mangan | luksus, ropa |
| Borealny | drewno, nikiel, węgiel brunatny, ryby | jedzenie z pól |
| Tajga | drewno, futra, wolfram, molibden | jedzenie |

4. **Biom subtropikalny ma 19% prowincji** – do sprawdzenia, czy to efekt parametrów klimatu (`biomeVariants`), czy zamierzone; jeśli nie – wyrównać w generatorze biomów (osobny krok, zmienia mapy bardziej).
5. **Tundra bez prowincji** – sprawdzić, czy to zamierzone (wszystko pod lodowcem/lądolodem); jeśli ma być grywalna – ryby, futra, węgiel brunatny, uran.

### C. Receptury i zużycie (rdzeń – `factories.rs`, `economy.rs`, `war.rs`)

1. **Prowiant z dwóch składników**: 0,7 skrobi (zboża | ryż | kukurydza | 1,0 ziemniaki) + 0,3 białka (bydło | trzoda | drób | ryby). Państwo potrzebuje i pól, i hodowli/łowisk → różnorodność, handel. (Alternatywa: zostawić jeden składnik, tylko zmniejszyć udział jedzenia na mapie.)
2. **Zużycie przez ludność – kolejność potrzeb**: prowiant **1 / 25 000** osób (najważniejszy), podstawowe cywilne **1 / 50 000**, luksusowe **1 / 100 000**. Zapasy startowe dopasowane (≈ 2 miesiące).
3. **Zaopatrzenie wojskowe w walce: 1 → 0,1 sztuki na 1 000 walczących dziennie** (armia 30 000 = 3/dzień = 3 poziomy fabryki). Do sprawdzenia symulatorem razem z siłą 0,25 bez zaopatrzenia.
4. **Uran**: do etapu broni jądrowej nie ma zastosowania. Propozycja: **usunąć z mapy do tego etapu** (te prowincje dostaną inny surowiec) albo zostawić rzadziej (0,3%) jako surowiec tylko na eksport (rynek).
5. **Każdy surowiec w ≥ 1 recepturze** – dziś bez użycia: uran; słabo używane: kawa, herbata, futra (tylko luksus), kauczuk, aluminium, molibden, wolfram (tylko ciężkie). Propozycje: kauczuk także w materiałach inżynieryjnych (opony, uszczelki – zamiennik), aluminium w standardowym (zamiennik manganu), cyna w amunicji (zamiennik ołowiu – już w GDD jako próg uniwersytetu).
6. **Materiały inżynieryjne**: 4 składniki (cement, drewno, żelazo, paliwo|konie) – zostaje; koszt budynków porównać symulatorem z produkcją (ile minut gry na poziom miasta).

### D. Usunąć / dodać – do decyzji

| Propozycja | Za | Przeciw |
|---|---|---|
| Usunąć **uran** z mapy do etapu broni jądrowej | brak martwych prowincji | trzeba przywrócić później (nowa wersja generatora) |
| Połączyć **kawę i herbatę** w „Używki” | obie po 0,7%, ta sama rola | mniej klimatu dwudziestolecia |
| Połączyć **molibden z wolframem** | molibden praktycznie nie istnieje | jeden rzadki metal mniej |
| Dodać **sól** (konserwacja prowiantu, chemia) | ważny surowiec epoki, nowy składnik prowiantu | kolejny surowiec do zbalansowania |
| Dodać **boksyt** jako nazwę dla aluminium | czytelniej (ruda, nie metal) | tylko kosmetyka |

## 4. Kolejność (osobne kroki, każdy z testami i zrzutami)

1. **A1–A3 narzędzia** – raport rozkładu, symulator bilansu, cele grup w `chances.mjs`. Bez zmian w grze.
2. **C2–C3 zużycie** (rdzeń, bez nowej wersji generatora) – kolejność potrzeb ludności, zaopatrzenie wojskowe w walce; sprawdzenie symulatorem.
3. **B1–B3 rozkład surowców** (nowa wersja generatora, nowy hash zasobów) – udziały grup, tożsamość biomów, rzadkie metale.
4. **C1, C4–C6 receptury** – prowiant z dwóch składników, zastosowania dla każdego surowca, decyzje z D.
5. **B4–B5 biomy** (jeśli potrzebne) – częstości biomów, tundra.
6. Gra testowa (2 graczy) i strojenie stałych z symulatora.

## 5. Pytania

1. Prowiant z **dwóch składników** (skrobia + białko) czy jeden składnik i tylko mniej jedzenia na mapie?
2. **Uran**: usunąć z mapy do broni jądrowej czy zostawić na eksport?
3. Łączyć kawę z herbatą i molibden z wolframem – tak/nie? Dodać sól?
4. Czy biomy subtropikalne (19%) i brak tundry to zamierzony efekt parametrów klimatu, czy do poprawy?
