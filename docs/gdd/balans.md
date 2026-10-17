# Plan: balans surowców, receptur i gospodarki

Stan na 2026-10-17 (wersja generatora 26). Kroki A, B, C zrobione – wyniki w rozdziałach 8 i 9; dalej: gra testowa i rynek.

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

## 5. Decyzje (2026-10-17)

1. **Prowiant z dwóch składników** (skrobia + białko) – i mimo to **mniej jedzenia na mapie** (cel ~19%).
2. **Uran – usunąć z mapy** do etapu broni jądrowej.
3. Kawa, herbata, molibden, wolfram – **bez łączenia**; kawa i herbata rzadziej, tak żeby **razem** miały tyle, ile miała jedna (≈ 0,7%). Soli nie dodajemy.
4. **Gwarancja biomów**: tropikalny, pustynny i tundra na każdej mapie – choćby mała wyspa, mały kontynent albo fragment innego kontynentu.
5. Metale do usunięcia – propozycja niżej (do potwierdzenia).
6. Chemia i materiały inżynieryjne – propozycja niżej (do potwierdzenia).
7. **Łańcuch zaopatrzenia wojskowego**: każdy wyższy rodzaj wymaga niższego jako półproduktu.

## 6. Propozycje do potwierdzenia

### Metale (punkt 5)

Rola dziś: żelazo – wszędzie; mangan, chrom, nikiel – stopy do standardowego (wymienne); chrom, nikiel, wolfram, molibden – ciężkie (wymienne); ołów, cynk – amunicja (wymienne); miedź – lekkie (zamiast żelaza) i luksus; cyna – luksus; aluminium – ciężkie; złoto – skarbiec.

- **Usunąć molibden** – ta sama rola co wolfram, chrom i nikiel w ciężkim, a na mapie praktycznie go nie ma (0,1%).
- **Usunąć cynk** – dokładnie ta sama rola co ołów (amunicja), obie po ~1%.
- Zostaje 10 metali: żelazo, mangan, chrom, nikiel, wolfram, miedź, ołów, cyna, aluminium, złoto. Każdy z wyraźną rolą: stal (żelazo, mangan), pancerz (chrom, nikiel), pociski przeciwpancerne (wolfram), amunicja (ołów, cyna jako zamiennik), łuski i luksus (miedź), lotnictwo (aluminium).
- Udział metali w mapie w górę (cel ~25%), rzadkie metale (wolfram, aluminium, nikiel) ≥ 0,6% każdy, z „ojczyzną” w biomie.

### Chemia i materiały inżynieryjne (punkt 6)

- **Cement (2,5%) to wąskie gardło**: jedyny niewymienny składnik materiałów, a materiały są potrzebne do każdego budynku poza 1. poziomem wydobycia i fabryki. Gracz bez cementu stoi do podboju albo rynku. Propozycja: **cement częściej (~4%), w wielu biomach** (wapień jest pospolity) i **zamiennik w materiałach: 0,8 Drewna** (budownictwo drewniane – gorsze, ale możliwe).
- **Nawozy (3,8%) mają jedno zastosowanie** (premia pól). Propozycja: **rzadziej (~2,5%)** i drugie zastosowanie – **zamiennik saletry** w zaopatrzeniu wojskowym (azotany; 1,5 raza więcej).
- Saletra (2,0%) i siarka (0,7%) – materiały wybuchowe, wymienne; siarka do ~1%.
- Materiały: 0,5 Cement \| 0,8 Drewno + 0,3 Drewno + 0,2 Żelazo + 0,1 Paliwo \| 0,2 Konie – koszt budynków sprawdzić symulatorem (ile minut gry na poziom miasta).

### Łańcuch wojskowy (punkt 7)

| Rodzaj | Receptura na 1 szt. (propozycja) |
|---|---|
| Lekkie | 0,5 Żelazo \| Miedź + 0,3 Bawełna \| Wełna \| Włókna + 0,2 Saletra \| Siarka \| 0,3 Nawozy + 0,1 Ołów \| Cyna |
| Standardowe | **0,5 Lekkie** + 0,4 Żelazo + 0,2 Mangan \| Chrom \| Nikiel + 0,2 Saletra \| Siarka \| 0,3 Nawozy + 0,2 Węgiel kamienny \| 0,3 brunatny |
| Ciężkie | **0,5 Standardowe** + 0,5 Żelazo + 0,2 Chrom \| Nikiel \| Wolfram + 0,3 Węgiel kamienny + 0,2 Kauczuk + 0,2 Aluminium |

Skutek: ciężkie wymaga całej linii fabryk (lekkie → standardowe → ciężkie), a nie tylko rzadkiego metalu; „pół sztuki” niższego to dość, żeby łańcuch był odczuwalny, a nie mnożył kosztów ×10.

