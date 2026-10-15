# Budynki

Budynki **nie są osobnymi obiektami na mapie** – są częścią prowincji. W jednej prowincji może stać najwyżej **jeden budynek danego typu**, rozwijany **poziomami**.

## Właściciel budynków

- Budynki należą do **prowincji**, a korzysta z nich **właściciel prowincji**: gracz, który ma **ponad 50% kafli** prowincji (udziały – `Owners::shares`, `core/src/owners.rs`). Tylko on może w niej budować i to do jego puli trafia produkcja (także wydobycie bazowe 0,10/dzień).
- Gdy nikt nie ma ponad 50% (prowincja podzielona frontem), budynki **stoją** – nikt nie produkuje ani nie buduje, a trwająca budowa jest wstrzymana.
- **Budynki nigdy nie tracą poziomu** – przy zmianie właściciela prowincja przechodzi do nowego gracza razem ze wszystkimi budynkami i ich poziomami.

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

- Intencja `Build { province, kind }` – rdzeń sprawdza: gracz jest właścicielem prowincji (> 50% kafli), poziom poniżej maksimum, odblokowanie (centrum dowodzenia), złoto w skarbcu (i materiały inżynieryjne przy umocnieniach).
- **Koszt w złocie** (z interfejsu) = koszt bazowy × **1,5^(poziom − 1)** *(do strojenia)*:

| Budynek | Koszt bazowy (poziom 1) |
|---|---|
| Wydobycie | 10 000 |
| Fabryka | 20 000 |
| Miasto | 40 000 |
| Uniwersytet | 30 000 |
| Umocnienia | 15 000 + materiały inżynieryjne |
| Port | 20 000 |
| Centrum dowodzenia | poziom 2: 30 000, każdy kolejny ×2 |

  Dla porównania: na starcie gracz ma ok. 12 500 robotników, czyli ok. 1 250 złota podatku na miesiąc – pierwszy budynek wydobycia to ok. 8 miesięcy gry (4 minuty) oszczędzania albo szybciej z kopalnią złota / handlem. Budynki są dużą inwestycją, a wczesna gra opiera się na wydobyciu bazowym.
- **Czas budowy** = **10 + 5 × poziom dni** *(do strojenia)* – poziom 1 ok. 15 s, poziom 10 ok. 1 min. Złoto pobierane na początku budowy.
- **Jedna budowa na prowincję naraz**; wiele prowincji może budować równolegle. Anulowanie zwraca 50% kosztu.

## Obsada

Budynki pracują dzięki **robotnikom** (suwak wojsko/robotnicy w pasku zasobów):

| Budynek | Robotników na poziom |
|---|---|
| Wydobycie | 10 000 |
| Fabryka | 20 000 |
| Uniwersytet | 15 000 |
| Miasto, umocnienia, port, centrum dowodzenia | 0 |

- **Pokrycie obsady** = min(1, robotnicy / suma wymaganej obsady wszystkich budynków państwa) i **mnoży całą produkcję** (wydobycie, fabryki, uniwersytety). Bez wybierania, który budynek staje – prosto i deterministycznie.
- Wydobycie bazowe 0,10/dzień w prowincji bez budynku nie wymaga obsady.
- Konsekwencja: przesunięcie suwaka w stronę wojska zmniejsza produkcję i podatki – realny wybór między armią a gospodarką. Przy limicie ok. 60 000 osób na start (połowa to robotnicy) państwo utrzyma w pełni ok. jednego budynku wydobycia i jednej fabryki – więcej wymaga ekspansji albo miast.

## Stolica i centrum dowodzenia

- **Stolica** to prowincja startowa; stoi w niej **centrum dowodzenia** (poziom 1 od początku gry).
- **Zmiana stolicy** za złoto: np. 20 000 × liczba dotychczasowych zmian + 1, nie częściej niż **raz na rok gry (6 min)** *(do strojenia)*. Centrum dowodzenia przenosi się razem ze stolicą i **zachowuje poziom** (budynki nie tracą poziomu).
- **Utrata stolicy = przegrana gracza** – **aneksja**: zdobywca przejmuje całe pozostałe terytorium pokonanego (z budynkami), **połowę jego złota** i **25% jego żywej populacji** (dopisane do populacji zdobywcy w proporcjach wojsko/robotnicy pokonanego, ponad limit – nadwyżka spada zwykłym ubytkiem ponad limitem). Reszta zapasów pokonanego (surowce, zaopatrzenie) przepada.

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

- Czy miasto ma limit na państwo (np. 1 na 5 prowincji), żeby nie było „samych miast”?
- Czy przy tak drogich budynkach (×10) i dużej obsadzie start nie jest zbyt wolny – do sprawdzenia w pierwszych grach.
