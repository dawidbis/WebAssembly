# Zaopatrzenie

Zaopatrzenie to dobra wytwarzane w **fabrykach** z surowców z globalnej puli. Dzieli się na **wojskowe** (siła wojska w walce), **paliwo** (szybkość zajmowania) i **cywilne** (wysokość podatków).

## Rodzaje i receptury

Receptura na **1 sztukę** *(do strojenia)*. Składniki rozdzielone „|” są wymienne – fabryka bierze ten, którego gracz ma najwięcej (dzięki temu brak jednego metalu nie blokuje produkcji).

| Rodzaj | Odblokowanie | Receptura na 1 szt. | Do czego |
|---|---|---|---|
| **Lekkie wojskowe** | centrum dowodzenia 1 | 0,5 Rudy żelaza \| Miedź + 0,3 Bawełna \| Wełna \| Włókna roślinne + 0,2 Saletra \| Siarka | walka |
| **Standardowe wojskowe** | centrum dowodzenia 2 | 0,6 Rudy żelaza + 0,2 Mangan \| Chrom \| Nikiel + 0,3 Saletra \| Siarka + 0,2 Węgiel kamienny \| brunatny | walka |
| **Ciężkie wojskowe** | centrum dowodzenia 4 | 0,8 Rudy żelaza + 0,3 Chrom \| Nikiel \| Wolfram \| Molibden + 0,3 Węgiel kamienny + 0,2 Kauczuk + 0,2 Aluminium | walka |
| **Paliwo** | od startu | 1,0 Ropa naftowa (uniwersytet: albo 2,0 Węgiel – paliwo syntetyczne) | szybkość zajmowania |
| **Podstawowe cywilne** | od startu | 0,6 Zboża \| Ryż \| Ziemniaki \| Bydło + 0,2 Bawełna \| Wełna \| Włókna roślinne + 0,2 Drewno | podatki |
| **Luksusowe cywilne** | od startu | 0,3 Kawa \| Herbata \| Tytoń \| Trzcina cukrowa + 0,2 Futra \| Wełna + 0,1 Miedź \| Cyna | podatki (mocniej) |
| **Materiały inżynieryjne** | centrum dowodzenia 3 | 0,5 Cement + 0,3 Drewno + 0,2 Rudy żelaza | mosty, tunele, umocnienia |
| *Wzbogacony uran* (później) | centrum dowodzenia 6 + uniwersytet | 2,0 Uran + 1,0 Paliwo | broń jądrowa |

**Materiały inżynieryjne** zużywa budowa mostów, tuneli i umocnień (obok złota) – dzięki temu cement i drewno mają znaczenie wojskowe.

## Fabryki

- Fabryka stoi w dowolnej prowincji gracza (położenie nie ma znaczenia – pula jest globalna); w prowincji najwyżej jedna fabryka, rozwijana poziomami.
- **Jeden rodzaj zaopatrzenia na raz** (wybierany intencją). Produkcja: **1,0 sztuki na dzień za poziom** *(do strojenia)* × pokrycie obsady robotnikami ([budynki.md](budynki.md#obsada)) × premia uniwersytetów.
- **Zmiana rodzaju** = przezbrojenie: **5 dni** bez produkcji *(do strojenia)*.
- **Brak surowców**: fabryka produkuje tyle, na ile starcza składników (ułamek sztuki). Kolejność fabryk przy wspólnych składnikach: rosnąco po numerze prowincji (deterministycznie).

## Wojsko w walce

Szczegóły walki powstaną w etapie ekspansji – tu tylko rola zaopatrzenia.

- **Siła** = walczące wojsko × **współczynnik zaopatrzenia**:

| Zaopatrzenie | Współczynnik |
|---|---|
| brak | **0,25** – wojsko bez zaopatrzenia jest bardzo nieefektywne |
| lekkie | 0,6 |
| standardowe | 1,0 |
| ciężkie | 1,4 |

- Walka **zużywa najlepsze dostępne** zaopatrzenie (jeśli gracz ma do niego dostęp – centrum dowodzenia): np. **1 sztuka na dzień na 1 000 walczącego wojska** *(do strojenia)*. Gdy najlepszego zabraknie, część wojska walczy na niższym; współczynnik to średnia ważona tym, ile wojska ma które zaopatrzenie.
- Wszystkie walki gracza biorą z **tej samej puli** – wiele frontów szybciej ją wyczerpuje.
- **Paliwo**: szybkość zajmowania × (1 + 0,5 × pokrycie paliwem) *(do strojenia)*; zużycie np. 0,5 sztuki na dzień na 1 000 walczącego wojska.
- **Konie – częściowy zamiennik paliwa**: gdy paliwa brakuje, walka bierze konie z puli surowców: 1 Konie = 0,5 Paliwa, ale konie pokrywają najwyżej **50%** potrzeby paliwa *(do strojenia)* – wojsko konne jest szybsze od pieszego, wolniejsze od zmotoryzowanego.

## Cywile i podatki

- Populacja **zużywa** zaopatrzenie cywilne: **1 sztuka podstawowego na dzień na 25 000 osób** i (gdy jest) **1 sztuka luksusowego na dzień na 50 000 osób** *(do strojenia)*.
- **Podatek** (początek każdego miesiąca) = robotnicy × 1 złota × **mnożnik**:
  - pokrycie podstawowego 0% → mnożnik **0,5**, 100% → **1,0** (liniowo),
  - luksusowe działa **dopiero przy pełnym podstawowym**: dodaje do **+0,5** liniowo z pokryciem.
- Pokrycie liczymy ze średniej z miesiąca (czy w każdym dniu starczyło), żeby jednorazowy zakup tuż przed podatkiem nie oszukiwał systemu.
- Na start każdy gracz ma mały zapas podstawowego (np. 30 sztuk – miesiąc dla 25 000 osób), żeby pierwszy podatek nie był od razu o połowę mniejszy.

## Utrzymanie wojska

**Poza walką wojsko nie zużywa zaopatrzenia.** Jego kosztem jest to, że **nie płaci podatków** i **nie obsadza budynków** (patrz [budynki.md](budynki.md#obsada)) – suwak wojsko/robotnicy już wymusza wybór między armią a gospodarką. Dodatkowe utrzymanie podwójnie karałoby za trzymanie wojska i utrudniało obronę słabszym graczom. Wrócimy do tego po pierwszych grach z walką, jeśli wojsko „w zapasie” okaże się zbyt tanie (wtedy np. 1 szt. podstawowego cywilnego na dzień na 50 000 wojska).

## Handel zaopatrzeniem

Na rynku handluje się **tylko surowcami** – zaopatrzenie produkuje się samemu (patrz [rynek.md](rynek.md#co-jest-na-rynku)).
