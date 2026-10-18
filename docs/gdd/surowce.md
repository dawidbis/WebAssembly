# Surowce

Każda prowincja ma jeden **stały surowiec** (`Province.resource`), wylosowany przy generowaniu mapy z tabeli szans zależnej od biomu i ukształtowania (README „Zasoby prowincji”, `crates/mapgen/src/resources.rs`). Surowiec się nie zmienia – zmienia się tylko to, ile go wydobywamy.

## Wydobycie

- Prowincja **bez budynku wydobycia** daje **0,10 surowca na dzień** (zbieractwo, drobne gospodarstwa) – niezależnie od wielkości prowincji. Produkcja trafia do właściciela kafla wydobycia ([budynki.md](budynki.md#kontrola-budynków-)).
- Każdy **poziom budynku wydobycia** dodaje **+1,0 na dzień** *(do strojenia)*, pomnożone przez pokrycie obsady robotnikami (patrz [budynki.md](budynki.md#obsada)).
- Wydobyty surowiec trafia do **globalnej puli gracza** – nie ma lokalnych magazynów ani transportu. Utrata prowincji zatrzymuje jej wydobycie, ale nie zabiera zapasu.
- Typ budynku wydobycia **wynika z surowca** – gracz buduje po prostu „wydobycie” w prowincji, a gra pokazuje odpowiednią nazwę i ikonę.
- **Pola uprawne, plantacje, rancza i łowiska** kosztują na każdym poziomie **tylko złoto** (bez materiałów inżynieryjnych) – jedzenie da się rozwijać od pierwszych minut.
- Surowce spożywcze: **skrobia** (Zboża, Ryż, Kukurydza, Ziemniaki) i **białko** (Bydło, Trzoda chlewna, Drób, Ryby) – prowiant potrzebuje obu i **soli** ([zaopatrzenie.md](zaopatrzenie.md)).

| Budynek wydobycia | Surowce |
|---|---|
| Pola uprawne | Zboża, Ryż, Kukurydza, Ziemniaki, Bawełna, Włókna roślinne, Tytoń, Trzcina cukrowa, Kawa, Herbata |
| Plantacja | Kauczuk |
| Ranczo | Bydło, Trzoda chlewna, Drób, Konie, Wełna |
| Łowisko (brzeg morza albo jeziora) | Ryby |
| Myśliwi | Futra |
| Tartak | Drewno |
| Kopalnia | Węgiel kamienny, Węgiel brunatny, Rudy żelaza, Mangan, Chrom, Nikiel, Wolfram, Miedź, Ołów, Aluminium (boksyt), Cyna, Złoto, Surowce nawozowe, Saletra, Siarka, Sól |
| Szyb naftowy | Ropa naftowa |
| Kamieniołom | Cement |

## Złoto

Złoto **nie trafia do puli i nie ma go na rynku**: każda wydobyta jednostka od razu dodaje **150 złota** do skarbca *(do strojenia)*. Dla porównania: robotnik płaci do 0,1 złota miesięcznie (w interfejsie; 50–100% według podstawowego zaopatrzenia), więc 12 500 robotników to 625–1 250 złota na miesiąc, a kopalnia złota poziomu 1 daje ok. 4 500 złota na miesiąc. Na mapie są tylko 1–4 prowincje ze złotem (co najmniej jedna gwarantowana) – o nie toczy się walka od startu.

## Ceny bazowe

Cena bazowa to punkt wyjścia dla rynku ([rynek.md](rynek.md)) – złoto z interfejsu za 1 jednostkę *(do strojenia)*. Rzadkie i strategiczne surowce są droższe.

| Grupa | Surowiec: cena |
|---|---|
| Rolnictwo | Zboża 8 · Ryż 8 · Kukurydza 7 · Ziemniaki 6 · Bawełna 12 · Włókna roślinne 10 · Tytoń 30 · Trzcina cukrowa 14 · Kawa 30 · Herbata 30 · Kauczuk 30 |
| Hodowla i połowy | Bydło 12 · Trzoda chlewna 11 · Drób 10 · Ryby 9 · Konie 20 · Wełna 12 · Futra 24 |
| Las | Drewno 8 |
| Energia | Węgiel kamienny 14 · Węgiel brunatny 9 · Ropa naftowa 28 |
| Metale | Rudy żelaza 15 · Mangan 30 · Chrom 40 · Nikiel 40 · Wolfram 60 · Miedź 25 · Ołów 18 · Aluminium 35 · Cyna 30 · Złoto – poza rynkiem |
| Chemia | Surowce nawozowe 12 · Cement 10 · Saletra 20 · Siarka 18 · Sól 16 |

## Do czego służą surowce

Każdy surowiec (poza złotem) jest składnikiem co najmniej jednego zaopatrzenia ([zaopatrzenie.md](zaopatrzenie.md)) – pilnuje tego test `every_resource_has_a_use` w `factories.rs`.

| Surowiec | Zaopatrzenie (receptura) |
|---|---|
| Zboża, Ryż, Kukurydza, Ziemniaki | **prowiant** – skrobia (wymienne): Zboża 0,6 · Ryż 0,55 · Kukurydza 0,6 · Ziemniaki 0,8 na sztukę |
| Bydło, Trzoda chlewna, Drób, Ryby | **prowiant** – białko (wymienne): Bydło 0,35 · Trzoda chlewna 0,35 · Drób 0,4 · Ryby 0,4 na sztukę |
| Sól | **prowiant** – 0,1 na sztukę, **bez zamiennika** (konserwacja); ~3,7% prowincji (pustynie, suche wybrzeża, góry); soli na start nie ma – złoże, podbój albo handel ([rynek.md](rynek.md)) |
| Bawełna, Włókna roślinne, Wełna | lekkie wojskowe, podstawowe cywilne; bawełna także w luksusowym (zamiast futer) |
| Kawa, Herbata, Tytoń | luksusowe cywilne (wymienne) |
| Trzcina cukrowa | luksusowe cywilne (bez zamiennika) |
| Futra | luksusowe cywilne (albo bawełna) |
| Kauczuk | ciężkie wojskowe |
| Konie | gorszy zamiennik paliwa (2 Konie za 1 Paliwo) w podstawowym cywilnym i materiałach inżynieryjnych; w walce – do 50% potrzeby paliwa |
| Drewno | podstawowe cywilne, materiały inżynieryjne |
| Węgiel kamienny | standardowe i ciężkie wojskowe, podstawowe cywilne (ogrzewanie), paliwo syntetyczne (uniwersytet) |
| Węgiel brunatny | gorszy zamiennik węgla kamiennego (1,5 raza więcej) – standardowe wojskowe, podstawowe cywilne, paliwo syntetyczne; nie w ciężkim |
| Ropa naftowa | paliwo (a przez nie także podstawowe cywilne), kauczuk syntetyczny (uniwersytet) |
| Rudy żelaza | całe zaopatrzenie wojskowe (lekkie – albo miedź), materiały inżynieryjne |
| Mangan, Chrom, Nikiel | standardowe wojskowe (stal stopowa, wymienne) |
| Wolfram | ciężkie wojskowe (pociski przeciwpancerne, bez zamiennika); rzadki (~0,6% prowincji) |
| Miedź | lekkie wojskowe (zamiast żelaza), luksusowe cywilne |
| Ołów | amunicja – lekkie wojskowe (albo cyna) |
| Cyna | amunicja (zamiast ołowiu), luksusowe cywilne (zamiast miedzi) |
| Aluminium | ciężkie wojskowe |
| Saletra, Siarka | lekkie i standardowe wojskowe (proch, wymienne) |
| Surowce nawozowe | premia do wydobycia pól uprawnych (niżej); zamiennik saletry w wojskowym (azotany, 0,3 zamiast 0,2) |
| Cement | materiały inżynieryjne |

## Rozkład na mapie

Szanse surowców zależą od biomu i ukształtowania (tabela generowana: `tools/resources/chances.mjs`). Każdy biom ma **tożsamość** – kilka mocnych surowców (zasoby dominujące w skrypcie) i wyraźne braki, więc kierunek ekspansji i handlu wynika z mapy. Udział każdego surowca w prowincjach jest dobrany kalibracją (`--calibrate`, skale w `tools/resources/scale.json`) do celu z [balans.md](balans.md):

| Grupa | Udział prowincji (6 map) |
|---|---|
| Jedzenie (skrobia ~11%, białko ~8%) | ~20% |
| Włókna | ~9% |
| Luksus (kawa, herbata i tytoń – egzotyczne, po ~0,35%) | ~5,5% |
| Drewno | ~7,5% |
| Energia | ~13% |
| Metale | ~26% |
| Chemia (cement ~5%, sól ~3,7%, nawozy ~2,5%, siarka ~1%) | ~14% |
| Kauczuk, konie | ~5% |

| Biom | Mocne |
|---|---|
| Las deszczowy | kauczuk, trzcina cukrowa, aluminium, cyna |
| Sawanna | bawełna, bydło, mangan, chrom |
| Pustynia | ropa, miedź, saletra, nawozy, sól |
| Step | konie, bydło, wełna, miedź |
| Śródziemnomorski | cement, ołów, ryby, sól (tytoń – rzadko, jak kawa i herbata) |
| Subtropikalny | ryż, kukurydza, herbata, żelazo, wolfram |
| Oceaniczny | ryby, wełna, węgiel kamienny, cyna |
| Gorące lato | kukurydza, trzoda chlewna, żelazo, węgiel kamienny |
| Ciepłe lato | zboża, ziemniaki, węgiel brunatny, mangan |
| Borealny | drewno, nikiel, węgiel brunatny, ryby |
| Tajga | drewno, futra, wolfram, nikiel |
| Tundra | węgiel brunatny, futra, nikiel, ryby |

Każda mapa ma tundrę, pustynię i biom tropikalny (**gwarancja biomów**, `MapGenParams::biome_guarantee`) – choćby jako fragment kontynentu albo mała wyspa.

**Uran** – usunięty z mapy do etapu broni jądrowej; wróci (nowa wersja generatora) z ceną zależną od postępu uniwersytetów graczy.


## Nawozy

Surowce nawozowe działają **automatycznie** jako premia do rolnictwa – gracz niczego nie ustawia:

- Każdy poziom **pól uprawnych** (wydobycie zbóż, ryżu, ziemniaków, bawełny, włókien, tytoniu, trzciny, kawy, herbaty) zużywa **0,2 nawozów na dzień** z puli gracza *(do strojenia)*.
- **Pokrycie nawozami** = min(1, zapas / dzienna potrzeba wszystkich pól) – liczone codziennie, nawozy zużywane w tej proporcji.
- Wydobycie pól uprawnych × **(1 + 0,5 × pokrycie)** – pełne nawożenie daje **+50%** *(do strojenia)*. Wydobycie bazowe 0,10/dzień (bez budynku) nie korzysta z nawozów.
- Brak nawozów niczego nie blokuje – pola po prostu dają mniej. Nawozy można kupić na rynku (grupa „chemia”), więc gracz z dużym rolnictwem ma powód, żeby je importować.

## Otwarte pytania

- Brak – kolejne pytania dopiszemy przy wdrażaniu.
