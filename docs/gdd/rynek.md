# Rynek

> ✅ **W kodzie** (etap 8): reguły i stałe w `crates/core/src/market.rs`, rozliczenie w grze w `crates/core/src/game/trade.rs`, okno `web/src/app/ui/market-window.*` (przycisk z wagą w pasku zasobów). Uproszczenia pierwszej wersji – na końcu dokumentu.

Rzadko który gracz ma wszystkie surowce potrzebne do produkcji – rynek pozwala kupić brakujące i sprzedać nadmiar. Handel jest **automatyczny**: gracz ustawia tylko **bilans import/eksport**, a gra codziennie sama decyduje, od kogo kupić, komu sprzedać i ile.

## Co jest na rynku

- **Tylko surowce** (półprodukty). Zaopatrzenie każdy produkuje sam – dzięki temu fabryki, obsada i receptury mają znaczenie, a rynek ma tylko 34 towary.
- **Złoto jest poza rynkiem** – wydobyte trafia od razu do skarbca ([surowce.md](surowce.md#złoto)).

## Suwak bilansu

Jeden suwak na **każdy surowiec** (co 1%; w oknie pogrupowane jak w legendzie mapy zasobów – grupy nie mają własnych suwaków). Zakres **−100% … 0 … +100%** – to część **przepustowości handlu** (niżej), niezależnie od produkcji fabryk:

| Ustawienie | Działanie |
|---|---|
| **0** | nic nie kupuj, nic nie sprzedawaj |
| **+X%** (eksport) | sprzedawaj dziennie przepustowość × X% (najwyżej cały zapas) |
| **−X%** (import) | kupuj dziennie przepustowość × X% (ile starczy złota) |

Potrzeba fabryk (dzienne zużycie) okno pokazuje tylko informacyjnie – automat jej nie używa.

## Pojemność handlu

**Przepustowość handlu** (jednostki dziennie, ta sama dla każdego surowca osobno) = **porty** (5 na poziom × **pokrycie obsady** – w portach pracują robotnicy, 5 000 na poziom, [budynki.md](budynki.md#obsada)) + **fabryki** (1 na poziom) *(do strojenia)*. Bez portów i fabryk – brak handlu. Porty dają znacznie więcej; niedobór robotników ogranicza handel morski.

**Handel z handlarzami spoza mapy** (handel morski) – tylko z co najmniej jednym portem. Bez portu państwo handluje tylko z graczami, do których prowadzi trasa lądem.

## Cena rynkowa

Każdy surowiec ma cenę bazową ([surowce.md](surowce.md#ceny-bazowe)). Raz dziennie:

- **cena docelowa** = cena bazowa × ((popyt + L) / (podaż + L))^0,5, przycięta do **[0,25; 4] × cena bazowa**, gdzie popyt i podaż to zamówienia z ostatnich dni (średnia krocząca ok. 10 dni), a **L** (płynność, np. 20 jednostek/dzień) tłumi skoki przy małym handlu,
- cena bieżąca zbliża się do docelowej o 1/10 dziennie – bez gwałtownych wahań.

## Koszt dostawy

Importer płaci cenę rynkową **plus koszt dostawy**. Trasę wybiera gra – najtańszą z dostępnych *(wszystkie stawki do strojenia; D – przekątna mapy)*:

**Lądem** – od najbliższej prowincji **wydobycia** surowca u eksportera do najbliższej **fabryki** importera, po grafie sąsiedztwa prowincji (rzeki do 4 kafli przechodzi się jak mostem):

- każdy odcinek kosztuje **50% × długość / D × współczynnik infrastruktury** prowincji, przez którą biegnie: poziom 1 – 1,0 · 2 – 0,8 · 3 – 0,65 · 4 – 0,5 · 5 – 0,4 (lepsze drogi = taniej),
- **pośrednicy**: za każde obce państwo na trasie (poza eksporterem i importerem) **+5%** – to **cło tranzytowe**, które trafia do skarbca tego państwa (położenie na szlaku handlowym jest coś warte); ląd niczyj – bez cła.

**Morzem** – gdy obaj mają port: lądem od wydobycia do portu eksportera (jak wyżej), **morzem port – port**, lądem od portu importera do jego fabryki:

- odcinek morski: **25% × długość / D × współczynnik portów** – średnia z obu portów: poziom 1 – 1,0 · 2 – 0,75 · 3 – 0,5,
- **opłata portowa** po każdej stronie: 3% / 2% / 1% przy porcie poziomu 1 / 2 / 3,
- na morzu nie ma pośredników (wody międzynarodowe).

Bez żadnej trasy oferta jest dla tego importera niedostępna. Koszt dostawy (poza cłem tranzytowym) **znika z gry** – hamuje nadmiar złota. Eksporter dostaje cenę rynkową.

Trasy liczy rdzeń algorytmem Dijkstry na grafie prowincji i **zapamiętuje** je – przelicza tylko po zmianie właścicieli, infrastruktury albo portów, a nie codziennie.

## Dopasowanie (codziennie, w rdzeniu)

1. Każdy gracz zgłasza **ofertę** (nadwyżka × eksport%, w granicach pojemności) i **zapotrzebowanie** ((nasycenie − zapas) × import%, w granicach pojemności).
2. Dla każdego surowca importerzy są obsługiwani **po kolei** (kolejność przesuwa się co dzień, żeby nikt nie był zawsze pierwszy). Importer bierze z ofert **od najtańszej z dostawą**; przy równym koszcie dostawy oferty dzielą się proporcjonalnie do **udziału w rynku** eksporterów.
3. **Udział w rynku** gracza = wartość jego eksportu / wartość całego eksportu z ostatnich 30 dni – widoczny w statystykach, decyduje o kolejności sprzedaży przy równych cenach.
4. **Handlarze spoza mapy** zamykają rynek: kupują niesprzedaną nadwyżkę po **0,6 × cena rynkowa** i sprzedają brakujący towar po **1,6 × cena rynkowa** (plus transport jak morzem z najbliższej krawędzi mapy albo lądem z +50%) – gracz zawsze może handlować, ale z innymi graczami opłaca się bardziej.

Wszystko w liczbach całkowitych i w ustalonej kolejności – wynik identyczny u każdego gracza (determinizm, hash stanu).

## Interfejs

- Okno rynku: przepustowość handlu (porty + fabryki) i informacja o porcie, suwak każdego surowca (co 1%, z ilością dziennie i powodem), ceny z trendem i udziałem gracza w eksporcie surowca (sortowanie po nazwie, cenie, zmianie i udziale), wczorajsze transakcje gracza (kto, ile, za ile, koszt transportu); w nagłówku obrót całego rynku wczoraj.
- Gracz widzi, **dlaczego** coś kupiono („brak Rudy żelaza na 12 dni potrzeby fabryk”) – automat nie może być czarną skrzynką.

## Otwarte pytania

- Czy przy równych cenach udział w rynku ma też obniżać koszt (np. wielki eksporter = tańszy transport), czy tylko ustalać kolejność? Na razie – tylko kolejność.
- Blokada morska (wróg odcina porty) – po walce na morzu.

## W kodzie – uproszczenia pierwszej wersji

- **Rozliczenie** raz dziennie (co 10 tur gry właściwej), po fabrykach; surowce po kolei według numeru. Ustawienia na start: wszystkie surowce **0** (bez handlu).
- **Potrzeba** (tylko informacja w oknie) = zużycie fabryk na dzień (produkcja × obsada × premia uniwersytetów, wejście, które fabryka weźmie) + nawozy pól.
- **Przepustowość** w jednostkach na dzień: `PORT_CAPACITY` (5) × poziomy portów × pokrycie obsady + `FACTORY_CAPACITY` (1) × poziomy fabryk (`market.rs`).
- **Trasy**: odcinek lądem między sąsiednimi prowincjami ma długość odległości środków ich prostokątów, pół odcinka liczy się według infrastruktury każdej z nich. Źródło – prowincje surowca, w których eksporter kontroluje wydobycie (bez nich – stolica), cel – fabryki importera (bez nich – stolica). Rejs – w linii prostej port – port nad tym samym morzem; port eksportera to najbliższy (lądem) jego źródła. Trasy przelicza się, gdy zmienią się właściciele albo budynki.
- **Cło tranzytowe**: 5% wartości od każdego państwa, które jest właścicielem (> 50% kafli) prowincji na trasie lądem; wybór trasy omija obce państwa jak cło.
- **Remis kosztu dostawy**: pierwszeństwo ma eksporter z większym udziałem w rynku (zamiast podziału proporcjonalnego), potem niższy numer państwa.
- **Handlarze**: tylko z portem; dostawa morzem od najbliższej krawędzi mapy do portu importera z opłatą portową i lądem do fabryk (port bez drogi lądem do fabryk – ląd za 50% wartości); najtańszy z naszych portów.
- **Złoto**: importer kupuje tylko tyle, na ile ma złota (cena + dostawa); w księdze złota – „Rynek – sprzedaż”, „Rynek – zakupy”, „Cło tranzytowe”.
