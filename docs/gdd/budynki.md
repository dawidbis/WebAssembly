# Budynki

Budynki są częścią prowincji: w jednej prowincji może stać najwyżej **jeden budynek danego typu**, rozwijany **poziomami**. Na mapie mają **stałe miejsca** (kafel) – okrągłe ikony w kolorze rodzaju z poziomem, kliknięcie otwiera okienko budynku (fabryka – panel produkcji); trwająca budowa to ikona z kołowym paskiem postępu. Miejsca to stała cecha mapy – liczy je generator (`mapgen/src/slots.rs`, `Province::slots`): miasto w głębi prowincji blisko środka, port przy morzu, wydobycie na pasującym terenie, fabryka w głębi prowincji. **Uniwersytet stoi w mieście** (czapka obok ikony miasta; okienko miasta pokazuje uniwersytet i jego obsadę) – bez miasta nie da się go zbudować. **Infrastruktura i umocnienia nie mają miejsca** – działają na całą prowincję; centrum dowodzenia to gwiazdki nad miastem stolicy (1–2 brązowe, 3–4 srebrne, 5–6 złote).

## Kontrola budynków ✅

- Budynek **kontroluje państwo, które ma jego kafel** (`Game::controller`) – niezależnie od udziału w prowincji. Tylko ono buduje go i anuluje budowę, ustawia fabrykę i jej wejścia, dostaje produkcję i limit populacji z miasta i płaci za obsadę. Wydobycie bazowe 0,10/dzień (bez budynku) też idzie do właściciela kafla wydobycia. Centrum dowodzenia – kto ma kafel miasta.
- **Infrastruktura i umocnienia** (bez kafla) – właściciel prowincji: gracz z **ponad 50% kafli**.
- Budynek, którego kafla nikt nie ma, **stoi** – nie produkuje, a trwająca budowa jest wstrzymana.
- **Budynki nigdy nie tracą poziomu** – zajęcie kafla przejmuje budynek ze wszystkimi poziomami.

## Typy budynków

