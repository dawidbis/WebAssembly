# Zaopatrzenie

Zaopatrzenie to dobra wytwarzane w **fabrykach** z surowców z globalnej puli. Dzieli się na **wojskowe** (siła wojska w walce), **paliwo** (szybkość zajmowania) i **cywilne** (wysokość podatków).

## Rodzaje i receptury

Receptura na **1 sztukę** *(do strojenia)*. Składniki rozdzielone „|” są wymienne – fabryka bierze ten, którego gracz ma najwięcej (dzięki temu brak jednego metalu nie blokuje produkcji). **Węgiel brunatny** jest gorszym zamiennikiem węgla kamiennego: wszędzie potrzeba go **1,5 raza więcej** (zapis „0,2 Węgiel kamienny | 0,3 Węgiel brunatny”); w ciężkim zaopatrzeniu (koks hutniczy) zastąpić się go nie da. Składnik „Paliwo” to zaopatrzenie z fabryk paliwa (półprodukt) – fabryki podstawowego cywilnego i materiałów inżynieryjnych biorą je z puli zaopatrzenia. **Konie** są tam gorszym zamiennikiem paliwa (0,1 Paliwo | 0,2 Konie – transport konny), więc gracz bez ropy może budować i zaopatrywać ludność, tylko drożej.

| Rodzaj | Odblokowanie | Receptura na 1 szt. | Do czego |
|---|---|---|---|
| **Lekkie wojskowe** | centrum dowodzenia 1 | 0,5 Rudy żelaza \| Miedź + 0,3 Bawełna \| Wełna \| Włókna roślinne + 0,2 Saletra \| Siarka \| 0,3 Surowce nawozowe + 0,1 Ołów \| Cyna (amunicja) | walka, półprodukt standardowego |
| **Standardowe wojskowe** | centrum dowodzenia 2 | **0,5 Lekkie wojskowe** + 0,4 Rudy żelaza + 0,2 Mangan \| Chrom \| Nikiel + 0,2 Saletra \| Siarka \| 0,3 Surowce nawozowe + 0,2 Węgiel kamienny \| 0,3 Węgiel brunatny | walka, półprodukt ciężkiego |
| **Ciężkie wojskowe** | centrum dowodzenia 4 | **0,5 Standardowe wojskowe** + 0,5 Rudy żelaza + 0,2 Wolfram (bez zamiennika) + 0,3 Węgiel kamienny + 0,2 Kauczuk + 0,2 Aluminium | walka |
| **Paliwo** | od startu | 1,0 Ropa naftowa (uniwersytet: albo 2,0 Węgiel kamienny \| 3,0 Węgiel brunatny – paliwo syntetyczne) | szybkość zajmowania, podstawowe cywilne |
| **Prowiant** | od startu | 0,6 Zboża \| 0,55 Ryż \| 0,6 Kukurydza \| 0,8 Ziemniaki (skrobia) + 0,35 Bydło \| 0,35 Trzoda chlewna \| 0,4 Drób \| 0,4 Ryby (białko) + 0,1 Sól (bez zamiennika) | jedzenie ludności: przyrost, głód |
| **Podstawowe cywilne** | od startu | 0,3 Bawełna \| Wełna \| Włókna roślinne + 0,2 Drewno + 0,2 Węgiel kamienny \| 0,3 Węgiel brunatny (ogrzewanie) + 0,1 Paliwo \| 0,2 Konie (transport, oświetlenie) | warunek premii z luksusowego |
| **Luksusowe cywilne** | od startu | 0,2 Kawa \| Herbata \| Tytoń + 0,2 Trzcina cukrowa + 0,1 Futra \| Bawełna + 0,1 Miedź \| Cyna | podatki (mocniej) |
| **Materiały inżynieryjne** | od startu | 0,5 Cement + 0,3 Drewno + 0,2 Rudy żelaza + 0,1 Paliwo \| 0,2 Konie (maszyny i transport) | budowa i rozbudowa wszystkich budynków, ⏳ mosty i tunele (od CD 2) |
| *Wzbogacony uran* (później – uran wróci wtedy na mapę) | centrum dowodzenia 6 + uniwersytet | 2,0 Uran + 1,0 Paliwo | broń jądrowa |

