# Rynek

Rzadko który gracz ma wszystkie surowce potrzebne do produkcji – rynek pozwala kupić brakujące i sprzedać nadmiar. Handel jest **automatyczny**: gracz ustawia tylko **bilans import/eksport**, a gra codziennie sama decyduje, od kogo kupić, komu sprzedać i ile.

## Co jest na rynku

- **Tylko surowce** (półprodukty). Zaopatrzenie każdy produkuje sam – dzięki temu fabryki, obsada i receptury mają znaczenie, a rynek ma tylko 34 towary.
- **Złoto jest poza rynkiem** – wydobyte trafia od razu do skarbca ([surowce.md](surowce.md#złoto)).

## Suwak bilansu

Jeden suwak na **grupę surowców** (rolnictwo, hodowla, las, energia, metale, chemia – grupy jak w legendzie mapy zasobów), z możliwością **nadpisania dla pojedynczego surowca** (widok zaawansowany). Zakres **−100% … 0 … +100%**:

| Ustawienie | Działanie |
|---|---|
| **0** | nic nie kupuj, nic nie sprzedawaj |
| **+X%** (eksport) | sprzedawaj X% nadwyżki – przy +100% całą nadwyżkę |
| **−X%** (import) | kupuj X% brakującej ilości do nasycenia – przy −100% kupuj, aż nie osiągniesz nasycenia |

Pojęcia (liczone osobno dla każdego surowca, codziennie):

- **Potrzeba** = dzienne zużycie surowca przez fabryki gracza (receptury × poziomy × obsada).
- **Rezerwa** = 10 dni potrzeby – poniżej rezerwy nic nie eksportujemy *(do strojenia)*.
- **Nadwyżka** = zapas − rezerwa.
- **Nasycenie** = 30 dni potrzeby – import kończy się, gdy zapas je osiągnie *(do strojenia)*. Surowca, którego fabryki nie potrzebują, import nie kupuje.

## Pojemność handlu

Dzienny limit transakcji w danym surowcu zależy od budynków *(do strojenia)*:

- **eksport** = 1 + suma poziomów **wydobycia** tego surowca + **przepustowość portów**,
- **import** = 1 + suma poziomów **fabryk** + **przepustowość portów**,
- **przepustowość portów** = 3 × suma poziomów portów × **pokrycie obsady** (w portach pracują robotnicy – 5 000 na poziom, [budynki.md](budynki.md#obsada)).

Wyższe poziomy portów, fabryk i wydobycia = większe dzienne transakcje; niedobór robotników ogranicza handel morski.

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

- Okno rynku: suwaki grup (i nadpisania surowców), ceny z trendem, wczorajsze transakcje gracza (kto, ile, za ile, koszt transportu), udział w rynku.
- Gracz widzi, **dlaczego** coś kupiono („brak Rudy żelaza na 12 dni potrzeby fabryk”) – automat nie może być czarną skrzynką.

## Otwarte pytania

- Czy przy równych cenach udział w rynku ma też obniżać koszt (np. wielki eksporter = tańszy transport), czy tylko ustalać kolejność? Na razie – tylko kolejność.
- Blokada morska (wróg odcina porty) – po walce na morzu.
