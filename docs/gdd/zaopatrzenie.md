# Zaopatrzenie

Zaopatrzenie to dobra wytwarzane w **fabrykach** z surowców z globalnej puli. Dzieli się na **wojskowe** (siła wojska w walce), **paliwo** (szybkość zajmowania) i **cywilne** (wysokość podatków).

## Rodzaje i receptury

Receptura na **1 sztukę** *(do strojenia)*. Składniki rozdzielone „|” są wymienne – fabryka bierze ten, którego gracz ma najwięcej (dzięki temu brak jednego metalu nie blokuje produkcji). **Węgiel brunatny** jest gorszym zamiennikiem węgla kamiennego: wszędzie potrzeba go **1,5 raza więcej** (zapis „0,2 Węgiel kamienny | 0,3 Węgiel brunatny”); w ciężkim zaopatrzeniu (koks hutniczy) zastąpić się go nie da. Składnik „Paliwo” to zaopatrzenie z fabryk paliwa (półprodukt) – fabryki podstawowego cywilnego i materiałów inżynieryjnych biorą je z puli zaopatrzenia. **Konie** są tam gorszym zamiennikiem paliwa (0,1 Paliwo | 0,2 Konie – transport konny), więc gracz bez ropy może budować i zaopatrywać ludność, tylko drożej.

| Rodzaj | Odblokowanie | Receptura na 1 szt. | Do czego |
|---|---|---|---|
| **Lekkie wojskowe** | centrum dowodzenia 1 | 0,5 Rudy żelaza \| Miedź + 0,3 Bawełna \| Wełna \| Włókna roślinne + 0,2 Saletra \| Siarka + 0,1 Ołów \| Cynk (amunicja) | walka |
| **Standardowe wojskowe** | centrum dowodzenia 2 | 0,6 Rudy żelaza + 0,2 Mangan \| Chrom \| Nikiel + 0,3 Saletra \| Siarka + 0,2 Ołów \| Cynk (amunicja) + 0,2 Węgiel kamienny \| 0,3 Węgiel brunatny | walka |
| **Ciężkie wojskowe** | centrum dowodzenia 4 | 0,8 Rudy żelaza + 0,3 Chrom \| Nikiel \| Wolfram \| Molibden + 0,3 Węgiel kamienny + 0,2 Kauczuk + 0,2 Aluminium | walka |
| **Paliwo** | od startu | 1,0 Ropa naftowa (uniwersytet: albo 2,0 Węgiel kamienny \| 3,0 Węgiel brunatny – paliwo syntetyczne) | szybkość zajmowania, podstawowe cywilne |
| **Podstawowe cywilne** | od startu | 0,6 Zboża \| Ryż \| Ziemniaki \| Bydło + 0,2 Bawełna \| Wełna \| Włókna roślinne + 0,2 Drewno + 0,2 Węgiel kamienny \| 0,3 Węgiel brunatny (ogrzewanie) + 0,1 Paliwo \| 0,2 Konie (transport, oświetlenie) | podatki |
| **Luksusowe cywilne** | od startu | 0,3 Kawa \| Herbata \| Tytoń \| Trzcina cukrowa + 0,2 Futra \| Wełna + 0,1 Miedź \| Cyna | podatki (mocniej) |
| **Materiały inżynieryjne** | od startu | 0,5 Cement + 0,3 Drewno + 0,2 Rudy żelaza + 0,1 Paliwo \| 0,2 Konie (maszyny i transport) | budowa i rozbudowa wszystkich budynków, ⏳ mosty i tunele (od CD 2) |
| *Wzbogacony uran* (później) | centrum dowodzenia 6 + uniwersytet | 2,0 Uran + 1,0 Paliwo | broń jądrowa |

