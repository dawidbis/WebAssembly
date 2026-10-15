# Surowce

Każda prowincja ma jeden **stały surowiec** (`Province.resource`), wylosowany przy generowaniu mapy z tabeli szans zależnej od biomu i ukształtowania (README „Zasoby prowincji”, `crates/mapgen/src/resources.rs`). Surowiec się nie zmienia – zmienia się tylko to, ile go wydobywamy.

## Wydobycie

- Prowincja **bez budynku wydobycia** daje **0,10 surowca na dzień** (zbieractwo, drobne gospodarstwa) – niezależnie od wielkości prowincji. Produkcja prowincji trafia do jej właściciela (ponad 50% kafli – [budynki.md](budynki.md#właściciel-budynków)).
- Każdy **poziom budynku wydobycia** dodaje **+1,0 na dzień** *(do strojenia)*, pomnożone przez pokrycie obsady robotnikami (patrz [budynki.md](budynki.md#obsada)).
- Wydobyty surowiec trafia do **globalnej puli gracza** – nie ma lokalnych magazynów ani transportu. Utrata prowincji zatrzymuje jej wydobycie, ale nie zabiera zapasu.
- Typ budynku wydobycia **wynika z surowca** – gracz buduje po prostu „wydobycie” w prowincji, a gra pokazuje odpowiednią nazwę i ikonę.

| Budynek wydobycia | Surowce |
|---|---|
| Pola uprawne | Zboża, Ryż, Ziemniaki, Bawełna, Włókna roślinne, Tytoń, Trzcina cukrowa, Kawa, Herbata |
| Plantacja | Kauczuk |
| Ranczo | Bydło, Konie, Wełna |
| Myśliwi | Futra |
| Tartak | Drewno |
| Kopalnia | Węgiel kamienny, Węgiel brunatny, Uran, Rudy żelaza, Mangan, Chrom, Nikiel, Wolfram, Molibden, Miedź, Ołów, Cynk, Aluminium (boksyt), Cyna, Złoto, Surowce nawozowe, Saletra, Siarka |
| Szyb naftowy | Ropa naftowa |
| Kamieniołom | Cement |

## Złoto

Złoto **nie trafia do puli i nie ma go na rynku**: każda wydobyta jednostka od razu dodaje **50 złota** do skarbca *(do strojenia)*. Dla porównania: robotnik płaci 0,1 złota miesięcznie (w interfejsie), więc 12 500 robotników to ok. 1 250 złota na miesiąc, a kopalnia złota poziomu 1 daje ok. 1 500 złota na miesiąc. Na mapie są tylko 1–4 prowincje ze złotem (co najmniej jedna gwarantowana) – o nie toczy się walka od startu.

## Ceny bazowe

Cena bazowa to punkt wyjścia dla rynku ([rynek.md](rynek.md)) – złoto z interfejsu za 1 jednostkę *(do strojenia)*. Rzadkie i strategiczne surowce są droższe.

| Grupa | Surowiec: cena |
|---|---|
| Rolnictwo | Zboża 8 · Ryż 8 · Ziemniaki 6 · Bawełna 12 · Włókna roślinne 10 · Tytoń 18 · Trzcina cukrowa 14 · Kawa 22 · Herbata 22 · Kauczuk 30 |
| Hodowla | Bydło 12 · Konie 20 · Wełna 12 · Futra 24 |
| Las | Drewno 8 |
| Energia | Węgiel kamienny 14 · Węgiel brunatny 9 · Ropa naftowa 28 · Uran 10 → 120 (zmienna, niżej) |
| Metale | Rudy żelaza 15 · Mangan 30 · Chrom 40 · Nikiel 40 · Wolfram 60 · Molibden 60 · Miedź 25 · Ołów 18 · Cynk 18 · Aluminium 35 · Cyna 30 · Złoto – poza rynkiem |
| Chemia | Surowce nawozowe 12 · Cement 10 · Saletra 20 · Siarka 18 |

## Do czego służą surowce

Każdy surowiec jest składnikiem co najmniej jednego zaopatrzenia ([zaopatrzenie.md](zaopatrzenie.md)). Surowce bez zastosowania w recepturach dostają je z czasem (uniwersytet – zamienniki), żeby żadna prowincja nie była „pusta”.

| Surowiec | Zaopatrzenie (receptura) |
|---|---|
| Zboża, Ryż, Ziemniaki, Bydło | podstawowe cywilne |
| Bawełna, Włókna roślinne, Wełna | lekkie wojskowe, podstawowe cywilne |
| Tytoń, Trzcina cukrowa, Kawa, Herbata | luksusowe cywilne |
| Futra | luksusowe cywilne |
| Kauczuk | ciężkie wojskowe |
| Konie | częściowy zamiennik paliwa (do 50% potrzeby – patrz zaopatrzenie) |
| Drewno | podstawowe cywilne, materiały inżynieryjne |
| Węgiel kamienny | standardowe i ciężkie wojskowe, podstawowe cywilne (ogrzewanie), paliwo syntetyczne (uniwersytet) |
| Węgiel brunatny | gorszy zamiennik węgla kamiennego (1,5 raza więcej) – standardowe wojskowe, podstawowe cywilne, paliwo syntetyczne; nie w ciężkim |
| Ropa naftowa | paliwo (a przez nie także podstawowe cywilne), kauczuk syntetyczny (uniwersytet) |
| Rudy żelaza | całe zaopatrzenie wojskowe, materiały inżynieryjne |
| Mangan, Chrom, Nikiel | standardowe i ciężkie wojskowe |
| Wolfram, Molibden | ciężkie wojskowe |
| Miedź | lekkie wojskowe (zamiast żelaza), luksusowe cywilne |
| Ołów, Cynk | amunicja – lekkie i standardowe wojskowe (wymienne) |
| Cyna | luksusowe cywilne |
| Aluminium | ciężkie wojskowe |
| Saletra, Siarka | lekkie i standardowe wojskowe (proch) |
| Surowce nawozowe | premia do wydobycia pól uprawnych (niżej) |
| Cement | materiały inżynieryjne |
| Uran | wzbogacony uran – broń jądrowa (później); do tego czasu towar eksportowy o rosnącej cenie |

## Cena uranu

Uran do niczego nie służy, dopóki nikt nie umie go wzbogacać – dlatego jego cena bazowa **zależy od rozwoju technologicznego graczy**:

- **postęp** = największa suma poziomów uniwersytetów wśród graczy / 40 (próg wzbogacania uranu, [budynki.md](budynki.md#uniwersytet)), przycięty do 0–1,
- **cena bazowa uranu** = 10 + 110 × postęp² – długo niska, rośnie szybko, gdy któryś gracz zbliża się do wzbogacania; po odblokowaniu wzbogacania przez kogokolwiek – pełne 120,
- cena liczona raz dziennie, zmienia się płynnie jak cena rynkowa ([rynek.md](rynek.md#cena-rynkowa)).

Dzięki temu prowincje z uranem na starcie są przeciętne, a w późnej grze stają się celem – kto je ma, zarabia na eksporcie, zanim sam zacznie wzbogacać.

## Nawozy

Surowce nawozowe działają **automatycznie** jako premia do rolnictwa – gracz niczego nie ustawia:

- Każdy poziom **pól uprawnych** (wydobycie zbóż, ryżu, ziemniaków, bawełny, włókien, tytoniu, trzciny, kawy, herbaty) zużywa **0,2 nawozów na dzień** z puli gracza *(do strojenia)*.
- **Pokrycie nawozami** = min(1, zapas / dzienna potrzeba wszystkich pól) – liczone codziennie, nawozy zużywane w tej proporcji.
- Wydobycie pól uprawnych × **(1 + 0,5 × pokrycie)** – pełne nawożenie daje **+50%** *(do strojenia)*. Wydobycie bazowe 0,10/dzień (bez budynku) nie korzysta z nawozów.
- Brak nawozów niczego nie blokuje – pola po prostu dają mniej. Nawozy można kupić na rynku (grupa „chemia”), więc gracz z dużym rolnictwem ma powód, żeby je importować.

## Otwarte pytania

- Brak – kolejne pytania dopiszemy przy wdrażaniu.
