# Plan wdrażania

Kolejność etapów gospodarki w kodzie. Każdy etap to osobna gałąź `claude/<nazwa>` z testami rdzenia, testem pętli tur na dwóch kartach (zgodne hashe) i zrzutami interfejsu. Kolejność do uzgodnienia – ekspansja (etap 6) może pójść wcześniej w prostej wersji bez zaopatrzenia.

| # | Etap | Rdzeń (`crates/core`) | Interfejs |
|---|---|---|---|
| 1 ✅ | **Surowce i pula gracza** | `goods.rs`: tabela surowców (typ wydobycia, cena bazowa); zapasy na państwo; wydobycie bazowe 0,10/dzień; złoto z kopalni złota do skarbca | okno zapasów, ikony surowców (piktogramy i symbole) |
| 2 ✅ | **Budynki i infrastruktura** | `buildings.rs`: poziomy, kontrola budynku przez właściciela jego kafla, miejsca na mapie (`slots.rs`), infrastruktura (punkty), koszty złota i materiałów, czas budowy, obsada robotnikami, nawozy | okno prowincji, ikony budynków na mapie z budową, okienko budynku |
| 3 ✅ | **Fabryki i zaopatrzenie** | receptury z zamiennikami, wybór rodzaju, przezbrojenie, wejścia składników (automatycznie do wyczerpania albo ręcznie) | panel produkcji fabryki, receptury jako „funkcja” |
| 4 ⏳ | **Cywile → podatki** | zużycie zaopatrzenia cywilnego, mnożnik podatku w `economy.rs` (zapas startowy już jest) | pokrycie i mnożnik podatku w pasku |
| 5 (częściowo) | **Centrum dowodzenia i stolica** | ✅ poziomy CD w stolicy i odblokowania (fronty, desanty, siła i paliwo zaopatrzenia, umocnienia); ⏳ zmiana stolicy | gwiazdki nad stolicą i w tabelach odblokowań |
| 6 (bez aneksji) ✅ | **Ekspansja i walka** | fronty na wiele prowincji, przeprawy, desanty przez jeziora i morze, armia tubylcza, siła × zaopatrzenie, paliwo według zaopatrzenia (konie jako zamiennik); ⏳ aneksja po utracie stolicy | panel ataku (Ctrl + klik, dwuklik), menu kołowe, panel frontów |
| 7 (częściowo) | **Miasto, uniwersytet, umocnienia** | ✅ limit populacji z miast, progi uniwersytetów (+10%, paliwo syntetyczne, +20%); ⏳ efekt umocnień, dalsze progi | – |
| 8 ⏳ | **Rynek i porty** | automatyczny handel surowcami wg [rynek.md](rynek.md) | okno rynku z suwakami grup |

## Czego jeszcze nie ma

Mechaniki opisane w GDD, których nie ma w kodzie (kolejność do uzgodnienia):

- **Aneksja** po utracie stolicy (terytorium, połowa złota, 25% populacji) i **koniec gry** dla pokonanego.
- **Zmiana stolicy** za złoto (raz na rok gry).
- **Zaopatrzenie cywilne → podatki** (zużycie przez populację, mnożnik 0,5–1,5).
- **Efekt umocnień** w walce (budować już można).
- **Mosty i tunele** (budowa; prowincje za tunelem są dziś nie do zaatakowania).
- **Rynek** surowców, **porty** (przepustowość handlu), handlarze spoza mapy, transport lądem zależny od infrastruktury.
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