**Materiały inżynieryjne** są potrzebne (obok złota) do **budowy i rozbudowy wszystkich budynków** oraz mostów i tuneli – cement, drewno i żelazo stają się podstawą rozwoju. Żeby start się nie zablokował, pierwszy poziom wydobycia i fabryki kosztuje tylko złoto, a każdy gracz zaczyna z 50 materiałami ([budynki.md](budynki.md#budowa)).

## Fabryki

- Fabryka stoi w dowolnej prowincji gracza (położenie nie ma znaczenia – pula jest globalna); w prowincji najwyżej jedna fabryka, rozwijana poziomami.
- **Jeden rodzaj zaopatrzenia na raz** (wybierany intencją). Wejścia składników fabryka wybiera **raz** (to, z którego zrobi najwięcej) i trzyma się ich do wyczerpania zapasu; gracz może wybrać wejście ręcznie w panelu produkcji (klik w zamiennik; ponowny – z powrotem automatycznie). Produkcja: **1,0 sztuki na dzień za poziom** *(do strojenia)* × pokrycie obsady robotnikami ([budynki.md](budynki.md#obsada)) × premia uniwersytetów.
- **Zmiana rodzaju** = przezbrojenie: **5 dni** bez produkcji *(do strojenia)*.
- **Brak surowców**: fabryka produkuje tyle, na ile starcza składników (ułamek sztuki). Kolejność fabryk przy wspólnych składnikach: rosnąco po numerze prowincji (deterministycznie).

## Wojsko w walce ✅

Zasady w `core/src/war.rs` (stałe) i `Game::attack_tick` / `check_attack`.

- **Front**: wojsko wysłane (suwak % w panelu ataku; dwuklik na prowincję atakuje od razu z tym samym udziałem) na **jedną albo kilka prowincji** (Ctrl + klik). Co turę zajmuje kafle celów stykające się z naszymi – w kolejności: łatwiejszy teren, kafle otoczone naszymi, odrobina losowości (spójny, płynny front). Limity frontów i prowincji na froncie – [centrum dowodzenia](budynki.md#stolica-i-centrum-dowodzenia).
- **Styk**: wspólna granica, przeprawa przez wąską rzekę (do 4 kafli), **desant przez jezioro** nad naszym brzegiem (CD 1) albo **desant morski** (CD 2 – jeden naraz, CD 3 – bez limitu). Kafel zajmowany z wody kosztuje i trwa **×1,5**.
- **Teren** mnoży koszt i czas kafla: niziny 1,0, las 1,25, wyżyny 1,5, wyżyny z lasem 1,9.
- **Ziemia niczyja – armia tubylcza**: każda prowincja ma tubylców: **8 osób na kafel × infrastruktura startowa** (poziom 1 – 80%, 2 – 100%, 3 – 130%); nie odradzają się. Walka jest **symulowana**: co turę tubylcy w styczności z frontem (kafle frontu niczyje × gęstość) zabijają dziennie **30%** swojej liczby wojska atakującego (× los 50–150%), sami tracą 60%; do tego kafel niczyj kosztuje **2 + 10% gęstości**. W teście: ok. 4–5 wojska na kafel (straty widać w panelu frontów – †). Mają spowalniać, nie blokować.
- **Chaos frontu**: szybkość co turę × los 60–140%; kępy 6 × 6 kafli z własnym losem co 1,2 s; co 2,5 s front wybiera punkty natarcia (1 na 40 kafli frontu), wokół których idzie szybciej – linia faluje i wybrzusza się zamiast przesuwać się równo.
- **Wyspy prowincji**: kawałek prowincji za wodą zajmuje się desantem bez wymogu centrum dowodzenia, jeśli mamy już brzeg tej samej prowincji nad tym morzem (koszt ×1,5).
- **Kafel gracza**: 20 + gęstość wojska obrońcy na kafel × 200% × (siła zaopatrzenia obrońcy / siła atakującego); obrońca traci gęstość wojska za każdy kafel. Obrona nie zużywa zaopatrzenia (liczy się najlepsze, jakie ma w puli).
- **Siła** zaopatrzenia (walka z graczem zużywa **1 sztukę na dzień na 1 000 wojska**, najlepsze odblokowane najpierw; współczynnik to średnia ważona pokryciem):

| Zaopatrzenie | Siła | Od CD | Paliwo na 1 000 wojska / dzień |
|---|---|---|---|
| brak (także ziemia niczyja) | 0,25 | – | 0,05 |
| lekkie | 0,6 (CD 3: **0,8**) | 1 | 0,10 (CD 3: 0,06) |
| standardowe | 1,0 (CD 4: **1,2**) | 2 | 0,20 (CD 4: 0,12) |
| ciężkie | 1,4 (CD 6: **1,6**) | 4 | 0,40 (CD 6: 0,24) |

- **Paliwo**: szybkość zajmowania × (1 + 0,5 × pokrycie paliwem). Zużycie zależy od zaopatrzenia w walce (tabela) – zajmowanie ziemi niczyjej jest tanie, ciężka armia pali dużo.
- **Konie – częściowy zamiennik paliwa**: gdy paliwa brakuje, walka bierze konie z puli surowców: 1 Konie = 0,5 Paliwa, najwyżej **50%** potrzeby paliwa.
- Wszystkie fronty gracza biorą z **tej samej puli** – wiele frontów szybciej ją wyczerpuje. Zużyte wojsko ginie, reszta wraca do kraju po zakończeniu albo odwołaniu frontu.
- ⏳ **Umocnienia** – zwiększą straty atakującego na kaflach prowincji (budowa już jest, efekt – nie).

## Cywile ✅

- Populacja **zużywa** zaopatrzenie cywilne: **1 sztuka podstawowego na dzień na 25 000 osób**, a gdy podstawowego starcza w pełni – także **1 sztuka luksusowego na dzień na 50 000 osób** *(do strojenia)*. Stałe w `core/src/economy.rs`.
- **Podstawowe → przyrost populacji**: przyrost × (50% + 100% × pokrycie) – bez podstawowego połowa, przy pełnym 150%. Samo podstawowe **nie podnosi podatku**.
- **Luksusowe → podatki**: działa tylko przy pełnym podstawowym; podatek (robotnicy × 1 złota na początku miesiąca) dostaje do **+50%** według **średniego pokrycia luksusowego z miesiąca** (jednorazowy zakup tuż przed podatkiem nie oszukuje).
- Na start każdy gracz ma 30 sztuk podstawowego (ok. 2 tygodnie dla startowej populacji).

## Utrzymanie wojska

**Poza walką wojsko nie zużywa zaopatrzenia.** Jego kosztem jest to, że **nie płaci podatków** i **nie obsadza budynków** (patrz [budynki.md](budynki.md#obsada)) – suwak wojsko/robotnicy już wymusza wybór między armią a gospodarką. Dodatkowe utrzymanie podwójnie karałoby za trzymanie wojska i utrudniało obronę słabszym graczom. Wrócimy do tego po pierwszych grach z walką, jeśli wojsko „w zapasie” okaże się zbyt tanie (wtedy np. 1 szt. podstawowego cywilnego na dzień na 50 000 wojska).

## Handel zaopatrzeniem

Na rynku handluje się **tylko surowcami** – zaopatrzenie produkuje się samemu (patrz [rynek.md](rynek.md#co-jest-na-rynku)).
