# Budynki

Budynki **nie są osobnymi obiektami na mapie** – są częścią prowincji. W jednej prowincji może stać najwyżej **jeden budynek danego typu**, rozwijany **poziomami**.

## Typy budynków

| Budynek | Gdzie | Maks. poziom | Działanie |
|---|---|---|---|
| **Wydobycie** (typ z surowca: kopalnia, pola uprawne…) | każda prowincja | 10 | +1,0 surowca/dzień za poziom ([surowce.md](surowce.md)) |
| **Fabryka** | każda prowincja | 10 | 1,0 szt. zaopatrzenia/dzień za poziom, jeden rodzaj naraz ([zaopatrzenie.md](zaopatrzenie.md#fabryki)) |
| **Miasto** | każda prowincja | 5 | +25 000 do limitu populacji (globalnie) za poziom |
| **Uniwersytet** | każda prowincja | 5 | poziomy sumują się w państwie → progi technologii (niżej) |
| **Umocnienia** | każda prowincja | 3 | zwiększają straty atakującego; poziom 1 od centrum dowodzenia 3, do 3 od CD 5 |
| **Port** | prowincja nad morzem | 3 | handel morski ([rynek.md](rynek.md)), desant morski (CD 3) |
| **Centrum dowodzenia** | tylko stolica | 6 | odblokowania (niżej) |

Maksymalne poziomy *(do strojenia)*.

## Budowa

- Intencja `Build { province, kind }` – rdzeń sprawdza: prowincja należy do gracza, poziom poniżej maksimum, odblokowanie (centrum dowodzenia), złoto w skarbcu.
- **Koszt w złocie** (z interfejsu) = koszt bazowy × **1,5^(poziom − 1)** *(do strojenia)*:

| Budynek | Koszt bazowy (poziom 1) |
|---|---|
| Wydobycie | 1 000 |
| Fabryka | 2 000 |
| Miasto | 4 000 |
| Uniwersytet | 3 000 |
| Umocnienia | 1 500 |
| Port | 2 000 |
| Centrum dowodzenia | poziom 2: 3 000, każdy kolejny ×2 |

  Dla porównania: na starcie gracz ma ok. 12 500 robotników, czyli ok. 1 250 złota podatku na miesiąc.
- **Czas budowy** = **10 + 5 × poziom dni** *(do strojenia)* – poziom 1 ok. 15 s, poziom 10 ok. 1 min. Złoto pobierane na początku budowy.
- **Jedna budowa na prowincję naraz**; wiele prowincji może budować równolegle. Anulowanie zwraca np. 50% kosztu.
- Utrata prowincji: budynki zostają w prowincji (nowy właściciel je przejmuje – np. z poziomem obniżonym o 1 za zniszczenia; do decyzji przy walce).

## Obsada

Budynki pracują dzięki **robotnikom** (suwak wojsko/robotnicy w pasku zasobów):

| Budynek | Robotników na poziom |
|---|---|
| Wydobycie | 1 000 |
| Fabryka | 2 000 |
| Uniwersytet | 1 500 |
| Miasto, umocnienia, port, centrum dowodzenia | 0 |

- **Pokrycie obsady** = min(1, robotnicy / suma wymaganej obsady wszystkich budynków) i **mnoży całą produkcję** (wydobycie, fabryki, uniwersytety). Bez wybierania, który budynek staje – prosto i deterministycznie.
- Wydobycie bazowe 0,10/dzień w prowincji bez budynku nie wymaga obsady.
- Konsekwencja: przesunięcie suwaka w stronę wojska zmniejsza produkcję i podatki – realny wybór między armią a gospodarką.

## Centrum dowodzenia

Stoi w **stolicy** (prowincji startowej) od początku gry na poziomie 1. Utrata stolicy: centrum przenosi się do innej prowincji gracza z poziomem obniżonym o 1 *(do decyzji przy walce)*.

| Poziom | Odblokowuje |
|---|---|
| 1 | lekkie zaopatrzenie wojskowe |
| 2 | standardowe zaopatrzenie wojskowe; przeprawy przez rzeki; desant przez jeziora; atak na więcej niż jedną prowincję naraz (z jednej naszej) |
| 3 | desant morski; budowa tuneli i mostów; umocnienia do poziomu 1; atak z wielu naszych prowincji na wiele prowincji wroga |
| 4 | ciężkie zaopatrzenie wojskowe; kolejkowanie ataków po zajęciu całej prowincji |
| 5 | umocnienia do poziomu 3; większy limit desantu morskiego |
| 6 | broń jądrowa *(w przyszłości)* |

## Miasto

+25 000 do **limitu populacji państwa** za każdy poziom (globalnie – dodaje się do limitu 100 osób na kafel). Nie wymaga obsady. Większy limit = szybszy przyrost i więcej robotników, ale też większe zużycie zaopatrzenia cywilnego.

## Uniwersytet

Poziomy uniwersytetów w całym państwie się **sumują**; przekroczenie progu odblokowuje technologię *(progi do strojenia)*:

| Suma poziomów | Technologia |
|---|---|
| 5 | fabryki +10% produkcji |
| 10 | paliwo syntetyczne z węgla (2,0 Węgiel → 1 Paliwo) |
| 15 | kauczuk syntetyczny z ropy (1,0 Ropa → 1 Kauczuk – w fabryce) |
| 20 | fabryki +20% (łącznie) |
| 25 | zamienniki w recepturach (np. ołów/cynk/cyna w wojskowym, nawozy w cywilnym) |
| 40 | wzbogacanie uranu *(w przyszłości)* |

## Otwarte pytania

- Czy budynki tracą poziom przy zajęciu prowincji przez wroga?
- Gdzie przenosi się centrum dowodzenia po utracie stolicy?
- Czy miasto ma limit na państwo (np. 1 na 5 prowincji), żeby nie było „samych miast”?