**Materiały inżynieryjne** są potrzebne (obok złota) do **budowy i rozbudowy wszystkich budynków** oraz mostów i tuneli – cement, drewno i żelazo stają się podstawą rozwoju. Żeby start się nie zablokował, pierwszy poziom wydobycia i fabryki kosztuje tylko złoto, a każdy gracz zaczyna ze 100 materiałami ([budynki.md](budynki.md#budowa)).

**Łańcuch wojskowy**: standardowe zawiera pół sztuki lekkiego, ciężkie – pół sztuki standardowego (półprodukty z puli zaopatrzenia, jak paliwo). Ciężkie wymaga więc całej linii fabryk, a nie tylko rzadkiego metalu. **Nawozy** są zamiennikiem saletry (azotany, 1,5 raza więcej), **cyna** – ołowiu w amunicji.

## Fabryki

- Fabryka stoi w dowolnej prowincji gracza (położenie nie ma znaczenia – pula jest globalna); w prowincji najwyżej jedna fabryka, rozwijana poziomami.
- **Jeden rodzaj zaopatrzenia na raz** (wybierany intencją). Wejścia składników fabryka wybiera **raz** (to, z którego zrobi najwięcej) i trzyma się ich do wyczerpania zapasu; gracz może wybrać wejście ręcznie w panelu produkcji (klik w zamiennik; ponowny – z powrotem automatycznie). Produkcja: **1,0 sztuki na dzień za poziom** *(do strojenia)* × pokrycie obsady robotnikami ([budynki.md](budynki.md#obsada)) × premia uniwersytetów.
- **Zmiana rodzaju** = przezbrojenie: **5 dni** bez produkcji *(do strojenia)*.
- **Brak surowców**: fabryka produkuje tyle, na ile starcza składników (ułamek sztuki). Kolejność fabryk przy wspólnych składnikach: rosnąco po numerze prowincji (deterministycznie).

## Wojsko w walce ✅

Zasady w `core/src/war.rs` (stałe) i `Game::attack_tick` / `check_attack`.

- **Front**: wojsko wysłane (suwak % w panelu ataku; dwuklik na prowincję atakuje od razu z tym samym udziałem) na **jedną albo kilka prowincji** (Ctrl + klik). Co turę zajmuje kafle celów stykające się z naszymi – w kolejności: łatwiejszy teren, kafle otoczone naszymi, odrobina losowości (spójny, płynny front). Limity frontów i prowincji na froncie – [centrum dowodzenia](budynki.md#stolica-i-centrum-dowodzenia).
- **Styk**: wspólna granica, przeprawa przez wąską rzekę (do 4 kafli), **desant przez jezioro** nad naszym brzegiem (CD 1) albo **desant morski** (niżej). Kafel zajmowany z wody kosztuje i trwa **×1,5**.
- **Desant morski** (CD 2 – jeden naraz, CD 3 – bez limitu): wypływa z **najbliższego naszego portu** (budynek, który kontrolujemy) nad tym samym morzem; rejs trwa **2 dni załadunku + 1 dzień na 20 kafli** odległości; **paliwo z góry**: 0,02 na 1 000 wojska na kafel odległości; najwięcej wojska na desancie: CD 2 – 5 000, CD 3 – 15 000, CD 4 – 30 000, CD 5 – 60 000, CD 6 – 120 000 (także z dosłaniem). W rejsie front nie zajmuje kafli.
- **Tempo frontu**: punkty szybkości × **√(siła ataku na kafel frontu / obrona na kafel)**, przycięte do 0,3–4×: siła ataku = wojsko frontu × siła zaopatrzenia / długość frontu; obrona = tubylcy (gęstość × 0,25) albo gracz (gęstość wojska × siła zaopatrzenia), najmniej 20 na kafel. **Duża armia na wąskim froncie idzie dużo szybciej niż ta sama rozciągnięta** na kilka prowincji; koszt kafla się nie zmienia. Tempo widać w panelu frontów (×).
- **Teren** mnoży koszt i czas kafla: niziny 1,0, las 1,25, wyżyny 1,5, wyżyny z lasem 1,9.
- **Ziemia niczyja – armia tubylcza**: każda prowincja ma tubylców: **8 osób na kafel × infrastruktura startowa** (poziom 1 – 80%, 2 – 100%, 3 – 130%); nie odradzają się. Walka jest **symulowana**: co turę tubylcy w styczności z frontem (kafle frontu niczyje × gęstość) zabijają dziennie **30%** swojej liczby wojska atakującego (× los 50–150%), sami tracą 60%; do tego kafel niczyj kosztuje **2 + 10% gęstości**. W teście: ok. 4–5 wojska na kafel (straty widać w panelu frontów – †). Mają spowalniać, nie blokować.
- **Wspólny front (starcie)**: atak i kontratak dwóch państw w tych samych albo sąsiednich prowincjach (każde ma kafle w prowincjach frontu drugiego). Obie strony dalej zajmują kafle – front jest żywy i chaotyczny, słabszy też coś zdobywa – ale: kafla bronią razem wojsko w kraju (na kafel państwa) i armia przeciwnego frontu (na kafel jego państwa w prowincjach obu frontów), a przy zajęciu kafla obie tracą ludzi według swojej siły; armie frontów co turę ścierają się – dziennie ginie razem 2 × **2%** mniejszej armii (× los 50–150%), dzielone odwrotnie do siły (wojsko × siła zaopatrzenia), więc słabszy traci więcej. Kolejność rozliczania frontów odwraca się co turę (bez przewagi pierwszego ruchu). W panelu frontów – jeden wiersz „starcie”: obie armie, tempo i straty obu stron, pasek przeciągania liny (`war::CLASH_PCT`, `Game::opposed` / `clash` / `defense`).
- **Automatyczny koniec frontu** (resztka wojska wraca do kraju): gdy armia frontu spadnie poniżej **2%** wojska wysłanego na niego (z posiłkami) albo gdy front stoi **30 dni** bez zdobytego kafla i bez posiłków – na wspólnym froncie dopiero, gdy tak stoją obie strony. Panel frontów pokazuje zastój od 5. dnia z odliczaniem (`war::AUTO_END_PCT`, `STALL_TICKS`, `Attack::sent` / `idle`).
- **Kocioł**: kafle państwa, do których nie da się dojść po jego kaflach (bokiem i przez wąską rzekę) od stolicy, od ziemi niczyjej ani od morza – z każdej strony wrogowie (albo góry, rzeki, jeziora) – bronią się na **40%** (koszt kafla i tempo frontu). Kocioł może obejmować kilka prowincji; liczony raz dziennie, gdy trwa walka z graczem (`war::POCKET_*`, `Game::update_pockets`). Panel frontów pokazuje liczbę kafli kotła.
- **Aneksja** po zdobyciu kafla stolicy – [budynki.md](budynki.md#stolica-i-centrum-dowodzenia).
- **Chaos frontu**: szybkość co turę × los 60–140%; kępy 6 × 6 kafli z własnym losem co 1,2 s; co 2,5 s front wybiera punkty natarcia (1 na 40 kafli frontu), wokół których idzie szybciej – linia faluje i wybrzusza się zamiast przesuwać się równo.
- **Wyspy prowincji**: kawałek prowincji za wodą zajmuje się desantem bez wymogu centrum dowodzenia, jeśli mamy już brzeg tej samej prowincji nad tym morzem (koszt ×1,5).
- **Kafel gracza**: 20 + gęstość wojska obrońcy na kafel × 200% × (siła zaopatrzenia obrońcy / siła atakującego); obrońca traci gęstość wojska za każdy kafel. Obrona nie zużywa zaopatrzenia (liczy się najlepsze, jakie ma w puli).
- **Siła** zaopatrzenia (walka z graczem zużywa **0,05 sztuki na dzień na 1 000 wojska**, najlepsze odblokowane najpierw; współczynnik to średnia ważona pokryciem):

| Zaopatrzenie | Siła | Od CD | Paliwo na 1 000 wojska / dzień |
|---|---|---|---|
| brak (także ziemia niczyja) | 0,25 | – | 0,05 |
| lekkie | 0,6 (CD 3: **0,8**) | 1 | 0,10 (CD 3: 0,06) |
| standardowe | 1,0 (CD 4: **1,2**) | 2 | 0,20 (CD 4: 0,12) |
| ciężkie | 1,4 (CD 6: **1,6**) | 4 | 0,40 (CD 6: 0,24) |

- **Paliwo**: szybkość zajmowania × (1 + 0,5 × pokrycie paliwem). Zużycie zależy od zaopatrzenia w walce (tabela) – zajmowanie ziemi niczyjej jest tanie, ciężka armia pali dużo.
- **Konie – częściowy zamiennik paliwa**: gdy paliwa brakuje, walka bierze konie z puli surowców: 1 Konie = 0,5 Paliwa, najwyżej **50%** potrzeby paliwa.
- Wszystkie fronty gracza biorą z **tej samej puli** – wiele frontów szybciej ją wyczerpuje. Zużyte wojsko ginie, reszta wraca do kraju po zakończeniu albo odwołaniu frontu.
- **Umocnienia** – obrona kafli prowincji **+50% za poziom** (1–3): koszt kafla i tempo frontu atakującego; działają dla tego, kto ma ponad połowę prowincji, więc zdobyte bronią zdobywcy (`war::FORT_DEFENSE_PCT`). Razem z kotłem mnożą się.

## Ludność: prowiant i cywile ✅

Stałe w `core/src/economy.rs`. W oknie zapasów przy prowiancie i cywilnych – **zaspokojenie 0–100%** (do 25% czerwone, do 50% pomarańczowe, do 75% żółte, dalej zielone).

- **Prowiant** (jedzenie): **1 sztuka na dzień na 200 000 osób**. Przyrost populacji × **150% × zaspokojenie**; przy braku – **głód**: ubywa do **0,5% populacji dziennie** (część bez jedzenia). Prowiant robią **fabryki** z surowców spożywczych (receptura wyżej) – surowce zostają w puli, więc nadwyżkę można później sprzedać na rynku (handlujemy tylko surowcami). Poziom pól (1,1/d) karmi mniej więcej jeden poziom fabryki prowiantu, a ten – ok. 50 000 osób.
- **Podstawowe cywilne**: **1 sztuka na dzień na 150 000 osób** – średnie pokrycie z miesiąca ustala podatek: **50%** bez niego, **100%** przy pełnym (liniowo); bez pełnego nie działa luksusowe (do +50% ponad to).
- **Luksusowe cywilne**: **1 sztuka na dzień na 300 000 osób**, zużywane i liczone tylko przy pełnym podstawowym: podatek (robotnicy × 1 złota na początku miesiąca) dostaje do **+50%** według **średniego zaspokojenia luksusowego z miesiąca**.
- **Start**: 300 prowiantu, 30 soli, 150 podstawowego cywilnego, 50 paliwa, 100 materiałów inżynieryjnych.

## Utrzymanie wojska

**Poza walką wojsko nie zużywa zaopatrzenia.** Jego kosztem jest to, że **nie płaci podatków** i **nie obsadza budynków** (patrz [budynki.md](budynki.md#obsada)) – suwak wojsko/robotnicy już wymusza wybór między armią a gospodarką. Dodatkowe utrzymanie podwójnie karałoby za trzymanie wojska i utrudniało obronę słabszym graczom. Wrócimy do tego po pierwszych grach z walką, jeśli wojsko „w zapasie” okaże się zbyt tanie (wtedy np. 1 szt. podstawowego cywilnego na dzień na 50 000 wojska).

## Handel zaopatrzeniem

Na rynku handluje się **tylko surowcami** – zaopatrzenie produkuje się samemu (patrz [rynek.md](rynek.md#co-jest-na-rynku)).