### Gwarancja biomów (punkt 4)

Po wygenerowaniu biomów: jeśli na mapie nie ma prowincji danego typu (tropikalny – las deszczowy albo sawanna; suchy – pustynia; tundra), generator przemienia najbardziej pasujący kawałek lądu (najcieplejszy / najsuchszy / najzimniejszy region przy lądolodzie, najlepiej osobna wyspa albo skraj kontynentu) w ten biom – na tyle duży, żeby powstało kilka prowincji. Własny RNG (`seed ^ SALT`), test, że mapy z tymi biomami się nie zmieniają. Wersja generatora w górę.

## 7. Ustalenia końcowe (2026-10-17)

- **Metale**: usuwamy molibden i cynk; zostaje 10 metali (żelazo, mangan, chrom, nikiel, wolfram, miedź, ołów, cyna, aluminium, złoto). Uran – usunięty z mapy do etapu broni jądrowej.
- **Cement** częściej (~4%, w wielu biomach), **bez zamiennika z drewna**.
- **Nawozy** rzadziej (~2,5%) i zamiennik saletry w zaopatrzeniu wojskowym (1,5 raza więcej); siarka ~1%.
- **Sól** – nowy surowiec, **niewymienny składnik prowiantu** (0,1 na sztukę). Występuje rzadko (pustynie, suche wybrzeża, góry). Brak soli ma być rozwiązywany handlem – także z prowincjami niczyimi (rynek, etap 8), nie tylko z graczami.
- **Prowiant**: skrobia (zboża | ryż | kukurydza | ziemniaki) + białko (bydło | trzoda | drób | ryby) + 0,1 soli.
- **Luksusowe cywilne**: 0,2 Kawa | Herbata | Tytoń + 0,2 Trzcina cukrowa + 0,1 Futra | Bawełna + 0,1 Miedź | Cyna.
- **Łańcuch wojskowy**: standardowe zawiera 0,5 lekkiego, ciężkie – 0,5 standardowego (tabela w rozdziale 6).
- **Zapasy startowe**: materiały inżynieryjne 100, paliwo 50, prowiant 120 (podstawowe cywilne 100).
- **Kawa i herbata** zostają, razem ~0,7% prowincji.
- **Gwarancja biomów**: tropikalny, pustynny i tundra na każdej mapie.

## 8. Narzędzia i pierwsze wyniki (krok A, C2)

- **Raport rozkładu**: `cargo run -p game-mapgen --release --features cli -- --stats 6 --seed 1` – udział grup i surowców, biomy (udział, na ilu mapach ich brak, najczęstsze surowce).
- **Symulator bilansu**: `cargo run -p game-core --release --example balance -- --maps 2 --size 10 --level 2` – losowe spójne państwa z prawdziwych map, wydobycie przy danym poziomie, ile zaopatrzenia wychodzi z receptur wobec popytu ludności i armii, najczęstsze braki, nadwyżki surowców.

Wyniki (6 map, przed rebalansem):

- **Biomy**: tundry nie ma na żadnej z 6 map, tajgi na 5/6, pustyni i lasu deszczowego na 2/6 – gwarancja biomów potrzebna.
- **Skala zużycia była kilkukrotnie za duża** wobec produkcji (prowincja przy limicie ma ~48 000 ludzi, a wydobywa ~2/d przy poziomie 2). Zmienione (C2): prowiant **1 / 100 000** osób, podstawowe **1 / 150 000**, luksusowe **1 / 300 000**, zaopatrzenie wojskowe w walce **0,05** na 1 000 walczących dziennie. Prowiant: 79–82% państw (10–20 prowincji) samowystarczalnych.
- **Receptury z 4–5 niewymiennymi składnikami** (podstawowe, materiały, luksus, wojskowe) są samowystarczalne tylko w kilku % państw – każdy składnik ma ~20% państw. To zadanie dla rozkładu (krok B: metale ~25%, cement ~4%) i **rynku** (etap 8, także handel z prowincjami niczyimi). Bez rynku większość zaopatrzenia będzie zależała od zapasów startowych i podboju.
- Zapasy startowe (ustalenia): materiały 100, paliwo 50, podstawowe 100, prowiant 120.

## 9. Rozkład i receptury (kroki B i C – zrobione, generator 26)

