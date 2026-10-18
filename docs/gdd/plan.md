# Plan wdrażania

Kolejność etapów gospodarki w kodzie. Każdy etap to osobna gałąź `claude/<nazwa>` z testami rdzenia, testem pętli tur na dwóch kartach (zgodne hashe) i zrzutami interfejsu. Kolejność do uzgodnienia – ekspansja (etap 6) może pójść wcześniej w prostej wersji bez zaopatrzenia.

| # | Etap | Rdzeń (`crates/core`) | Interfejs |
|---|---|---|---|
| 1 ✅ | **Surowce i pula gracza** | `goods.rs`: tabela surowców (typ wydobycia, cena bazowa); zapasy na państwo; wydobycie bazowe 0,10/dzień; złoto z kopalni złota do skarbca | okno zapasów, ikony surowców (piktogramy i symbole) |
| 2 ✅ | **Budynki i infrastruktura** | `buildings.rs`: poziomy, kontrola budynku przez właściciela jego kafla, miejsca na mapie (`slots.rs`), infrastruktura (punkty), koszty złota i materiałów, czas budowy, obsada robotnikami, nawozy | okno prowincji, ikony budynków na mapie z budową, okienko budynku |
| 3 ✅ | **Fabryki i zaopatrzenie** | receptury z zamiennikami, wybór rodzaju, przezbrojenie, wejścia składników (automatycznie do wyczerpania albo ręcznie) | panel produkcji fabryki, receptury jako „funkcja” |
| 4 ✅ | **Ludność** | prowiant z fabryk (8 surowców spożywczych, wymienne) → przyrost (do 150%) i głód; podstawowe cywilne – podatek 50–100% i warunek luksusowego; luksusowe (przy pełnym podstawowym) → do +50% podatku | zaspokojenie w oknie zapasów (kolory), podpowiedzi paska |
| 5 ✅ | **Centrum dowodzenia i stolica** | ✅ poziomy CD w stolicy i odblokowania (fronty, desanty, siła i paliwo zaopatrzenia, umocnienia), zmiana stolicy za złoto (raz na rok gry) | gwiazdki nad stolicą i w tabelach odblokowań |
| 6 ✅ | **Ekspansja i walka** | wspólny front ataku i kontrataku (starcie armii, wspólna obrona, jeden wiersz w panelu), aneksja po zdobyciu kafla stolicy (terytorium, 50% złota, 25% populacji, 30% magazynu) i koniec gry, kotły (okrążone kafle bronią się na 40%), fronty na wiele prowincji, tempo ze stosunku sił, przeprawy, desanty przez jeziora i z portu przez morze (rejs, paliwo z góry, limit wojska), armia tubylcza z potyczkami, siła × zaopatrzenie, paliwo według zaopatrzenia (konie jako zamiennik); ⏳ aneksja po utracie stolicy | panel ataku (Ctrl + klik, dwuklik), menu kołowe, panel frontów |
| 7 (częściowo) | **Miasto, uniwersytet, umocnienia** | ✅ limit populacji z miast, progi uniwersytetów (+10%, paliwo syntetyczne, +20%), umocnienia (obrona +50% za poziom); ⏳ dalsze progi uniwersytetów | – |
| 8 ✅ | **Rynek i porty** | `market.rs` + `game/trade.rs`: codzienny automatyczny handel surowcami wg [rynek.md](rynek.md) – bilans każdego surowca jako % przepustowości (porty × obsada + fabryki), handlarze tylko z portem, cena z popytu i podaży, trasy lądem i morzem (Dijkstra, infrastruktura, porty, cło tranzytowe), handlarze spoza mapy, udział w rynku | okno rynku (bilans z powodami, ceny z trendem, wczorajsze transakcje), rynek w księdze złota |

## Czego jeszcze nie ma

Mechaniki opisane w GDD, których nie ma w kodzie (kolejność do uzgodnienia):

- **Mosty i tunele** (budowa; prowincje za tunelem są dziś nie do zaatakowania).
- **Uniwersytety**: kauczuk syntetyczny (15), dodatkowe zamienniki w recepturach (25), wzbogacanie uranu (40).
- **Broń jądrowa** (CD 6, wzbogacony uran).
- **Kolejkowanie ataków** (dawny pomysł z CD 4 – w nowej tabeli go nie ma, do decyzji).
- **Utrzymanie wojska** poza walką – celowo brak (do powrotu po pierwszych grach).
- **Dyplomacja** – poza zakresem.

## Zasady wspólne

- Wszystkie ilości w **liczbach całkowitych** (np. tysięczne części jednostki), rozliczane co turę – determinizm natywny vs wasm.
- Stan w `Game`, inicjalizowany w `from_map`, dopisany do `state_hash`; mapa pozostaje niezmienna.
- Intencje walidowane w rdzeniu; stałe zasad gry na górze modułów (bez suwaków w panelu debugu).
- Interfejs dostaje raporty z workera raz na sekundę (jak pasek zasobów – `NationReport`).