| Budynek | Gdzie | Maks. poziom | Działanie |
|---|---|---|---|
| **Wydobycie** (typ z surowca: kopalnia, pola uprawne…) | każda prowincja | 10 | +1,0 surowca/dzień za poziom ([surowce.md](surowce.md)) |
| **Fabryka** | każda prowincja | 10 | 1,0 szt. zaopatrzenia/dzień za poziom, jeden rodzaj naraz ([zaopatrzenie.md](zaopatrzenie.md#fabryki)) |
| **Miasto** | każda prowincja | 5 | +10 000 do limitu populacji (globalnie) za poziom 1, kolejne więcej (+12 500, +17 500, +25 000, +35 000) |
| **Uniwersytet** | prowincja z miastem (stoi w mieście) | 5 | poziomy sumują się w państwie → progi technologii (niżej) |
| **Umocnienia** | każda prowincja | 3 | ✅ obrona kafli prowincji +50% za poziom (koszt kafla i tempo frontu atakującego) – dla tego, kto ma ponad połowę prowincji (zdobyte bronią zdobywcy): poziom 1 od centrum dowodzenia 2, 2 od CD 3, 3 od CD 5 |
| **Port** | prowincja nad morzem | 3 | baza desantów morskich ✅ (wojska na desant według poziomu: 5 000 / 15 000 / 40 000; desant zawsze tworzy przyczółek); handel morski ✅ – przepustowość handlu (5 na poziom × obsada) i jedyna droga do handlarzy spoza mapy – przepustowość i niższy koszt dostawy ([rynek.md](rynek.md)) |
| **Centrum dowodzenia** | tylko stolica | 6 | odblokowania (niżej) |
| **Infrastruktura** | każda prowincja | 5 | punkty infrastruktury na budynki (niżej), większa armia tubylcza; ⏳ tańszy transport lądem |

Maksymalne poziomy *(do strojenia)*. Ile poziomów budynków zmieści się w prowincji, ogranicza jej **infrastruktura**.

## Infrastruktura

- Każda prowincja ma **poziom infrastruktury** (1–5), a z nim **punkty infrastruktury**: poziom 1 – **100 punktów**, każdy kolejny **+50** (2 – 150, 3 – 200, 4 – 250, 5 – 300) *(do strojenia)*.
- **Każdy poziom budynku zajmuje punkty** – suma nie może przekroczyć punktów prowincji *(do strojenia)*:

| Budynek | Punktów za poziom |
|---|---|
| Wydobycie | 10 |
| Fabryka | 20 |
| Miasto | 25 |
| Uniwersytet | 15 |
| Port | 15 |
| Umocnienia | 10 |
| Centrum dowodzenia | 0 (stolica ma je zawsze) |

  Przykład: na 100 punktach zmieści się np. wydobycie 3 + fabryka 2 + miasto 1 (30 + 40 + 25 = 95).
- **Poziom startowy** (stała właściwość prowincji z mapy, liczony w rdzeniu z cech prowincji):
  - bazowo **2**;
  - **−1** za trudny biom: pustynia, lądolód, tundra, las deszczowy (dżungla);
  - **−1** za przewagę wyżyn albo sąsiedztwo gór;
  - **+1** za rzekę albo morze (szlaki wodne), najwyżej do 3;
  - wynik przycięty do 1–3.
- Rozbudowa infrastruktury to zwykła budowa (koszt jak niżej, nie zajmuje punktów). Wyższa infrastruktura **obniża koszt transportu lądem** przez prowincję ([rynek.md](rynek.md#koszt-dostawy)).
- Infrastruktura, jak inne budynki, **nie traci poziomu** przy zmianie właściciela.

## Budowa

- Intencja `Build { province, building }` – rdzeń sprawdza: gracz kontroluje budynek (kafel budynku; infrastruktura i umocnienia – > 50% kafli prowincji), poziom poniżej maksimum, wolne punkty infrastruktury, odblokowanie (centrum dowodzenia), złoto i **materiały inżynieryjne** w puli.
- **Koszt** = (złoto, materiały) bazowe × **1,5^(poziom − 1)** *(do strojenia)*. Złoto – z interfejsu; materiały inżynieryjne – zaopatrzenie z fabryk ([zaopatrzenie.md](zaopatrzenie.md)):

| Budynek | Złoto (poziom 1) | Materiały (poziom 1) |
|---|---|---|
| Wydobycie | 10 000 | 10 (poziom 1: 0; pola, plantacje, rancza i łowiska – zawsze 0) |
| Fabryka | 20 000 | 20 (poziom 1: 0) |
| Miasto | 40 000 | 40 |
| Uniwersytet | 30 000 | 30 |
| Umocnienia | 15 000 | 30 |
| Port | 20 000 | 25 |
| Infrastruktura | 15 000 | 30 |
| Centrum dowodzenia | poziom 2: 30 000, każdy kolejny ×2 | poziom 2: 30, każdy kolejny ×2 |

- **Start bez blokady**: pierwszy poziom wydobycia i fabryki kosztuje **tylko złoto** (pola, plantacje, rancza i łowiska – każdy poziom), a każdy gracz zaczyna z zapasem **40 materiałów inżynieryjnych** (do tego 60 paliwa, 200 podstawowego cywilnego i 300 prowiantu; soli i innych surowców na start nie ma) – można postawić fabrykę materiałów, zanim zabraknie ich na resztę.

  Na start gracz ma **30 000 złota** (`economy::START_GOLD`) i **miasto poziomu 1** w stolicy (obok centrum dowodzenia). Dla porównania: na starcie gracz ma ok. 12 500 robotników, czyli ok. 1 250 złota podatku na miesiąc przy pełnym podstawowym zaopatrzeniu cywilnym (0,1 od robotnika; bez niego połowa) – pierwszy budynek wydobycia to ok. 8 miesięcy gry (4 minuty) oszczędzania albo dużo szybciej z kopalnią złota (poziom 1 – ok. 4 500 złota na miesiąc) / handlem. Budynki są dużą inwestycją, a wczesna gra opiera się na wydobyciu bazowym.
- **Czas budowy** = **10 + 5 × poziom dni** *(do strojenia)* – poziom 1 ok. 15 s, poziom 10 ok. 1 min. Złoto pobierane na początku budowy.
- **Jedna budowa na prowincję naraz**; wiele prowincji może budować równolegle. Anulowanie zwraca 50% kosztu.

## Obsada

Budynki pracują dzięki **robotnikom** (suwak wojsko/robotnicy w pasku zasobów):

| Budynek | Robotników na poziom |
|---|---|
| Wydobycie | 10 000 |
| Fabryka | 20 000 |
| Uniwersytet | 15 000 |
| Port | 5 000 |
| Miasto, umocnienia, infrastruktura, centrum dowodzenia | 0 |

- **Pokrycie obsady** = min(1, robotnicy / suma wymaganej obsady wszystkich budynków państwa) i **mnoży całą produkcję** (wydobycie, fabryki, uniwersytety) oraz **przepustowość portów** (pojemność handlu morskiego, [rynek.md](rynek.md#pojemność-handlu)). Bez wybierania, który budynek staje – prosto i deterministycznie.
- Wydobycie bazowe 0,10/dzień w prowincji bez budynku nie wymaga obsady.
- Konsekwencja: przesunięcie suwaka w stronę wojska zmniejsza produkcję i podatki – realny wybór między armią a gospodarką. Przy limicie ok. 60 000 osób na start (połowa to robotnicy) państwo utrzyma w pełni ok. jednego budynku wydobycia i jednej fabryki – więcej wymaga ekspansji albo miast.

## Stolica i centrum dowodzenia

- **Stolica** to prowincja startowa; stoi w niej **centrum dowodzenia** (poziom 1 od początku gry) ✅.
- ✅ **Zmiana stolicy** (okno prowincji – „Przenieś tutaj”): do prowincji, w której gracz ma ponad połowę kafli i kafel miasta, za **20 000 złota × (liczba dotychczasowych zmian + 1)**, nie częściej niż **raz na rok gry (6 min)** *(do strojenia; `buildings::CAPITAL_MOVE_*`)*. Centrum dowodzenia przenosi się razem ze stolicą i **zachowuje poziom**; nie w trakcie jego rozbudowy.
- ✅ **Utrata stolicy = przegrana gracza** – **aneksja**, gdy wróg zajmie **kafel centrum dowodzenia** (miejsce miasta w stolicy): zdobywca przejmuje całe pozostałe terytorium pokonanego (z budynkami), **połowę jego złota**, **25% jego populacji** (wojsko i robotnicy w kraju – dopisane do zdobywcy) i **30% każdego surowca i zaopatrzenia z magazynu**. Wojsko pokonanego na frontach przepada, centrum dowodzenia jest zniszczone, reszta zapasów znika. Pokonany ogląda dalej grę; gdy zostaje jedno państwo – zwycięstwo (`war::ANNEX_*`, `Game::annex`).

**Front** to jeden atak na jedną albo kilka prowincji naraz (zaznaczanie z Ctrl); zaczyna się z każdej naszej prowincji, która styka się z celami ([zaopatrzenie.md](zaopatrzenie.md#wojsko-w-walce)). Odblokowania ✅ (stałe w `core/src/war.rs`, `buildings::FORTIFICATION_CAP`, `factories::COMMAND_LEVEL`):

| Poziom | Fronty naraz | Prowincji na froncie | Odblokowuje |
|---|---|---|---|
| 1 | 2 | 1 | lekkie zaopatrzenie wojskowe; przeprawy przez rzeki (do 4 kafli); desant przez jeziora; desant morski z portu (1 na morzu naraz, wielkość według poziomu portu, zawsze przyczółek) |
| 2 | 2 | 2 | standardowe zaopatrzenie wojskowe; desant do 5 000 wojska; umocnienia do poziomu 1; przeprawy: rzeki do 30 tys., góry do 12 tys. wojska |
| 3 | 3 | 3 | desanty morskie bez limitu (do 15 000 wojska); umocnienia do poziomu 2; lekkie zaopatrzenie: siła 0,6 → 0,8 i 40% mniej paliwa |
| 4 | bez limitu | 4 | ciężkie zaopatrzenie wojskowe; desant do 30 000; standardowe: siła 1,0 → 1,2 i 40% mniej paliwa |
| 5 | bez limitu | bez limitu | umocnienia do poziomu 3; desant do 60 000 |
| 6 | bez limitu | bez limitu | ciężkie zaopatrzenie: siła 1,4 → 1,6 i 40% mniej paliwa; desant do 120 000; ⏳ broń jądrowa |

Koszt: poziom 2 – 30 000 złota i 30 materiałów, każdy kolejny ×2.

## Miasto ✅

+10 000 do **limitu populacji państwa** za poziom 1, każdy kolejny więcej – `LEVEL_BONUS_PCT` (globalnie – dodaje się do limitu 10 osób na kafel). Nie wymaga obsady. Większy limit = szybszy przyrost i więcej robotników, ale też większe zużycie zaopatrzenia cywilnego.

## Uniwersytet

Poziomy uniwersytetów w całym państwie się **sumują**; przekroczenie progu odblokowuje technologię *(progi do strojenia)*. W kodzie: progi 5, 10 i 20 ✅, reszta ⏳.

| Suma poziomów | Technologia |
|---|---|
| 5 | fabryki +10% produkcji |
| 10 | paliwo syntetyczne z węgla (2,0 Węgiel → 1 Paliwo) |
| 15 | kauczuk syntetyczny z ropy (1,0 Ropa → 1 Kauczuk – w fabryce) |
| 20 | fabryki +20% (łącznie) |
| 25 | dodatkowe zamienniki w recepturach (np. aluminium zamiast chromu w ciężkim) |
| 40 | wzbogacanie uranu *(w przyszłości)* |

## Tempo gry: poziomy, wojsko i koszary ✅

Zmiana tempa (prośba użytkownika – gra była za szybka, miasta za mało warte). Stałe do strojenia w kodzie, wartości poniżej to stan obecny.

- **Ludność:** kafel daje **10** osób limitu (`economy::PEOPLE_PER_TILE`), miasto **+10 000** za poziom 1. Mniej ludzi – proporcjonalnie mniej obsady (`buildings::STAFF_PER_LEVEL`: wydobycie 2 000, fabryka 4 000, uniwersytet 3 000, port 1 000 na poziom), tubylców (`war::NATIVES_PER_TILE` 1, `MIN_DEFENSE` 2), wojska na desancie i przeprawie (÷10). Podatek od robotnika do **0,5** złota na miesiąc (`TAX_PER_1000_WORKERS`).
- **Rosnące bonusy poziomów** (`buildings::LEVEL_BONUS_PCT` – % bazowego bonusu): 100, 125, 175, 250, 350, 475, 625, 800, 1000, 1225. Dotyczy miasta (+10k, +12,5k, +17,5k, +25k, +35k), wydobycia, produkcji fabryk, przepustowości portów i koszar; obsada rośnie liniowo – wyższe poziomy się opłacają.
- **Koszt poziomów:** poziom 1 – **150%** bazowego złota (`FIRST_GOLD_PCT`; wydobycie i fabryka bez materiałów), kolejne – **50%** bazy rosnąco o 15% za poziom w złocie (`UPGRADE_GOLD_PCT`, `GOLD_GROWTH_PCT`), za to materiały **×1,6** za poziom (`MATERIALS_GROWTH_PCT`).
- **Utrzymanie wojska** (co miesiąc z podatkiem, `Economy::upkeep`): **0,1** złota od żołnierza (w kraju i na frontach), ponad **miękkim limitem** dochodzi 2 × nadwyżka² / limit (przy 2× limitu każdy kolejny żołnierz kosztuje 5×). Bez złota – płaci, ile ma (bez dezercji). Księga złota: „Utrzymanie wojska”.
- **Miękki limit wojska:** 5 000 + 3 na kafel państwa + **koszary** (`Building::Barracks`, od CD 2, bez miejsca na mapie): +5 000 za poziom 1, kolejne rosnąco (+6 250, +8 750, +12 500, +17 500).

## Otwarte pytania

- Limit miast rozwiązuje infrastruktura (miasto zajmuje 25 punktów na poziom) – do sprawdzenia, czy to wystarczy.
- Czy przy tak drogich budynkach (×10) i dużej obsadzie start nie jest zbyt wolny – do sprawdzenia w pierwszych grach.
