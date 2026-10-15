# Plan wdrażania

Kolejność etapów gospodarki w kodzie. Każdy etap to osobna gałąź `claude/<nazwa>` z testami rdzenia, testem pętli tur na dwóch kartach (zgodne hashe) i zrzutami interfejsu. Kolejność do uzgodnienia – ekspansja (etap 6) może pójść wcześniej w prostej wersji bez zaopatrzenia.

| # | Etap | Rdzeń (`crates/core`) | Interfejs |
|---|---|---|---|
| 1 | **Surowce i pula gracza** | `goods.rs`: tabela surowców (typ wydobycia, cena bazowa – z kolejności `RESOURCE_NAMES`); zapasy na państwo; wydobycie bazowe 0,10/dzień z każdej prowincji; złoto z kopalni złota do skarbca | panel zapasów surowców |
| 2 | **Budynki** | `buildings.rs`: poziomy w prowincji, intencja `Build`, koszt złota, czas budowy, obsada robotnikami (pokrycie mnoży produkcję) | budynki i budowa w ramce prowincji |
| 3 | **Fabryki i zaopatrzenie** | receptury z zamiennikami, intencja wyboru rodzaju fabryki, przezbrojenie, zapasy zaopatrzenia | wybór produkcji fabryki, zapasy zaopatrzenia |
| 4 | **Cywile → podatki** | zużycie zaopatrzenia cywilnego, mnożnik podatku w `economy.rs`, zapas startowy | pokrycie i mnożnik podatku w pasku |
| 5 | **Centrum dowodzenia** | poziomy w stolicy, flagi odblokowań | poziom i odblokowania |
| 6 | **Ekspansja i walka** | zajmowanie prowincji wojskiem, siła × zaopatrzenie, paliwo, przeprawy wg centrum dowodzenia | ataki na mapie |
| 7 | **Miasto, uniwersytet, umocnienia** | limit populacji, progi technologii, straty atakującego | – |
| 8 | **Rynek i porty** | po doprecyzowaniu [rynek.md](rynek.md) | okno rynku |

## Zasady wspólne

- Wszystkie ilości w **liczbach całkowitych** (np. tysięczne części jednostki), rozliczane co turę – determinizm natywny vs wasm.
- Stan w `Game`, inicjalizowany w `from_map`, dopisany do `state_hash`; mapa pozostaje niezmienna.
- Intencje walidowane w rdzeniu; stałe zasad gry na górze modułów (bez suwaków w panelu debugu).
- Interfejs dostaje raporty z workera raz na sekundę (jak pasek zasobów – `NationReport`).