- **Surowce**: usunięte uran, molibden i cynk, dodana sól – 37 zasobów, ułożonych grupami (numery w Ruście: `resources::id::*`, w TS grupy i kategorie po nazwach). Tożsamość biomów – zasoby dominujące w `tools/resources/chances.mjs` (tabela w [surowce.md](surowce.md#rozkład-na-mapie)).
- **Kalibracja**: `node tools/resources/chances.mjs --calibrate N` – udział każdego surowca dobierany do celu (`TARGET` w skrypcie) przez skale w `tools/resources/scale.json`. Wynik (6 map): jedzenie 19,6%, włókna 8,9%, luksus 7,5% (kawa 0,30 + herbata 0,44), drewno 7,5%, energia 13,4%, metale 26,4% (wolfram 1,25, aluminium 1,46, nikiel 1,74), chemia 11,6% (cement 4,85, nawozy 2,49, siarka 1,02, sól 1,01).
- **Gwarancja biomów** (`biome_guarantee`, domyślnie włączona): tundra, pustynia i biom tropikalny na każdej mapie (test na 8 mapach). Bez losowania – mapy, które je mają, są identyczne jak bez gwarancji. Przy okazji: polarny wariant „tajga + tundra” bez lądolodu oddaje udział lądolodu tajdze (tundra zostaje pasem, zamiast zajmować 70% obszaru). Tundra to teraz ~10% prowincji (wcześniej prawie zawsze 0).
- **Receptury** (`factories.rs`, [zaopatrzenie.md](zaopatrzenie.md)): prowiant = skrobia + białko + 0,1 soli; luksus = 0,2 Kawa | Herbata | Tytoń + 0,2 Trzcina + 0,1 Futra | Bawełna + 0,1 Miedź | Cyna; łańcuch wojskowy (0,5 lekkiego w standardowym, 0,5 standardowego w ciężkim); nawozy zamiast saletry (0,3), cyna zamiast ołowiu. Każdy surowiec poza złotem ma zastosowanie (test).

### Symulator po zmianach (3 mapy, wydobycie poziom 2)

| Zaopatrzenie | 10 prowincji | 20 prowincji |
|---|---|---|
| Prowiant | 2% (bez soli byłoby 27%) | 3% |
| Podstawowe cywilne | 2% | 11% |
| Materiały inżynieryjne | 3% | 13% |
| Luksusowe cywilne | 0% | 6% |
| Lekkie wojskowe | 3% | 18% |

Wnioski (przed poprawką niżej):

1. **Sól decyduje o jedzeniu.** Ma ~1% prowincji, więc państwo z 10 prowincjami ma ją w ~10% przypadków. Bez rynku prawie każdy gracz po zużyciu 120 startowego prowiantu (kilka miesięcy gry) zacznie głodować. Do decyzji przed testową grą: zapas soli na start, więcej soli na mapie albo wcześniejszy handel z prowincjami niczyimi.
2. **Skrobia + białko** to razem ~30% samowystarczalnych państw (10 prowincji) – jedzenie przestało być powszechne, zgodnie z celem; poziom wydobycia niewiele zmienia (27–31% przy poziomach 2–4), liczy się posiadanie obu grup.
3. Receptury z 4–6 niewymiennymi składnikami są samowystarczalne w kilku–kilkunastu % państw – zgodnie z planem o reszcie zdecyduje **rynek** (etap 8).

### Poprawka (generator 27)

- **Sól**: zapas na start **30** (`goods::START_SALT`, w puli surowców – 300 sztuk prowiantu) i więcej na mapie (cel 1,6%, wynik 1,62%).
- **Wolfram** rzadziej (cel 0,6%, wynik 0,64%) – udział oddany soli.
- **Ciężkie wojskowe**: zamiast „Chrom | Nikiel | Wolfram” tylko **0,2 Wolframu** (bez zamiennika). Chrom i nikiel zostają w standardowym.
- Handel z prowincjami niczyimi – później (etap rynku).

### Poprawka (generator 28)

- **Tytoń** egzotyczny jak kawa i herbata (cel 0,35%, wynik 0,36%; cena bazowa 30) – udział oddany **soli** (cel 3,75%, wynik 3,69%). Luksus ma teraz ~5,5% prowincji, a składnik „Kawa | Herbata | Tytoń” ~1% – luksusowe cywilne stało się towarem handlowym.
- **Prowiant**: zużycie o połowę mniejsze – **1 sztuka na 200 000 osób** dziennie; na start **150** (plus 30 soli).

### Poprawka – ludność i złoto

- **Przyrost logistyczny** (krzywa S): `4 × szczyt × x × (1 − x)` – mała populacja rośnie wykładniczo, najszybciej przy połowie limitu, potem coraz wolniej dochodzi do limitu.
- **Podatek zależny od podstawowego zaopatrzenia cywilnego**: 0,1 złota od robotnika na miesiąc × 50% (bez podstawowego) do 100% (pełne, średnia z miesiąca) – `economy::BASIC_TAX_MIN_PCT`; luksusowe dokłada do +50% ponad to.
- **Kopalnia złota ×3**: 150 złota za jednostkę (`goods::GOLD_PER_UNIT`).
- **Start**: prowiant 300, podstawowe cywilne 150.
