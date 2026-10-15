# Plan wdrażania

Kolejność etapów gospodarki w kodzie. Każdy etap to osobna gałąź `claude/<nazwa>` z testami rdzenia, testem pętli tur na dwóch kartach (zgodne hashe) i zrzutami interfejsu. Kolejność do uzgodnienia – ekspansja (etap 6) może pójść wcześniej w prostej wersji bez zaopatrzenia.

| # | Etap | Rdzeń (`crates/core`) | Interfejs |
|---|---|---|---|
| 1 ✅ | **Surowce i pula gracza** (zrobione) | `goods.rs`: tabela surowców (typ wydobycia, cena bazowa – z kolejności `RESOURCE_NAMES`); zapasy na państwo; wydobycie bazowe 0,10/dzień z każdej prowincji; złoto z kopalni złota do skarbca | panel zapasów surowców |
| 2 | **Budynki i infrastruktura** | `buildings.rs`: poziomy w prowincji (nigdy nie maleją), właściciel = gracz z ponad 50% kafli, infrastruktura (punkty na budynki, poziom startowy z cech prowincji), intencja `Build`, koszt złota i materiałów inżynieryjnych (zapas startowy 30, pierwszy poziom wydobycia i fabryki tylko za złoto), czas budowy, obsada robotnikami (pokrycie mnoży produkcję i przepustowość portów); nawozy dla pól uprawnych | budynki i budowa w ramce prowincji |
| 3 | **Fabryki i zaopatrzenie** | receptury z zamiennikami (najpierw materiały inżynieryjne – potrzebne do budowy), intencja wyboru rodzaju fabryki, przezbrojenie, zapasy zaopatrzenia | wybór produkcji fabryki, zapasy zaopatrzenia |
| 4 | **Cywile → podatki** | zużycie zaopatrzenia cywilnego, mnożnik podatku w `economy.rs`, zapas startowy | pokrycie i mnożnik podatku w pasku |
| 5 | **Centrum dowodzenia i stolica** | poziomy w stolicy, flagi odblokowań, zmiana stolicy za złoto (raz na rok gry) | poziom i odblokowania |
| 6 | **Ekspansja i walka** | zajmowanie prowincji wojskiem, siła × zaopatrzenie, paliwo (konie jako zamiennik), przeprawy wg centrum dowodzenia, materiały inżynieryjne; utrata stolicy = aneksja (teren, połowa złota, 25% populacji) | ataki na mapie |
| 7 | **Miasto, uniwersytet, umocnienia** | limit populacji, progi technologii, straty atakującego | – |
| 8 | **Rynek i porty** | automatyczny handel surowcami wg [rynek.md](rynek.md): suwaki bilansu, pojemność (porty z obsadą), cena z popytu i podaży, trasy Dijkstrą po grafie prowincji (infrastruktura, porty, cło tranzytowe pośredników), handlarze spoza mapy | okno rynku z suwakami grup |

## Zasady wspólne

- Wszystkie ilości w **liczbach całkowitych** (np. tysięczne części jednostki), rozliczane co turę – determinizm natywny vs wasm.
- Stan w `Game`, inicjalizowany w `from_map`, dopisany do `state_hash`; mapa pozostaje niezmienna.
- Intencje walidowane w rdzeniu; stałe zasad gry na górze modułów (bez suwaków w panelu debugu).
- Interfejs dostaje raporty z workera raz na sekundę (jak pasek zasobów – `NationReport`).
